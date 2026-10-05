//! Ordered SSML voice-stack composition with explicit name resolution.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

pub const STACK: usize = 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Frame {
    pub kind: i32,
    pub variant: i32,
    pub gender: i32,
    pub age: i32,
    pub name: [u8; 40],
    pub language: [u8; 20],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Choice {
    pub name: [u8; 40],
    pub identifier: [u8; 40],
    pub language: [u8; 40],
    pub gender: u32,
    pub age: u32,
    pub variant: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Resolver,
}

fn terminated(input: &[u8]) -> Result<&[u8], Error> {
    input
        .iter()
        .position(|b| *b == 0)
        .map(|length| &input[..length])
        .ok_or(Error::Bounds)
}
fn copy<const N: usize>(output: &mut [u8; N], value: &[u8]) -> Result<(), Error> {
    if value.len() >= N {
        return Err(Error::Capacity);
    }
    output[..value.len()].copy_from_slice(value);
    output[value.len()] = 0;
    Ok(())
}
fn languages(input: &[u8]) -> Result<Option<&[u8]>, Error> {
    if input.len() > 300 {
        return Err(Error::Bounds);
    }
    let mut pos = 0;
    let mut first = None;
    loop {
        let priority = *input.get(pos).ok_or(Error::Bounds)?;
        if priority == 0 {
            return Ok(first);
        }
        let word = terminated(&input[pos + 1..])?;
        if word.len() >= 40 {
            return Err(Error::Capacity);
        }
        if first.is_none() {
            first = Some(word);
        }
        pos += word.len() + 2;
    }
}
fn language_provided(input: &[u8], wanted: &[u8]) -> bool {
    let mut pos = 0;
    while input[pos] != 0 {
        let word = terminated(&input[pos + 1..]).expect("validated language list");
        if word == wanted {
            return true;
        }
        pos += word.len() + 2;
    }
    false
}

/// Resolver calls are ordered. Each returns an owned identifier snapshot and
/// must keep frames, base metadata and prior identifier immutable and alive.
/// No exclusive engine/catalogue owner borrow is held across resolution.
pub fn choice(
    frames: &[Frame],
    base_languages: &[u8],
    previous_identifier: &[u8; 40],
    mut resolve: impl FnMut(&[u8; 40]) -> Result<Option<[u8; 40]>, Error>,
) -> Result<Choice, Error> {
    if frames.is_empty() || frames.len() > STACK {
        return Err(Error::Bounds);
    }
    terminated(previous_identifier)?;
    let primary = languages(base_languages)?;
    for frame in frames {
        terminated(&frame.name)?;
        terminated(&frame.language)?;
    }
    let first = &frames[0];
    let mut result = Choice {
        name: [0; 40],
        identifier: *previous_identifier,
        language: [0; 40],
        gender: u32::from(first.gender as u8),
        age: u32::from(first.age as u8),
        variant: u32::from(first.variant as u8),
    };
    copy(&mut result.name, terminated(&first.name)?)?;
    copy(&mut result.language, terminated(&first.language)?)?;
    for frame in frames {
        let name = terminated(&frame.name)?;
        let mut specified = false;
        if !name.is_empty() {
            if let Some(identifier) = resolve(&frame.name)? {
                let identifier = terminated(&identifier)?;
                copy(&mut result.name, name)?;
                copy(&mut result.identifier, identifier)?;
                result.language[0] = 0;
                result.gender = 0;
                result.age = 0;
                result.variant = 0;
                specified = true;
            }
        }
        let language = terminated(&frame.language)?;
        if !language.is_empty() {
            let language = if language_provided(base_languages, language) {
                primary.expect("provided implies a primary language")
            } else {
                language
            };
            copy(&mut result.language, language)?;
            if !specified {
                result.name[0] = 0;
                result.identifier[0] = 0;
            }
        }
        if frame.gender != 0 {
            result.gender = u32::from(frame.gender as u8);
        }
        if frame.age != 0 {
            result.age = u32::from(frame.age as u8);
        }
        if frame.variant != 0 {
            result.variant = u32::from(frame.variant as u8);
        }
    }
    Ok(result)
}

/// Apply the original base variant only when no selected variant/gender
/// overrides it. The owned compatibility identifier clips to 39 bytes.
pub fn base_variant(
    selected: &[u8],
    gender: u8,
    base_gender: u8,
    variant: &[u8],
) -> Option<[u8; 40]> {
    if selected.contains(&b'+') || (gender != 0 && gender != base_gender) || variant.is_empty() {
        return None;
    }
    let mut output = [0; 40];
    for (target, byte) in output[..39]
        .iter_mut()
        .zip(selected.iter().chain([&b'+']).chain(variant))
    {
        *target = *byte;
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(name: &[u8], language: &[u8]) -> Frame {
        let mut f = Frame {
            kind: 2,
            variant: 0,
            gender: 0,
            age: 0,
            name: [0; 40],
            language: [0; 20],
        };
        copy(&mut f.name, name).unwrap();
        copy(&mut f.language, language).unwrap();
        f
    }
    #[test]
    fn ordered_resolution_aliases_inheritance_and_byte_fields() {
        let mut frames = [
            frame(b"known", b"en"),
            frame(b"unknown", b"fr"),
            frame(b"known", b""),
        ];
        frames[0].gender = 2;
        frames[0].age = 50;
        frames[0].variant = 7;
        frames[2].age = 257;
        frames[2].gender = -1;
        let mut calls = Vec::new();
        let mut id = [0; 40];
        copy(&mut id, b"gmw/en").unwrap();
        let result = choice(&frames, b"\x05en-gb\0\x08en\0\0", &[0; 40], |name| {
            calls.push(terminated(name).unwrap().to_vec());
            Ok((terminated(name)? == b"known").then_some(id))
        })
        .unwrap();
        assert_eq!(
            calls,
            [b"known".to_vec(), b"unknown".to_vec(), b"known".to_vec()]
        );
        assert_eq!(terminated(&result.name).unwrap(), b"known");
        assert_eq!(terminated(&result.identifier).unwrap(), b"gmw/en");
        assert_eq!(terminated(&result.language).unwrap(), b"");
        assert_eq!((result.gender, result.age, result.variant), (255, 1, 0));
        let alias = choice(&frames[..1], b"\x05en-gb\0\x08en\0\0", &[0; 40], |_| {
            Ok(Some(id))
        })
        .unwrap();
        assert_eq!(terminated(&alias.language).unwrap(), b"en-gb");
        assert_eq!(alias.variant, 7);
    }
    #[test]
    fn stale_identifier_variant_clipping_and_admission() {
        let mut stale = [0; 40];
        copy(&mut stale, b"previous").unwrap();
        let f = frame(b"missing", b"");
        let result = choice(&[f], b"\0", &stale, |_| Ok(None)).unwrap();
        assert_eq!(result.identifier, stale);
        let mut bad = f;
        bad.name = [b'x'; 40];
        assert_eq!(
            choice(&[f, bad], b"\0", &stale, |_| panic!("must preflight")),
            Err(Error::Bounds)
        );
        assert_eq!(
            choice(&[f], b"\x05en", &stale, |_| panic!("must preflight")),
            Err(Error::Bounds)
        );
        let id = base_variant(b"en", 0, 1, b"m2").unwrap();
        assert_eq!(terminated(&id).unwrap(), b"en+m2");
        assert!(base_variant(b"en+m1", 0, 1, b"m2").is_none());
        assert!(base_variant(b"en", 2, 1, b"m2").is_none());
        let long = base_variant(&[b'e'; 39], 1, 1, b"m2").unwrap();
        assert_eq!(&long[..39], &[b'e'; 39]);
        assert_eq!(long[39], 0);
    }
}
