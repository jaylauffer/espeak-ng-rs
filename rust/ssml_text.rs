//! Transactional SSML text and say-as output plans.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ssml::{self, Attribute, CopyPlan, Wide};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct State {
    pub offset: i32,
    pub mode: i32,
    pub start: i32,
    pub ignore: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Arithmetic,
    Tag,
}
struct Command {
    bytes: [u8; 16],
    length: usize,
}
impl Write for Command {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.length.checked_add(s.len()).ok_or(std::fmt::Error)?;
        let destination = self
            .bytes
            .get_mut(self.length..end)
            .ok_or(std::fmt::Error)?;
        destination.copy_from_slice(s.as_bytes());
        self.length = end;
        Ok(())
    }
}
enum Output<'a> {
    None,
    Phoneme(CopyPlan<'a>),
    Alias(CopyPlan<'a>),
    Open(Command),
    Close {
        key: Option<([u8; 4], usize)>,
        old_nul: Option<usize>,
    },
}
pub struct Plan<'a> {
    pub state: State,
    offset: usize,
    output: Output<'a>,
}
impl Plan<'_> {
    /// Emit only admitted writes, including the historical old-end NUL when
    /// key substitution shortens the logical output. Unused tail is untouched.
    pub(crate) fn emit(&self, mut write: impl FnMut(usize, &[u8])) {
        let mut offset = self.offset;
        match &self.output {
            Output::None => {}
            Output::Phoneme(copy) => {
                write(offset, b"[[");
                offset += 2;
                copy.emit_content(|bytes| {
                    write(offset, bytes);
                    offset += bytes.len();
                });
                write(offset, b"]]");
            }
            Output::Alias(copy) => {
                copy.emit(|bytes| {
                    write(offset, bytes);
                    offset += bytes.len();
                });
            }
            Output::Open(command) => write(offset, &command.bytes[..=command.length]),
            Output::Close { key, old_nul } => {
                if let Some(index) = old_nul {
                    write(*index, &[0]);
                }
                if let Some((bytes, length)) = key {
                    offset = self.state.start as usize;
                    write(offset, &bytes[..*length]);
                    offset += length;
                }
                write(offset, &[1, b'Y']);
            }
        }
    }
    pub fn write(&self, output: &mut [u8]) -> Result<State, Error> {
        let mut maximum = 0;
        self.emit(|index, bytes| maximum = maximum.max(index + bytes.len()));
        if maximum > output.len() {
            return Err(Error::Capacity);
        }
        self.emit(|index, bytes| output[index..index + bytes.len()].copy_from_slice(bytes));
        Ok(self.state)
    }
}

pub struct Request<'a, 'b> {
    pub kind: i32,
    pub input: Wide<'a>,
    pub start: usize,
    pub prefix: &'b [u8],
    pub capacity: usize,
    pub state: State,
}
pub fn plan<'a>(
    request: Request<'a, '_>,
    wide_space: impl Fn(u32) -> bool,
    byte_space: impl Fn(u32) -> bool,
) -> Result<Plan<'a>, Error> {
    let Request {
        kind,
        input,
        start,
        prefix,
        capacity,
        mut state,
    } = request;
    if input.len() > crate::ssml_control::TAG_UNITS
        || start == 0
        || start >= input.len()
        || !(start..input.len()).any(|i| input.get(i) == Some(0))
        || state.ignore > 1
        || usize::try_from(state.offset).ok() != Some(prefix.len())
        || capacity < prefix.len()
        || capacity > i32::MAX as usize
    {
        return Err(Error::Bounds);
    }
    let offset = prefix.len();
    let remaining = capacity - offset;
    let find = |name: &[u8]| ssml::attribute(input, start, name, &wide_space).unwrap_or(None);
    let value = |attr| match attr {
        Some(Attribute::Value(index)) => input.tail(index),
        _ => Some(Wide::U32(&[0])),
    };
    let copy = |attr, capacity| {
        let (text, preceding) = match attr {
            Some(Attribute::Value(index)) => (
                input.tail(index).ok_or(Error::Bounds)?,
                input.get(index - 1).ok_or(Error::Bounds)?,
            ),
            _ => (Wide::U32(&[0]), 0),
        };
        ssml::copy_plan(text, preceding, capacity, &byte_space).map_err(|_| Error::Capacity)
    };
    let (output, length) = match kind {
        8 => {
            if ssml::attribute_matches(value(find(b"alphabet")), b"espeak") {
                if remaining < 4 {
                    return Err(Error::Capacity);
                }
                let copy = copy(find(b"ph"), remaining - 2)?;
                let length = copy.length() + 4;
                if length > remaining {
                    return Err(Error::Capacity);
                }
                (Output::Phoneme(copy), length)
            } else {
                (Output::None, 0)
            }
        }
        4 => {
            let interpret = value(find(b"interpret-as"));
            let mut mode = [
                (b"characters".as_slice(), 18),
                (b"tts:char", 20),
                (b"tts:key", 36),
                (b"tts:digits", 64),
                (b"telephone", 193),
            ]
            .iter()
            .find(|(name, _)| ssml::attribute_matches(interpret, name))
            .map_or(-1, |(_, mode)| *mode);
            if ssml::attribute_matches(value(find(b"format")), b"glyphs") {
                mode = 19;
            }
            let detail = ssml::attribute_number(value(find(b"detail")), 0, false)
                .map_err(|_| Error::Arithmetic)?;
            if mode == 64 {
                mode = if detail <= 1 {
                    193
                } else {
                    64i32.checked_add(detail).ok_or(Error::Arithmetic)?
                };
            }
            let mut command = Command {
                bytes: [0; 16],
                length: 0,
            };
            write!(&mut command, "\u{1}{mode}Y").map_err(|_| Error::Capacity)?;
            if command.length >= remaining {
                return Err(Error::Capacity);
            }
            state.mode = mode;
            state.start = (offset + command.length) as i32;
            let length = command.length;
            (Output::Open(command), length)
        }
        36 => {
            let old_nul = if state.mode == 36 {
                if remaining == 0 {
                    return Err(Error::Capacity);
                }
                Some(offset)
            } else {
                None
            };
            let key = if state.mode == 36 {
                let begin = usize::try_from(state.start).map_err(|_| Error::Bounds)?;
                let text = prefix.get(begin..).ok_or(Error::Bounds)?;
                // C lookup stops at the first embedded NUL, if one exists.
                let end = text.iter().position(|c| *c == 0).unwrap_or(text.len());
                ssml::key_name(&text[..end]).map(|(bytes, length, _)| (bytes, length))
            } else {
                None
            };
            let next = key
                .as_ref()
                .map_or(offset, |(_, length)| state.start as usize + length);
            let end = next.checked_add(2).ok_or(Error::Capacity)?;
            if end > capacity {
                return Err(Error::Capacity);
            }
            state.mode = 0;
            state.offset = end as i32;
            return Ok(Plan {
                state,
                offset,
                output: Output::Close { key, old_nul },
            });
        }
        9 => {
            if let Some(attr) = find(b"alias") {
                let copy = copy(Some(attr), remaining)?;
                let length = copy.length();
                state.ignore = 1;
                (Output::Alias(copy), length)
            } else {
                (Output::None, 0)
            }
        }
        14 => {
            state.ignore = 1;
            (Output::None, 0)
        }
        41 | 46 => {
            state.ignore = 0;
            (Output::None, 0)
        }
        _ => return Err(Error::Tag),
    };
    state.offset = (offset + length) as i32;
    Ok(Plan {
        state,
        offset,
        output,
    })
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
    fn request<'a, 'b>(
        kind: i32,
        xml: &'a [u32],
        prefix: &'b [u8],
        capacity: usize,
        state: State,
    ) -> Request<'a, 'b> {
        Request {
            kind,
            input: Wide::U32(xml),
            start: 1,
            prefix,
            capacity,
            state,
        }
    }
    #[test]
    fn key_close_preserves_old_end_nul_and_intervening_tail() {
        let xml = wide(" ");
        let mut out = [0xa5; 20];
        out[..9].copy_from_slice(b"abcspace ");
        let state = State {
            offset: 9,
            mode: 36,
            start: 3,
            ignore: 0,
        };
        let p = plan(request(36, &xml, &out[..9], 20, state), space, space).unwrap();
        let result = p.write(&mut out).unwrap();
        assert_eq!((result.offset, result.mode), (8, 0));
        assert_eq!(
            &out[..10],
            &[b'a', b'b', b'c', 0xee, 0x80, 0xa0, 1, b'Y', b' ', 0]
        );
        assert_eq!(out[10], 0xa5);
        assert!(plan(request(36, &xml, &out[..9], 9, state), space, space).is_err());
    }
    #[test]
    fn output_admission_and_numeric_errors_leave_buffers_unchanged() {
        let xml = wide(" alphabet='espeak' ph='abcdef'");
        let state = State {
            offset: 0,
            mode: 0,
            start: -1,
            ignore: 0,
        };
        let p = plan(request(8, &xml, &[], 8, state), space, space).unwrap();
        let mut out = [0xa5; 8];
        p.write(&mut out).unwrap();
        assert_eq!(&out, b"[[ab]]\xa5\xa5");
        let before = out;
        assert!(p.write(&mut out[..5]).is_err());
        assert_eq!(out, before);
        assert!(plan(request(8, &xml, &[], 3, state), space, space).is_err());
        let xml = wide(" alphabet='espeak' ph='αβ界😀'");
        assert!(matches!(
            plan(request(8, &xml, &[], 14, state), space, space),
            Err(Error::Capacity)
        ));
        let xml = wide(" interpret-as='tts:digits' detail='2147483647'");
        assert!(matches!(
            plan(request(4, &xml, &[], 30, state), space, space),
            Err(Error::Arithmetic)
        ));
    }
}
