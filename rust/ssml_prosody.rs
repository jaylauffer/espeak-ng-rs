//! Bounded SSML floating values and ordered prosody parameter effects.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ssml::{attribute_matches, Wide};
pub const NUMBER_CAPACITY: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Arithmetic,
    Parameter,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Value {
    pub kind: i32,
    pub value: i32,
}
fn digit(c: u32) -> Option<u8> {
    match c {
        48..=57 => Some((c - 48) as u8),
        65..=70 => Some((c - 65 + 10) as u8),
        97..=102 => Some((c - 97 + 10) as u8),
        _ => None,
    }
}
fn cast(value: f64) -> Result<i32, Error> {
    if !value.is_finite()
        || value.trunc() < f64::from(i32::MIN)
        || value.trunc() > f64::from(i32::MAX)
    {
        return Err(Error::Arithmetic);
    }
    Ok(value as i32)
}

// Round a hexadecimal significand directly to IEEE binary64, with guard/sticky
// bits and ties-to-even, including subnormals. No accumulating floating error.
fn hex_float(bytes: &[u8], exponent: i64, fractional: usize, negative: bool) -> f64 {
    let bits = || {
        bytes
            .iter()
            .filter(|b| **b != b'.')
            .flat_map(|c| {
                let d = digit(u32::from(*c)).expect("validated hexadecimal digit");
                (0..4).rev().map(move |bit| (d >> bit) & 1)
            })
            .skip_while(|b| *b == 0)
    };
    let length = bits().count() as i64;
    let sign = u64::from(negative) << 63;
    if length == 0 {
        return f64::from_bits(sign);
    }
    let scale = exponent - 4 * fractional as i64;
    let mut power = length - 1 + scale;
    if power > 1023 {
        return f64::from_bits(sign | (0x7ff << 52));
    }
    let keep = if power >= -1022 {
        53
    } else {
        length + scale + 1074
    };
    if keep < 0 {
        return f64::from_bits(sign);
    }
    let mut significand = 0u64;
    let mut guard = false;
    let mut sticky = false;
    for (index, bit) in bits().enumerate() {
        let index = index as i64;
        if index < keep {
            significand = (significand << 1) | u64::from(bit);
        } else if index == keep {
            guard = bit != 0;
        } else {
            sticky |= bit != 0;
        }
    }
    if keep > length {
        significand <<= keep - length;
    }
    if guard && (sticky || significand & 1 != 0) {
        significand += 1;
    }
    if power < -1022 {
        return f64::from_bits(sign | significand);
    }
    if significand == 1 << 53 {
        significand >>= 1;
        power += 1;
    }
    f64::from_bits(sign | (((power + 1023) as u64) << 52) | (significand & ((1 << 52) - 1)))
}

/// Native equivalent of the defined finite `wcstod` grammar. Decimal separator
/// and whitespace come from the host locale; arithmetic uses binary64.
pub fn number(
    input: Wide<'_>,
    start: usize,
    decimal: u32,
    space: impl Fn(u32) -> bool,
) -> Result<Option<(f64, usize)>, Error> {
    if input.len() > NUMBER_CAPACITY + 1 {
        return Err(Error::Capacity);
    }
    let at = |pos| input.get(pos).ok_or(Error::Bounds);
    let mut pos = start;
    while space(at(pos)?) {
        pos += 1;
    }
    let begin = pos;
    let negative = at(pos)? == 45;
    if matches!(at(pos)?, 43 | 45) {
        pos += 1;
    }
    let digits_begin = pos;
    let ascii_lower = |c: u32| if (65..=90).contains(&c) { c + 32 } else { c };
    let keyword = |name: &[u8]| {
        name.iter()
            .enumerate()
            .all(|(i, c)| input.get(pos + i).map(ascii_lower) == Some(u32::from(*c)))
    };
    // Their eventual C-to-int conversions were undefined. Return a non-finite
    // value so the effect planner rejects them without publishing a parameter.
    if keyword(b"inf") {
        return Ok(Some((
            if negative {
                -f64::INFINITY
            } else {
                f64::INFINITY
            },
            pos + if keyword(b"infinity") { 8 } else { 3 },
        )));
    }
    if keyword(b"nan") {
        let mut tail = pos + 3;
        if input.get(tail) == Some(40) {
            let mut end = tail + 1;
            while matches!(input.get(end), Some(48..=57 | 65..=90 | 95 | 97..=122)) {
                end += 1;
            }
            if input.get(end) == Some(41) {
                tail = end + 1;
            }
        }
        return Ok(Some((f64::NAN, tail)));
    }
    let mut bytes = [0u8; NUMBER_CAPACITY];
    let mut length = 0;
    let mut put = |byte| {
        if length == NUMBER_CAPACITY {
            return Err(Error::Capacity);
        }
        bytes[length] = byte;
        length += 1;
        Ok(())
    };
    let hexadecimal = at(pos)? == 48 && matches!(input.get(pos + 1), Some(88 | 120));
    if hexadecimal {
        pos += 2;
        let mut digits = 0;
        let mut fractional = 0;
        while digit(at(pos)?).is_some() {
            put(at(pos)? as u8)?;
            pos += 1;
            digits += 1;
        }
        if at(pos)? == decimal {
            put(b'.')?;
            pos += 1;
            while digit(at(pos)?).is_some() {
                put(at(pos)? as u8)?;
                pos += 1;
                digits += 1;
                fractional += 1;
            }
        }
        if digits != 0 {
            let mut exponent = 0i64;
            if matches!(at(pos)?, 80 | 112) {
                let marker = pos;
                pos += 1;
                let minus = at(pos)? == 45;
                if matches!(at(pos)?, 43 | 45) {
                    pos += 1;
                }
                let first = pos;
                while let c @ 48..=57 = at(pos)? {
                    exponent = (exponent * 10 + i64::from(c - 48)).min(1_000_000);
                    pos += 1;
                }
                if pos == first {
                    pos = marker;
                    exponent = 0;
                } else if minus {
                    exponent = -exponent;
                }
            }
            return Ok(Some((
                hex_float(&bytes[..length], exponent, fractional, negative),
                pos,
            )));
        }
        // A malformed hexadecimal prefix still consumes its decimal zero.
        return Ok(Some((if negative { -0.0 } else { 0.0 }, digits_begin + 1)));
    }
    if negative {
        put(b'-')?;
    }
    let mut digits = 0;
    while let c @ 48..=57 = at(pos)? {
        put(c as u8)?;
        pos += 1;
        digits += 1;
    }
    if at(pos)? == decimal {
        put(b'.')?;
        pos += 1;
        while let c @ 48..=57 = at(pos)? {
            put(c as u8)?;
            pos += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return Ok(None);
    }
    if matches!(at(pos)?, 69 | 101) {
        let marker = pos;
        pos += 1;
        let sign = at(pos)?;
        if matches!(sign, 43 | 45) {
            pos += 1;
        }
        let first = pos;
        while matches!(at(pos)?, 48..=57) {
            pos += 1;
        }
        if pos == first {
            pos = marker;
        } else {
            put(b'e')?;
            if sign == 45 {
                put(b'-')?;
            }
            for ix in first..pos {
                put(at(ix)? as u8)?;
            }
        }
    }
    let text = std::str::from_utf8(&bytes[..length]).expect("ASCII numeric grammar");
    let value = text.parse::<f64>().map_err(|_| Error::Arithmetic)?;
    debug_assert!(pos > begin);
    Ok(Some((value, pos)))
}

pub fn value(
    param: i32,
    input: Wide<'_>,
    decimal: u32,
    space: impl Fn(u32) -> bool,
) -> Result<Value, Error> {
    if input.len() > NUMBER_CAPACITY + 1 {
        return Err(Error::Capacity);
    }
    let at = |pos| input.get(pos).ok_or(Error::Bounds);
    let mut pos = 0;
    while space(at(pos)?) {
        pos += 1;
    }
    let mut sign = 0;
    if at(pos)? == 43 {
        pos += 1;
        sign = 1;
    }
    if at(pos)? == 45 {
        pos += 1;
        sign = -1;
    }
    let Some((value, tail)) = number(input, pos, decimal, space)? else {
        return Ok(Value {
            kind: 2,
            value: 100,
        });
    };
    if !value.is_finite() {
        return Err(Error::Arithmetic);
    }
    if at(tail)? == 37 {
        let value = if sign != 0 {
            100.0 + f64::from(sign) * value
        } else {
            value
        };
        return Ok(Value {
            kind: 2,
            value: cast(value)?,
        });
    }
    if at(tail)? == 115 && input.get(tail + 1) == Some(116) {
        let x = 2.0f64.powf((value * f64::from(sign)) / 12.0) * 100.0;
        return Ok(Value {
            kind: 2,
            value: cast(x)?,
        });
    }
    if param == 1 {
        let value = if sign == 0 {
            cast(value * 100.0)?
        } else {
            100i32
                .checked_add(cast(f64::from(sign) * value * 100.0)?)
                .ok_or(Error::Arithmetic)?
        };
        return Ok(Value { kind: 2, value });
    }
    Ok(Value {
        kind: sign,
        value: cast(value)?,
    })
}

pub fn parameter(
    param: usize,
    input: Wide<'_>,
    base: i32,
    current: i32,
    decimal: u32,
    space: impl Fn(u32) -> bool,
) -> Result<i32, Error> {
    const NAMES: [&[u8]; 7] = [
        b"default", b"x-soft", b"soft", b"medium", b"loud", b"x-loud", b"silent",
    ];
    const RATE: [(&[u8], i32); 6] = [
        (b"default", 100),
        (b"x-slow", 60),
        (b"slow", 80),
        (b"medium", 100),
        (b"fast", 125),
        (b"x-fast", 160),
    ];
    const PITCH: [(&[u8], i32); 6] = [
        (b"default", 100),
        (b"x-low", 70),
        (b"low", 85),
        (b"medium", 100),
        (b"high", 110),
        (b"x-high", 120),
    ];
    const RANGE: [(&[u8], i32); 6] = [
        (b"default", 100),
        (b"x-low", 20),
        (b"low", 50),
        (b"medium", 100),
        (b"high", 140),
        (b"x-high", 180),
    ];
    const VOLUME: [i32; 7] = [100, 30, 65, 100, 150, 230, 0];
    let named = match param {
        1 => RATE
            .iter()
            .find(|(name, _)| attribute_matches(Some(input), name))
            .map(|(_, v)| *v),
        2 => NAMES
            .iter()
            .position(|name| attribute_matches(Some(input), name))
            .map(|i| VOLUME[i]),
        3 => PITCH
            .iter()
            .find(|(name, _)| attribute_matches(Some(input), name))
            .map(|(_, v)| *v),
        4 => RANGE
            .iter()
            .find(|(name, _)| attribute_matches(Some(input), name))
            .map(|(_, v)| *v),
        _ => return Err(Error::Parameter),
    };
    if let Some(value) = named {
        return base
            .checked_mul(value)
            .map(|v| v / 100)
            .ok_or(Error::Arithmetic);
    }
    let value = value(param as i32, input, decimal, space)?;
    match value.kind {
        0 => Ok(value.value),
        2 => current
            .checked_mul(value.value)
            .map(|v| v / 100)
            .ok_or(Error::Arithmetic),
        sign => value
            .value
            .checked_mul(sign)
            .and_then(|v| current.checked_add(v))
            .ok_or(Error::Arithmetic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(s: &str, param: i32) -> Result<Value, Error> {
        let units: Vec<u32> = s.chars().map(u32::from).chain([0]).collect();
        value(param, Wide::U32(&units), 46, |c| matches!(c, 9..=13 | 32))
    }
    #[test]
    fn prosody_preserves_sign_order_semitones_and_checked_casts() {
        assert_eq!(
            parse("+12st'", 3),
            Ok(Value {
                kind: 2,
                value: 200
            })
        );
        assert_eq!(
            parse("12st'", 3),
            Ok(Value {
                kind: 2,
                value: 100
            })
        );
        assert_eq!(parse("+-5.5%", 3), Ok(Value { kind: 2, value: 94 }));
        assert_eq!(
            parse("--5.5%", 3),
            Ok(Value {
                kind: 2,
                value: 105
            })
        );
        assert_eq!(
            parse("+0.259", 1),
            Ok(Value {
                kind: 2,
                value: 125
            })
        );
        assert_eq!(
            parse("nonsense", 3),
            Ok(Value {
                kind: 2,
                value: 100
            })
        );
        assert_eq!(parse("nan", 3), Err(Error::Arithmetic));
        assert_eq!(parse("2147483648", 3), Err(Error::Arithmetic));
        assert_eq!(parse("-2147483649", 3), Err(Error::Arithmetic));
        assert_eq!(parse("0x1.8p+2", 3), Ok(Value { kind: 0, value: 6 }));
        assert_eq!(parse("0xg", 3), Ok(Value { kind: 0, value: 0 }));
    }
    #[test]
    fn hexadecimal_rounding_locale_and_parameter_admission() {
        let parse = |s: &str| {
            let units: Vec<u32> = s.chars().map(u32::from).chain([0]).collect();
            number(Wide::U32(&units), 0, 46, |c| matches!(c, 9..=13 | 32))
                .unwrap()
                .unwrap()
                .0
        };
        assert_eq!(parse("0x1.00000000000008p0").to_bits(), 1.0f64.to_bits());
        assert_eq!(
            parse("0x1.00000000000018p0").to_bits(),
            1.0f64.to_bits() + 2
        );
        assert_eq!(parse("0x1p-1075").to_bits(), 0);
        assert_eq!(parse("0x1.8p-1075").to_bits(), 1);
        assert_eq!(parse("-0x0p+100").to_bits(), 1 << 63);
        for (text, expected) in [
            ("infinity!", 8),
            ("NAN(test)!", 9),
            ("nan()", 5),
            ("nan($)", 3),
        ] {
            let units: Vec<u32> = text.chars().map(u32::from).chain([0]).collect();
            assert_eq!(
                number(Wide::U32(&units), 0, 46, |_| false)
                    .unwrap()
                    .unwrap()
                    .1,
                expected
            );
        }
        let units = [49, 44, 53, 0];
        assert_eq!(
            number(Wide::U32(&units), 0, 44, |_| false).unwrap(),
            Some((1.5, 3))
        );
        assert_eq!(
            parameter(
                3,
                Wide::U32(&[104, 105, 103, 104, 39, 0]),
                100,
                50,
                46,
                |_| false
            ),
            Ok(110)
        );
        assert_eq!(
            parameter(
                3,
                Wide::U32(&[104, 105, 103, 104, 39, 0]),
                i32::MAX,
                50,
                46,
                |_| false
            ),
            Err(Error::Arithmetic)
        );
        assert_eq!(
            number(Wide::U32(&[49, 50]), 0, 46, |_| false),
            Err(Error::Bounds)
        );
    }
}
