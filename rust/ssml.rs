//! Bounded SSML attribute and character-reference helpers.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

/// Compatibility wide characters are code units, including on Windows.
#[derive(Clone, Copy)]
pub enum Wide<'a> {
    U16(&'a [u16]),
    U32(&'a [u32]),
}
impl Wide<'_> {
    pub fn len(self) -> usize {
        match self {
            Self::U16(s) => s.len(),
            Self::U32(s) => s.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn get(self, index: usize) -> Option<u32> {
        match self {
            Self::U16(s) => s.get(index).map(|c| u32::from(*c)),
            Self::U32(s) => s.get(index).copied(),
        }
    }
    pub fn tail(self, start: usize) -> Option<Self> {
        match self {
            Self::U16(s) => s.get(start..).map(Self::U16),
            Self::U32(s) => s.get(start..).map(Self::U32),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Arithmetic,
}

/// Matching requires a closing quote, as in the original attribute mnemonics.
pub fn attribute_matches(input: Option<Wide<'_>>, name: &[u8]) -> bool {
    let Some(input) = input else { return false };
    for (index, byte) in name.iter().enumerate() {
        if input.get(index) != Some(u32::from(*byte)) {
            return false;
        }
    }
    matches!(input.get(name.len()), Some(34 | 39))
}

pub fn attribute_number(input: Option<Wide<'_>>, default: i32, time: bool) -> Result<i32, Error> {
    let Some(input) = input else {
        return Ok(default);
    };
    if !matches!(input.get(0), Some(48..=57)) {
        return Ok(default);
    }
    let mut value = 0i32;
    let mut index = 0;
    while let Some(c @ 48..=57) = input.get(index) {
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add((c - 48) as i32))
            .ok_or(Error::Arithmetic)?;
        index += 1;
    }
    let next = input.get(index).ok_or(Error::Bounds)?;
    if time && crate::unicode::to_lower(next) == u32::from(b's') {
        value = value.checked_mul(1000).ok_or(Error::Arithmetic)?;
    }
    Ok(value)
}

/// Index of the borrowed value, or absence. `start` includes a preceding unit
/// in the span. Whitespace classification is supplied by the host/locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    Value(usize),
    Empty,
}

pub fn attribute(
    input: Wide<'_>,
    start: usize,
    name: &[u8],
    space: impl Fn(u32) -> bool,
) -> Result<Option<Attribute>, Error> {
    if start == 0 || start >= input.len() || name.contains(&0) {
        return Err(Error::Bounds);
    }
    let at = |index| input.get(index).ok_or(Error::Bounds);
    let mut pos = start;
    while at(pos)? != 0 {
        if space(at(pos - 1)?) {
            let mut ix = 0;
            // Keep the legacy scan position even after a partial name match.
            while ix < name.len() && at(pos)? == u32::from(name[ix]) {
                pos += 1;
                ix += 1;
            }
            if ix == name.len() {
                while space(at(pos)?) {
                    pos += 1;
                }
                if at(pos)? == u32::from(b'=') {
                    pos += 1;
                }
                while space(at(pos)?) {
                    pos += 1;
                }
                if matches!(at(pos)?, 34 | 39) {
                    at(pos + 1)?;
                    return Ok(Some(Attribute::Value(pos + 1)));
                }
                return Ok(Some(if at(pos)? == 47 {
                    Attribute::Empty
                } else {
                    Attribute::Value(pos)
                }));
            }
        }
        pos += 1;
    }
    Ok(None)
}

/// A validated copy plan retains the source, avoiding whole-tag allocation.
pub struct CopyPlan<'a> {
    input: Wide<'a>,
    units: usize,
    length: usize,
}
impl CopyPlan<'_> {
    pub fn length(&self) -> usize {
        self.length
    }
    pub(crate) fn emit_content(&self, mut write: impl FnMut(&[u8])) {
        for ix in 0..self.units {
            let (bytes, length) =
                crate::suffix::encode(self.input.get(ix).expect("validated unit"));
            write(&bytes[..length]);
        }
    }
    pub(crate) fn emit(&self, mut write: impl FnMut(&[u8])) {
        self.emit_content(&mut write);
        write(&[0]);
    }
    pub fn write(&self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() <= self.length {
            return Err(Error::Capacity);
        }
        let mut offset = 0;
        self.emit(|bytes| {
            output[offset..offset + bytes.len()].copy_from_slice(bytes);
            offset += bytes.len();
        });
        Ok(self.length)
    }
}

pub fn copy_plan(
    input: Wide<'_>,
    preceding: u32,
    capacity: usize,
    byte_space: impl Fn(u32) -> bool,
) -> Result<CopyPlan<'_>, Error> {
    if capacity == 0 || capacity > isize::MAX as usize {
        return Err(Error::Capacity);
    }
    let quote = if matches!(preceding, 34 | 39) {
        preceding
    } else {
        0
    };
    let mut units = 0;
    let mut length = 0usize;
    let mut previous = 0;
    while length < capacity.saturating_sub(4) {
        let c = input.get(units).ok_or(Error::Bounds)?;
        if c == 0
            || (quote == 0 && ((c <= 255 && byte_space(c)) || c == 47))
            || (quote != 0 && c == quote && previous != 92)
        {
            break;
        }
        length += crate::suffix::encode(c).1;
        previous = c;
        units += 1;
    }
    Ok(CopyPlan {
        input,
        units,
        length,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Reference {
    pub status: i32,
    pub first: i32,
    pub second: i32,
}

/// `scanf`'s partial-number and zero-conversion behavior is compatibility data.
/// Arithmetic outside the defined destination range is rejected.
pub fn reference(
    input: &[u8],
    first: i32,
    second: i32,
    space: impl Fn(u32) -> bool,
) -> Result<Reference, Error> {
    let mut result = Reference {
        status: -1,
        first,
        second,
    };
    if input.first() != Some(&b'#') {
        if let Some(value) = match input {
            b"gt" => Some(62),
            b"lt" => Some(0xe03c),
            b"amp" => Some(38),
            b"quot" => Some(34),
            b"nbsp" => Some(32),
            b"apos" => Some(39),
            _ => None,
        } {
            result.first = value;
            result.status = value;
            if second == 0 {
                result.second = 32;
            }
        }
        return Ok(result);
    }
    let hex = input.get(1) == Some(&b'x');
    let mut pos = if hex { 2 } else { 1 };
    while input.get(pos).is_some_and(|c| space(u32::from(*c))) {
        pos += 1;
    }
    if pos == input.len() {
        return Ok(result);
    }
    result.status = 0;
    let negative = input.get(pos) == Some(&b'-');
    if matches!(input.get(pos), Some(b'+' | b'-')) {
        pos += 1;
    }
    let base = if hex { 16 } else { 10 };
    let mut digits = 0;
    let mut value = 0u32;
    if hex && input.get(pos) == Some(&b'0') && matches!(input.get(pos + 1), Some(b'x' | b'X')) {
        pos += 2;
        // scanf accepts the leading zero even if the prefix has no hex digit.
        digits = 1;
    }
    while let Some(&c) = input.get(pos) {
        let digit = match c {
            b'0'..=b'9' => u32::from(c - b'0'),
            b'a'..=b'f' => u32::from(c - b'a') + 10,
            b'A'..=b'F' => u32::from(c - b'A') + 10,
            _ => break,
        };
        if digit >= base {
            break;
        }
        value = value
            .checked_mul(base)
            .and_then(|v| v.checked_add(digit))
            .ok_or(Error::Arithmetic)?;
        pos += 1;
        digits += 1;
    }
    if digits != 0 {
        result.first = if hex {
            (if negative {
                value.wrapping_neg()
            } else {
                value
            }) as i32
        } else if negative {
            i32::try_from(-(i64::from(value))).map_err(|_| Error::Arithmetic)?
        } else {
            i32::try_from(value).map_err(|_| Error::Arithmetic)?
        };
        result.status = 1;
    }
    Ok(result)
}

pub fn key_name(input: &[u8]) -> Option<([u8; 4], usize, i32)> {
    let code = match input {
        b"space " => 0xe020,
        b"tab " => 0xe009,
        b"underscore " => 0xe05f,
        b"double-quote " => 34,
        _ => return None,
    };
    let (bytes, length) = crate::suffix::encode(code);
    Some((bytes, length, code as i32))
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
    #[test]
    fn attributes_retain_partial_matches_quotes_and_capacity() {
        let text = wide(" nameSuffix='x' name='a\\'b' /");
        let Attribute::Value(pos) = attribute(Wide::U32(&text), 1, b"name", space)
            .unwrap()
            .unwrap()
        else {
            panic!("value")
        };
        assert_eq!(text[pos], u32::from(b'S'));
        assert_eq!(
            attribute(Wide::U32(&wide(" name=/")), 1, b"name", space),
            Ok(Some(Attribute::Empty))
        );
        assert_eq!(
            attribute(Wide::U32(&wide(" name='/x'")), 1, b"name", space),
            Ok(Some(Attribute::Value(7)))
        );
        assert_eq!(
            attribute(Wide::U32(&wide(" name")), 1, b"name", space),
            Ok(Some(Attribute::Value(5)))
        );
        let source = wide("a\\'b'junk");
        let plan = copy_plan(Wide::U32(&source), 39, 40, space).unwrap();
        let mut out = [0xa5; 40];
        assert_eq!(plan.write(&mut out), Ok(4));
        assert_eq!(&out[..6], b"a\\'b\0\xa5");
        assert_eq!(plan.write(&mut [0; 4]), Err(Error::Capacity));
        assert!(attribute_matches(
            Some(Wide::U32(&wide("male'ignored"))),
            b"male"
        ));
        assert!(!attribute_matches(Some(Wide::U32(&wide("male"))), b"male"));
        assert_eq!(
            attribute_number(Some(Wide::U32(&wide("2147483648"))), 7, false),
            Err(Error::Arithmetic)
        );
        assert_eq!(
            attribute_number(Some(Wide::U32(&wide("4S"))), 7, true),
            Ok(4000)
        );
        assert!(matches!(
            copy_plan(Wide::U32(&[65]), 34, 40, space),
            Err(Error::Bounds)
        ));
    }
    #[test]
    fn code_units_and_references_preserve_legacy_outputs() {
        let input = [0xd800u16, 39, 0];
        let mut out = [0xa5; 8];
        assert_eq!(
            copy_plan(Wide::U16(&input), 39, 8, space)
                .unwrap()
                .write(&mut out),
            Ok(3)
        );
        assert_eq!(&out[..5], &[0xed, 0xa0, 0x80, 0, 0xa5]);
        assert_eq!(
            reference(b"#x-ffffffff!", 7, 8, space).unwrap(),
            Reference {
                status: 1,
                first: 1,
                second: 8
            }
        );
        assert_eq!(
            reference(b"#bad", 7, 8, space).unwrap(),
            Reference {
                status: 0,
                first: 7,
                second: 8
            }
        );
        assert_eq!(reference(b"#  ", 7, 8, space).unwrap().status, -1);
        assert_eq!(
            reference(b"lt", 7, 0, space).unwrap(),
            Reference {
                status: 0xe03c,
                first: 0xe03c,
                second: 32
            }
        );
        assert_eq!(
            reference(b"#2147483648", 7, 8, space),
            Err(Error::Arithmetic)
        );
        assert_eq!(key_name(b"space ").unwrap().2, 0xe020);
        assert_eq!(key_name(b"space"), None);
    }
}
