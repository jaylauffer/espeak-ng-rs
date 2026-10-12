//! Complete speech-parameter, embedded translation-speed and vowel table policy.
// Copyright (C) 2005-2011 Jonathan Duddington, 2015-2016 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::ssml_parameters::PARAMETERS;
#[path = "parameter_length_tables.rs"]
mod tables;
pub const DEFAULTS: [i32; PARAMETERS] = [0, 175, 100, 50, 50, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0];
pub const EMBEDDED_LIMIT: usize = 250;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Arithmetic,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Rate(i32),
    Volume(i32),
    Pitch(i32),
    Range(i32),
    Line(i32),
    Gap(i32),
    Tone { flags: i32, group: Option<i32> },
    None,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameter {
    pub index: usize,
    pub stored: i32,
    pub effect: Effect,
    pub status: i32,
}
/// Preserve stored pre-clamp values, signed division and writes for unsupported
/// in-range parameters. Invalid indexing/arithmetic admits no publication.
pub fn parameter(
    defaults: &[i32; PARAMETERS],
    parameter: i32,
    value: i32,
    relative: bool,
) -> Result<Parameter, Error> {
    let index = usize::try_from(parameter)
        .ok()
        .filter(|i| *i < PARAMETERS)
        .ok_or(Error::Bounds)?;
    let stored = if relative && index < 5 {
        defaults[index]
            .checked_mul(value)
            .and_then(|v| defaults[index].checked_add(v / 100))
            .ok_or(Error::Arithmetic)?
    } else {
        value
    };
    let effect = match index {
        1 => Effect::Rate(stored),
        2 => Effect::Volume(stored),
        3 => Effect::Pitch(stored.clamp(0, 99)),
        4 => Effect::Range(stored.min(99)),
        7 => Effect::Gap(stored),
        9 => Effect::Tone {
            flags: stored,
            group: ((stored & 255) != 0).then_some(stored & 255),
        },
        13 => Effect::Line(stored),
        _ => Effect::None,
    };
    Ok(Parameter {
        index,
        stored,
        effect,
        status: if matches!(effect, Effect::None) && index != 10 {
            22
        } else {
            0
        },
    })
}
pub trait SpeedHost {
    /// Cursor advances before the matching embedded command's effects.
    fn advance(&mut self, cursor: usize);
    /// SetEmbedded(masked command, value), followed by SetSpeed(1).
    fn speed(&mut self, command: i32, value: i32);
}
pub fn boundary(words: &[u32], start: usize) -> Result<usize, Error> {
    if words.len() > EMBEDDED_LIMIT || start >= words.len() {
        return Err(Error::Bounds);
    }
    words[start..]
        .iter()
        .position(|w| w & 128 != 0)
        .map(|i| start + i + 1)
        .ok_or(Error::Bounds)
}
/// Admit the entire terminated bounded span before effects, including ignored
/// commands and zero words. No callback can change the admitted snapshot.
pub fn embedded_speed(
    words: &[u32],
    start: usize,
    host: &mut impl SpeedHost,
) -> Result<usize, Error> {
    let end = boundary(words, start)?;
    for (i, word) in words.iter().enumerate().take(end).skip(start) {
        host.advance(i + 1);
        if word & 31 == 2 {
            host.speed((word & 127) as i32, (word >> 8) as i32);
        }
    }
    Ok(end)
}
/// Six legacy aliases over three immutable owned 100-byte tables. The second
/// index is value/100; zero retains the first selection.
pub fn length_mods(value: i32) -> Result<(&'static [u8; 100], &'static [u8; 100]), Error> {
    fn table(index: i32) -> Result<&'static [u8; 100], Error> {
        Ok(match index {
            0 | 1 => &tables::EN,
            2 => &tables::EN0,
            3..=5 => &tables::EQUAL,
            _ => return Err(Error::Bounds),
        })
    }
    let first = table(value % 100)?;
    let second = if value / 100 == 0 {
        first
    } else {
        table(value / 100)?
    };
    Ok((first, second))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_values_clamps_and_unsupported_publication_preserve_policy() {
        let p = parameter(&DEFAULTS, 1, -125, true).unwrap();
        assert_eq!((p.stored, p.effect), (-43, Effect::Rate(-43)));
        assert_eq!(
            parameter(&DEFAULTS, 3, 300, false).unwrap().effect,
            Effect::Pitch(99)
        );
        assert_eq!(
            parameter(&DEFAULTS, 4, -123, false).unwrap().effect,
            Effect::Range(-123)
        );
        let p = parameter(&DEFAULTS, 5, 77, true).unwrap();
        assert_eq!((p.stored, p.effect, p.status), (77, Effect::None, 22));
        assert_eq!(parameter(&DEFAULTS, 10, 100, false).unwrap().status, 0);
        assert!(parameter(&DEFAULTS, -1, 10, true).is_err());
        assert!(parameter(&DEFAULTS, 15, 10, true).is_err());
        assert!(parameter(&DEFAULTS, 1, i32::MAX, true).is_err());
    }
    #[test]
    fn tone_and_length_tables_preserve_optional_aliases_and_bound_indices() {
        assert_eq!(
            parameter(&DEFAULTS, 9, 256, false).unwrap().effect,
            Effect::Tone {
                flags: 256,
                group: None
            }
        );
        assert_eq!(
            parameter(&DEFAULTS, 9, -1, false).unwrap().effect,
            Effect::Tone {
                flags: -1,
                group: Some(255)
            }
        );
        let (a, b) = length_mods(1).unwrap();
        assert!(std::ptr::eq(a, b));
        let (a, b) = length_mods(201).unwrap();
        assert!(!std::ptr::eq(a, b));
        assert_eq!((a[1], b[1]), (120, 150));
        assert!(length_mods(-1).is_err());
        assert!(length_mods(6).is_err());
        assert!(length_mods(600).is_err());
    }
    #[test]
    fn embedded_cursor_order_zero_words_and_last_slot_are_bounded() {
        #[derive(Default)]
        struct Host(Vec<(usize, i32, i32)>);
        impl SpeedHost for Host {
            fn advance(&mut self, cursor: usize) {
                self.0.push((cursor, 0, 0));
            }
            fn speed(&mut self, command: i32, value: i32) {
                self.0.push((0, command, value));
            }
        }
        let mut h = Host::default();
        assert_eq!(embedded_speed(&[0, (175 << 8) | 66, 128], 0, &mut h), Ok(3));
        assert_eq!(h.0, vec![(1, 0, 0), (2, 0, 0), (0, 66, 175), (3, 0, 0)]);
        let before = h.0.clone();
        assert!(embedded_speed(&[0; 250], 0, &mut h).is_err());
        assert_eq!(h.0, before);
        let mut words = [0; 250];
        words[249] = 130;
        assert_eq!(embedded_speed(&words, 249, &mut h), Ok(250));
        assert!(embedded_speed(&words, 250, &mut h).is_err());
    }
}
