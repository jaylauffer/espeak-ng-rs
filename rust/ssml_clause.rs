//! SSML pause and ordered clause/voice transition plans.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ssml::{self, Attribute, Wide};
pub const NONE: i32 = 0x4000;
pub const VOICE_CHANGE: i32 = 0x20000;
const VOICE: i32 = NONE | VOICE_CHANGE;
const PERIOD: i32 = 0x80000 + 40;
const PARAGRAPH: i32 = 0x80000 + 70;
const COLON: i32 = 0x40000 + 30;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Arithmetic,
    Tag,
    Effect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Break {
    pub value: i32,
    pub terminator: i32,
    pub timed: u32,
    pub milliseconds: i32,
    pub rate: i32,
    pub length: u32,
    pub command: [u8; 4],
}
/// Plan attributes and multiplier arithmetic before publishing a command or
/// requesting a host rate update. The owner publishes command+NUL first, then
/// applies the rate update and snapshots pause factors for `finish`.
pub fn pause(
    input: Wide<'_>,
    start: usize,
    rate: i32,
    multiplier: i32,
    space: impl Fn(u32) -> bool,
) -> Result<Break, Error> {
    if input.len() > crate::ssml_control::TAG_UNITS
        || start == 0
        || start >= input.len()
        || !(start..input.len()).any(|i| input.get(i) == Some(0))
    {
        return Err(Error::Bounds);
    }
    let find = |name: &[u8]| ssml::attribute(input, start, name, &space).unwrap_or(None);
    let text = |attr| match attr {
        Some(Attribute::Value(index)) => input.tail(index),
        _ => Some(Wide::U32(&[0])),
    };
    let mut result = Break {
        value: 21,
        terminator: NONE,
        timed: 0,
        milliseconds: 0,
        rate,
        length: 0,
        command: [0; 4],
    };
    if let Some(strength) = find(b"strength") {
        let text = text(Some(strength));
        let index = [
            (b"none".as_slice(), 0),
            (b"x-weak", 1),
            (b"weak", 2),
            (b"medium", 3),
            (b"strong", 4),
            (b"x-strong", 5),
        ]
        .iter()
        .find(|(name, _)| ssml::attribute_matches(text, name))
        .map_or(2, |(_, index)| *index);
        if index < 3 {
            result.command = [1, b'0' + index as u8, b'B', 0];
            result.length = 3;
            result.terminator = 0;
        }
        result.value = [0, 7, 14, 21, 40, 80][index];
    }
    if let Some(time) = find(b"time") {
        let value =
            ssml::attribute_number(text(Some(time)), 0, true).map_err(|_| Error::Arithmetic)?;
        result.milliseconds = value.checked_mul(multiplier).ok_or(Error::Arithmetic)? / 100;
        result.timed = 1;
    }
    Ok(result)
}
impl Break {
    pub fn finish(&self, clause_pause: i32, pause: i32, sonic: bool) -> Result<i32, Error> {
        if self.timed > 1 || !matches!(self.terminator, 0 | NONE) || !matches!(self.length, 0 | 3) {
            return Err(Error::Effect);
        }
        let mut value = self.value;
        let mut terminator = self.terminator;
        if self.timed != 0 {
            let mut milliseconds = self.milliseconds;
            if sonic && self.rate >= 450 {
                let multiplier = f64::from(self.rate) / 175.0;
                let value = f64::from(milliseconds) * multiplier;
                if !value.is_finite()
                    || value.trunc() < f64::from(i32::MIN)
                    || value.trunc() > f64::from(i32::MAX)
                {
                    return Err(Error::Arithmetic);
                }
                milliseconds = value as i32;
            }
            let numerator = milliseconds.checked_mul(256).ok_or(Error::Arithmetic)?;
            let divide = |factor: i32| {
                numerator
                    .checked_div(factor.checked_mul(10).ok_or(Error::Arithmetic)?)
                    .ok_or(Error::Arithmetic)
            };
            value = divide(clause_pause)?;
            if value < 200 {
                value = divide(pause)?;
            }
            if terminator == 0 {
                terminator = NONE;
            }
        }
        if terminator != 0 {
            if value > 0xfff {
                value = (value / 32).min(0xfff);
                terminator |= 0x800000;
            }
            terminator.checked_add(value).ok_or(Error::Arithmetic)
        } else {
            Ok(0)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct VoicePlan {
    pub count: u32,
    pub length: u32,
    pub tags: [i32; 3],
    pub terminator: i32,
    pub open: u32,
}
/// Only initialized frame kinds are needed. Ordered host selection calls are
/// explicit actions; no engine borrow or callback crosses this pure plan.
pub fn voice(kind: i32, frames: &[i32]) -> Result<VoicePlan, Error> {
    if frames.is_empty() || frames.len() > 20 {
        return Err(Error::Bounds);
    }
    let top = frames[frames.len() - 1];
    let mut result = VoicePlan {
        count: frames.len() as u32,
        length: 0,
        tags: [0; 3],
        terminator: 0,
        open: 0,
    };
    let mut add = |tag| {
        result.tags[result.length as usize] = tag;
        result.length += 1;
    };
    match kind {
        1 | 2 => {
            add(kind);
            result.open = 1;
        }
        33 | 34 => {
            let wanted = kind - 32;
            let mut count = frames.len();
            while count > 1 && frames[count - 1] != wanted {
                count -= 1;
            }
            add(kind);
            result.count = count as u32;
            result.terminator = if kind == 33 { PERIOD } else { 0 };
        }
        15 | 47 => result.terminator = COLON,
        6 | 7 => {
            if top == 6 {
                add(38);
            }
            if kind == 7 && top == 7 {
                add(39);
            }
            add(kind);
            result.terminator = PARAGRAPH;
        }
        38 => {
            if top == 6 {
                add(kind);
            }
            result.terminator = PERIOD;
        }
        39 => {
            if matches!(top, 6 | 7) {
                add(kind);
            }
            result.terminator = PARAGRAPH;
        }
        _ => return Err(Error::Tag),
    }
    Ok(result)
}
impl VoicePlan {
    pub fn finish(&self, flags: i32) -> Result<i32, Error> {
        if !matches!(flags, 0 | VOICE_CHANGE)
            || self.open > 1
            || !(1..=20).contains(&self.count)
            || self.length > 3
            || !matches!(self.terminator, 0 | PERIOD | PARAGRAPH | COLON)
        {
            return Err(Error::Effect);
        }
        Ok(if self.open != 0 {
            if flags == 0 {
                0
            } else {
                VOICE
            }
        } else {
            self.terminator
                .checked_add(flags)
                .ok_or(Error::Arithmetic)?
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timed_pause_checks_denominators_products_and_long_scaling() {
        let xml: Vec<_> = " strength='weak' time='10s'"
            .chars()
            .map(u32::from)
            .chain([0])
            .collect();
        let p = pause(Wide::U32(&xml), 1, 700, 100, |c| c == 32).unwrap();
        assert_eq!(p.command, [1, b'2', b'B', 0]);
        assert_eq!(p.finish(256, 128, false), Ok(NONE + 1000));
        assert_eq!(p.finish(256, 128, true), Ok(NONE + 4000));
        assert_eq!(p.finish(1, 128, false), Ok((NONE | 0x800000) + 4095));
        assert_eq!(p.finish(0, 128, false), Err(Error::Arithmetic));
        assert_eq!(p.finish(i32::MAX, 128, false), Err(Error::Arithmetic));
        let p = Break {
            milliseconds: i32::MAX,
            ..p
        };
        assert_eq!(p.finish(256, 128, false), Err(Error::Arithmetic));
    }
    #[test]
    fn voice_transitions_unwind_and_preserve_order_without_mutating_frames() {
        let p = voice(33, &[0, 1, 6, 7]).unwrap();
        assert_eq!((p.count, p.tags[0]), (2, 33));
        assert_eq!(p.finish(VOICE_CHANGE), Ok(PERIOD + VOICE_CHANGE));
        let p = voice(7, &[0, 6]).unwrap();
        assert_eq!(&p.tags[..p.length as usize], &[38, 7]);
        assert_eq!(p.count, 2);
        assert_eq!(p.finish(0), Ok(PARAGRAPH));
        assert_eq!(voice(2, &[0]).unwrap().finish(VOICE_CHANGE), Ok(VOICE));
        assert_eq!(voice(2, &[0]).unwrap().finish(0), Ok(0));
        assert_eq!(voice(2, &[]), Err(Error::Bounds));
    }
}
