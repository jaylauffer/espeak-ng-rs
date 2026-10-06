//! SSML marker/audio requests and output effects without backend execution.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ssml::{self, Attribute, Wide};
use std::fmt::Write;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Tag,
    Capacity,
    Effect,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Request {
    pub kind: i32,
    pub present: u32,
    pub name: [u8; 160],
}
impl Request {
    pub fn name(&self) -> Result<&[u8], Error> {
        if !matches!(self.kind, 1 | 5 | 11) || self.present > 1 {
            return Err(Error::Effect);
        }
        let end = self
            .name
            .iter()
            .position(|b| *b == 0)
            .ok_or(Error::Bounds)?;
        Ok(&self.name[..end])
    }
}
pub fn request(
    kind: i32,
    input: Wide<'_>,
    start: usize,
    wide_space: impl Fn(u32) -> bool,
    byte_space: impl Fn(u32) -> bool,
) -> Result<Request, Error> {
    let name = match kind {
        1 => b"xml:base".as_slice(),
        5 => b"name",
        11 => b"src",
        _ => return Err(Error::Tag),
    };
    if input.len() > crate::ssml_control::TAG_UNITS
        || start == 0
        || start >= input.len()
        || !(start..input.len()).any(|i| input.get(i) == Some(0))
    {
        return Err(Error::Bounds);
    }
    let mut result = Request {
        kind,
        present: 0,
        name: [0; 160],
    };
    if let Some(value) = ssml::attribute(input, start, name, wide_space).unwrap_or(None) {
        let (input, preceding) = match value {
            Attribute::Value(index) => (
                input.tail(index).ok_or(Error::Bounds)?,
                input.get(index - 1).ok_or(Error::Bounds)?,
            ),
            Attribute::Empty => (Wide::U32(&[0]), 0),
        };
        ssml::copy_plan(input, preceding, result.name.len(), byte_space)
            .map_err(|_| Error::Bounds)?
            .write(&mut result.name)
            .map_err(|_| Error::Capacity)?;
        result.present = 1;
    }
    Ok(result)
}
/// 0 absent, 1 clears the awaited marker without appending, 2 append name.
pub fn marker(request: &Request, skip: &[u8]) -> Result<u32, Error> {
    if request.kind != 5 {
        return Err(Error::Tag);
    }
    if skip.len() >= 50 || skip.contains(&0) {
        return Err(Error::Bounds);
    }
    let name = request.name()?;
    Ok(if request.present == 0 {
        0
    } else if !name.is_empty() && name == skip {
        1
    } else {
        2
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Path {
    pub length: u32,
    pub bytes: [u8; 256],
}
pub fn file(request: &Request, base: Option<&[u8]>) -> Result<Path, Error> {
    if request.kind != 11 || request.present != 1 {
        return Err(Error::Tag);
    }
    let name = request.name()?;
    let base = if name.first() != Some(&b'/') {
        base
    } else {
        None
    };
    let mut result = Path {
        length: 0,
        bytes: [0; 256],
    };
    let start = if let Some(base) = base {
        if base.contains(&0) || base.len() > 254 {
            return Err(Error::Bounds);
        }
        result.bytes[..base.len()].copy_from_slice(base);
        result.bytes[base.len()] = b'/';
        base.len() + 1
    } else {
        0
    };
    let end = start
        .checked_add(name.len())
        .filter(|end| *end < 256)
        .ok_or(Error::Capacity)?;
    result.bytes[start..end].copy_from_slice(name);
    result.length = end as u32;
    Ok(result)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Signal {
    pub length: u32,
    pub silence: u32,
    pub bytes: [u8; 16],
}
impl Write for Signal {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let start = self.length as usize;
        let end = start.checked_add(text.len()).ok_or(std::fmt::Error)?;
        self.bytes
            .get_mut(start..end)
            .ok_or(std::fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end as u32;
        Ok(())
    }
}
impl Signal {
    pub fn write(&self, output: &mut [u8]) -> Result<usize, Error> {
        if self.length == 0 {
            return Ok(0);
        }
        let length = self.length as usize;
        if length >= self.bytes.len() || length >= output.len() {
            return Err(Error::Capacity);
        }
        output[..=length].copy_from_slice(&self.bytes[..=length]);
        Ok(length)
    }
}
/// type1 mark, 2 loaded sound, 3 accepted URI. Failed index emits nothing.
pub fn signal(kind: u32, index: i32) -> Result<Signal, Error> {
    let suffix = match kind {
        1 => 'M',
        2 => 'I',
        3 => 'U',
        _ => return Err(Error::Tag),
    };
    let mut result = Signal {
        length: 0,
        silence: 0,
        bytes: [0; 16],
    };
    if index >= 0 {
        write!(&mut result, "\u{1}{index}{suffix}").map_err(|_| Error::Capacity)?;
        result.silence = u32::from(kind != 1);
    }
    Ok(result)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Audio {
    pub push: u32,
    pub pop: u32,
    pub text: u32,
    pub terminator: i32,
}
/// text0 false/1 true/2 preserve. Execute push, backend request, parameter
/// merge, then optional pop/text publication; no owner borrow crosses backend.
pub fn audio(kind: i32, self_closing: bool) -> Result<Audio, Error> {
    Ok(match kind {
        11 => Audio {
            push: 1,
            pop: u32::from(self_closing),
            text: if self_closing { 2 } else { 1 },
            terminator: crate::ssml_clause::NONE,
        },
        43 => Audio {
            push: 0,
            pop: 1,
            text: 0,
            terminator: crate::ssml_clause::NONE,
        },
        _ => return Err(Error::Tag),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_retain_empty_base_slash_precedence_and_admit_complete_capacity() {
        let mut r = Request {
            kind: 11,
            present: 1,
            name: [0; 160],
        };
        r.name[..8].copy_from_slice(b"tone.wav");
        assert_eq!(&file(&r, Some(b"")).unwrap().bytes[..10], b"/tone.wav\0");
        assert_eq!(&file(&r, None).unwrap().bytes[..9], b"tone.wav\0");
        assert!(file(&r, Some(&[b'a'; 250])).is_err());
        r.name[..10].copy_from_slice(b"/tone.wav\0");
        assert_eq!(
            &file(&r, Some(&[b'a'; 300])).unwrap().bytes[..10],
            b"/tone.wav\0"
        );
    }
    #[test]
    fn markers_and_audio_publish_explicit_ordered_effects() {
        let mut r = Request {
            kind: 5,
            present: 1,
            name: [0; 160],
        };
        r.name[..6].copy_from_slice(b"marker");
        assert_eq!(marker(&r, b"marker"), Ok(1));
        assert_eq!(marker(&r, b"other"), Ok(2));
        r.name[0] = 0;
        assert_eq!(marker(&r, b""), Ok(2));
        let signal = signal(3, i32::MAX).unwrap();
        let mut output = [0xa5; 20];
        assert_eq!(signal.write(&mut output), Ok(12));
        assert_eq!(&output[..13], b"\x012147483647U\0");
        assert_eq!(output[13], 0xa5);
        let before = output;
        assert!(signal.write(&mut output[..12]).is_err());
        assert_eq!(output, before);
        assert_eq!(audio(11, true).unwrap().text, 2);
        assert_eq!(audio(43, false).unwrap().push, 0);
    }
}
