//! SSML tag decoding and parameter directive plans.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ssml::{self, Attribute, Wide};
use crate::ssml_parameters::{Frame, PARAMETERS};
pub const TAG_UNITS: usize = 501;
#[path = "ssml_tag_data.rs"]
mod data;
use data::TAGS;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Empty,
    ByteDomain,
    Tag,
    Emphasis,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Tag {
    pub kind: i32,
    pub attributes: u32,
    pub separator: u32,
    pub self_closing: u32,
    pub ignore: u32,
    /// Replace that slash with a space before attribute processing; MAX = none.
    pub slash_index: u32,
}

pub fn tag(
    input: Wide<'_>,
    signed_bytes: bool,
    wide_space: impl Fn(u32) -> bool,
    byte_lower: impl Fn(u32) -> i32,
) -> Result<Tag, Error> {
    if input.len() > TAG_UNITS {
        return Err(Error::Bounds);
    }
    let length = (0..input.len())
        .find(|i| input.get(*i) == Some(0))
        .ok_or(Error::Bounds)?;
    if length == 0 {
        return Err(Error::Empty);
    }
    let self_closing = input.get(length - 1) == Some(47);
    let mut name = [0u8; 40];
    let mut index = 0;
    while index < 39 {
        let c = if self_closing && index == length - 1 {
            32
        } else {
            input.get(index).ok_or(Error::Bounds)?
        };
        if c == 0 || wide_space(c) {
            break;
        }
        let byte = c as u8;
        if signed_bytes && byte >= 128 && byte != 255 {
            return Err(Error::ByteDomain);
        }
        name[index] = byte_lower(u32::from(byte)) as u8;
        index += 1;
    }
    let end = name.iter().position(|c| *c == 0).expect("terminated name");
    let closing = name[0] == 47;
    let search = if closing { &name[1..end] } else { &name[..end] };
    let kind = TAGS
        .iter()
        .find(|(name, _)| *name == search)
        .map_or(0, |(_, kind)| *kind);
    Ok(Tag {
        kind: kind + if closing { 32 } else { 0 },
        attributes: index as u32,
        separator: u32::from(kind != 16),
        self_closing: u32::from(self_closing),
        ignore: u32::from(
            !closing && self_closing && matches!(kind, 1 | 2 | 3 | 4 | 9 | 10 | 12 | 14),
        ),
        slash_index: if self_closing {
            (length - 1) as u32
        } else {
            u32::MAX
        },
    })
}

pub struct Context<'a> {
    pub base: &'a [i32; PARAMETERS],
    pub current: &'a [i32; PARAMETERS],
    pub tone_language: i32,
    pub decimal: u32,
}

pub fn directive(
    kind: i32,
    input: Wide<'_>,
    start: usize,
    context: Context<'_>,
    space: impl Fn(u32) -> bool,
) -> Result<Frame, Error> {
    if !matches!(kind, 3 | 10 | 12) {
        return Err(Error::Tag);
    }
    if input.len() > TAG_UNITS
        || start == 0
        || start >= input.len()
        || !(start..input.len()).any(|i| input.get(i) == Some(0))
    {
        return Err(Error::Bounds);
    }
    let find = |name: &[u8]| ssml::attribute(input, start, name, &space).unwrap_or(None);
    let text = |value: Option<Attribute>| match value {
        Some(Attribute::Value(index)) => input.tail(index).ok_or(Error::Bounds),
        _ => Ok(Wide::U32(&[0])),
    };
    let lookup =
        |value: Option<Attribute>, table: &[(&[u8], i32)], fallback| -> Result<i32, Error> {
            let input = text(value)?;
            Ok(table
                .iter()
                .find(|(name, _)| ssml::attribute_matches(Some(input), name))
                .map_or(fallback, |(_, v)| *v))
        };
    let mut frame = Frame {
        kind,
        values: [-1; PARAMETERS],
    };
    match kind {
        3 => {
            for (index, name) in [b"rate".as_slice(), b"volume", b"pitch", b"range"]
                .iter()
                .enumerate()
            {
                let index = index + 1;
                if let Some(value) = find(name) {
                    if let Ok(value) = crate::ssml_prosody::parameter(
                        index,
                        text(Some(value))?,
                        context.base[index],
                        context.current[index],
                        context.decimal,
                        &space,
                    ) {
                        frame.values[index] = value;
                    }
                }
            }
        }
        10 => {
            let field = text(find(b"field"))?;
            let mode = find(b"mode");
            if ssml::attribute_matches(Some(field), b"punctuation") {
                frame.values[5] = lookup(mode, &[(b"none", 1), (b"all", 2), (b"some", 3)], -1)?;
            } else if ssml::attribute_matches(Some(field), b"capital_letters") {
                frame.values[6] = lookup(
                    mode,
                    &[(b"no", 0), (b"icon", 1), (b"spelling", 2), (b"pitch", 20)],
                    -1,
                )?;
            }
        }
        12 => {
            let level = find(b"level");
            let value = if level.is_none() {
                3
            } else {
                lookup(
                    level,
                    &[
                        (b"none", 1),
                        (b"reduced", 2),
                        (b"moderate", 3),
                        (b"strong", 4),
                        (b"x-strong", 5),
                    ],
                    -1,
                )?
            };
            let index = usize::try_from(value).map_err(|_| Error::Emphasis)?;
            if context.tone_language == 1 {
                frame.values[4] = *[50, 50, 40, 70, 90, 100]
                    .get(index)
                    .ok_or(Error::Emphasis)?;
                frame.values[2] = *[100, 100, 70, 110, 135, 150]
                    .get(index)
                    .ok_or(Error::Emphasis)?;
            } else {
                frame.values[2] = *[100, 100, 75, 100, 120, 150]
                    .get(index)
                    .ok_or(Error::Emphasis)?;
                frame.values[12] = value;
            }
        }
        _ => unreachable!("admitted directive"),
    }
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wide(s: &str) -> Vec<u32> {
        s.chars().map(u32::from).chain([0]).collect()
    }
    fn space(c: u32) -> bool {
        matches!(c, 9..=13 | 32)
    }
    fn lower(c: u32) -> i32 {
        (c as u8).to_ascii_lowercase() as i32
    }
    #[test]
    fn tags_preserve_case_whitespace_self_close_and_legacy_byte_narrowing() {
        let parsed = tag(Wide::U32(&wide("VOICE /")), true, space, lower).unwrap();
        assert_eq!(
            (
                parsed.kind,
                parsed.attributes,
                parsed.separator,
                parsed.ignore,
                parsed.slash_index
            ),
            (2, 5, 1, 1, 6)
        );
        let closing = tag(Wide::U32(&wide("/B/")), true, space, lower).unwrap();
        assert_eq!(
            (closing.kind, closing.separator, closing.ignore),
            (48, 0, 0)
        );
        assert_eq!(tag(Wide::U32(&[0]), true, space, lower), Err(Error::Empty));
        assert_eq!(
            tag(Wide::U32(&[0x180, 0]), true, space, lower),
            Err(Error::ByteDomain)
        );
        assert_eq!(
            tag(Wide::U32(&[0x10042, 0]), true, space, lower)
                .unwrap()
                .kind,
            16
        );
        assert_eq!(
            tag(Wide::U32(&wide("unknown/")), true, space, lower)
                .unwrap()
                .kind,
            0
        );
    }
    #[test]
    fn directives_plan_all_parameters_before_mutating_owner() {
        let base = [100; PARAMETERS];
        let current = [50; PARAMETERS];
        let context = |tone_language| Context {
            base: &base,
            current: &current,
            tone_language,
            decimal: 46,
        };
        let result = directive(
            3,
            Wide::U32(&wide(" rate='fast' pitch='+12st' volume='2147483648'")),
            1,
            context(0),
            space,
        )
        .unwrap();
        assert_eq!(result.values[1], 125);
        assert_eq!(result.values[3], 100);
        assert_eq!(result.values[2], -1);
        let tone = directive(
            12,
            Wide::U32(&wide(" level='strong'")),
            1,
            context(1),
            space,
        )
        .unwrap();
        assert_eq!(
            (tone.values[2], tone.values[4], tone.values[12]),
            (135, 90, -1)
        );
        assert_eq!(
            directive(
                12,
                Wide::U32(&wide(" level='unknown'")),
                1,
                context(0),
                space
            ),
            Err(Error::Emphasis)
        );
        let style = directive(
            10,
            Wide::U32(&wide(" field='capital_letters' mode='pitch'")),
            1,
            context(0),
            space,
        )
        .unwrap();
        assert_eq!(style.values[6], 20);
    }
}
