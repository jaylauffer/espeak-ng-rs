//! Mutable language options on explicit owner snapshots.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2022 Juho Hiltunen; Rust adaptation (C) 2026. GPL-3.0-or-later.
use crate::{phoneme_data::InvalidPhonemeData as Error, voice::numbers};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Options {
    pub dictionary_minimum: i32,
    pub dictionary_conditions: u32,
    pub tone_flags: i32,
    pub stress_lengths: [i16; 8],
    pub stress_amplitudes: [u8; 8],
    pub word_gap: i32,
    pub vowel_pause: i32,
    pub stress_rule: i32,
    pub stress_flags: u32,
    pub unstressed_single: i32,
    pub unstressed_multiple: i32,
    pub parameters: [i32; 18],
    pub numbers: u32,
    pub numbers2: u32,
    pub thousands_separator: i32,
    pub decimal_separator: i32,
    pub intonation_group: i32,
    pub tunes: [u8; 6],
    pub lowercase_sentence: u8,
    pub spelling_stress: u8,
}
pub trait Environment {
    fn tune(&self, name: &[u8]) -> Option<i32>;
    fn bad_ordinal(&mut self, _key: u32, _number: i32) {}
    fn unknown_tune(&mut self, _name: &[u8]) {}
}
pub struct Tunes<'a>(pub &'a [&'a [u8]]);
pub fn key(keyword: &[u8]) -> Option<u32> {
    Some(match keyword {
        b"apostrophe" => 0x110,
        b"brackets" => 0x10c,
        b"bracketsAnnounced" => 0x111,
        b"dict_min" => 35,
        b"dictrules" => 27,
        b"intonation" => 21,
        b"l_dieresis" => 0x100,
        b"l_prefix" => 0x102,
        b"l_regressive_v" => 0x103,
        b"l_unpronouncable" => 0x104,
        b"l_sonorant_min" => 0x105,
        b"lowercaseSentence" => 19,
        b"numbers" => 30,
        b"spellingStress" => 23,
        b"stressAdd" => 26,
        b"stressAmp" => 25,
        b"stressLength" => 24,
        b"stressOpt" => 29,
        b"stressRule" => 28,
        b"tunes" => 22,
        b"words" => 20,
        _ => return None,
    })
}
impl Environment for Tunes<'_> {
    fn tune(&self, name: &[u8]) -> Option<i32> {
        self.0.iter().position(|n| *n == name).map(|i| i as i32)
    }
}
fn space(byte: &u8) -> bool {
    matches!(byte, b'\t'..=b'\r' | b' ')
}
pub fn ordinal_flags<E: Environment>(
    bytes: &[u8],
    maximum: i32,
    key: u32,
    environment: &mut E,
) -> Result<(u32, u32), Error> {
    if !(1..=64).contains(&maximum) {
        return Err(Error("language ordinal limit outside 1..=64"));
    }
    let mut cursor = 0;
    let mut first = 0;
    let mut second = 0;
    while cursor < bytes.len() && bytes[cursor] != 0 {
        while bytes.get(cursor).is_some_and(space) {
            cursor += 1;
        }
        if bytes.get(cursor).is_none_or(|c| *c == 0) {
            break;
        }
        let start = cursor;
        let (value, count) = numbers::<1>(&bytes[cursor..])?;
        if count == 1 && value[0] > 0 {
            cursor += 1;
            if value[0] >= maximum {
                environment.bad_ordinal(key, value[0]);
            } else if value[0] < 32 {
                first |= 1 << value[0];
            } else {
                second |= 1 << (value[0] - 32);
            }
        }
        while bytes.get(cursor).is_some_and(u8::is_ascii_alphanumeric) {
            cursor += 1;
        }
        if start == cursor {
            return Err(Error("non-progressing language ordinal token"));
        }
    }
    Ok((first, second))
}
pub fn separators(numbers: u32, thousands: &mut i32, decimal: &mut i32) {
    if numbers & 8 != 0 {
        *thousands = i32::from(b'.');
        *decimal = i32::from(b',');
    }
    if numbers & 4 != 0 {
        *thousands = 0;
    }
}
impl Options {
    /// Parse/validate on a value snapshot before committing mutable options.
    /// Diagnostics are owner effects; tune lookup is read-only setup work.
    pub fn apply<E: Environment>(
        &mut self,
        key: u32,
        bytes: &[u8],
        environment: &mut E,
    ) -> Result<(), Error> {
        let bytes = &bytes[..bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len())];
        let mut next = *self;
        match key {
            35 => {
                let (v, n) = numbers::<1>(bytes)?;
                if n == 1 {
                    next.dictionary_minimum = v[0];
                }
            }
            27 | 29 | 30 => {
                let (first, second) =
                    ordinal_flags(bytes, if key == 30 { 64 } else { 32 }, key, environment)?;
                match key {
                    27 => next.dictionary_conditions |= first,
                    29 => next.stress_flags |= first,
                    _ => {
                        next.numbers |= first;
                        next.numbers2 |= second;
                        separators(
                            next.numbers,
                            &mut next.thousands_separator,
                            &mut next.decimal_separator,
                        );
                    }
                }
            }
            21 => {
                let (v, n) = numbers::<1>(bytes)?;
                if n == 1 {
                    next.tone_flags = v[0];
                }
                if next.tone_flags & 255 != 0 {
                    next.intonation_group = next.tone_flags & 255;
                }
            }
            19 => next.lowercase_sentence = 1,
            23 => next.spelling_stress = 1,
            24..=26 => {
                let (v, n) = numbers::<8>(bytes)?;
                for (i, value) in v[..n.max(0) as usize].iter().enumerate() {
                    match key {
                        24 => next.stress_lengths[i] = *value as i16,
                        25 => next.stress_amplitudes[i] = *value as u8,
                        _ => {
                            next.stress_lengths[i] = (i32::from(next.stress_lengths[i])
                                .checked_add(*value)
                                .ok_or(Error("language stress addition overflow"))?)
                                as i16
                        }
                    }
                }
            }
            28 => {
                let (v, n) = numbers::<3>(bytes)?;
                for (target, value) in [
                    &mut next.stress_rule,
                    &mut next.unstressed_single,
                    &mut next.unstressed_multiple,
                ]
                .into_iter()
                .zip(v)
                .take(n.max(0) as usize)
                {
                    *target = value;
                }
            }
            22 => {
                next.intonation_group = 0;
                for (i, name) in bytes
                    .split(space)
                    .filter(|n| !n.is_empty())
                    .take(6)
                    .enumerate()
                {
                    let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];
                    if name.is_empty() {
                        break;
                    }
                    if name.len() > 39 {
                        return Err(Error("language tune name exceeds 39 bytes"));
                    }
                    if name == b"NULL" {
                        continue;
                    }
                    if let Some(index) = environment.tune(name) {
                        next.tunes[i] = index as u8;
                    } else {
                        environment.unknown_tune(name);
                    }
                }
            }
            20 => {
                let (v, n) = numbers::<2>(bytes)?;
                if n > 0 {
                    next.word_gap = v[0];
                }
                if n > 1 {
                    next.vowel_pause = v[1];
                }
            }
            _ if key & 0xff00 == 0x100 => {
                let target = next
                    .parameters
                    .get_mut((key & 255) as usize)
                    .ok_or(Error("language parameter index outside options"))?;
                let (v, n) = numbers::<1>(bytes)?;
                if n == 1 {
                    *target = v[0];
                }
            }
            _ => {}
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_options_preserve_partial_arrays_flags_and_tune_intent() {
        let mut options = Options {
            stress_lengths: [100; 8],
            stress_amplitudes: [20; 8],
            thousands_separator: i32::from(b','),
            decimal_separator: i32::from(b'.'),
            tunes: [7; 6],
            ..Default::default()
        };
        let mut tunes = Tunes(&[b"first", b"second"]);
        options.apply(24, b"160 180", &mut tunes).unwrap();
        options.apply(26, b"10 -5", &mut tunes).unwrap();
        assert_eq!(
            options.stress_lengths,
            [170, 175, 100, 100, 100, 100, 100, 100]
        );
        options.apply(25, b"260 -1", &mut tunes).unwrap();
        assert_eq!(&options.stress_amplitudes[..3], &[4, 255, 20]);
        options.apply(30, b"2 3 33 63", &mut tunes).unwrap();
        assert_eq!(options.numbers, 12);
        assert_eq!(options.numbers2, 0x80000002);
        assert_eq!(options.thousands_separator, 0);
        assert_eq!(options.decimal_separator, i32::from(b','));
        options.apply(21, b"265", &mut tunes).unwrap();
        assert_eq!(options.intonation_group, 9);
        options
            .apply(22, b"NULL second absent first", &mut tunes)
            .unwrap();
        assert_eq!(options.tunes, [7, 1, 7, 0, 7, 7]);
        assert_eq!(options.intonation_group, 0);
        options.apply(28, b"4 2", &mut tunes).unwrap();
        assert_eq!(options.stress_rule, 4);
        assert_eq!(options.unstressed_single, 2);
        options
            .apply(key(b"apostrophe").unwrap(), b"3", &mut tunes)
            .unwrap();
        assert_eq!(options.parameters[16], 3);
        options.apply(22, b"first\0 second", &mut tunes).unwrap();
        assert_eq!(options.tunes[1], 1);
    }
    #[test]
    fn non_progressing_and_overflowing_tokens_leave_snapshots_unchanged() {
        let mut options = Options {
            stress_lengths: [10; 8],
            ..Default::default()
        };
        let original = options;
        let mut tunes = Tunes(&[]);
        for (key, input) in [
            (30, b"1 -2".as_slice()),
            (29, b"1 , 2"),
            (26, b"2147483647"),
            (0x112, b"1"),
            (21, b"9999999999999999999999"),
            (22, b"abcdefghijklmnopqrstuvwxyzabcdefghijklmnop"),
        ] {
            assert!(options.apply(key, input, &mut tunes).is_err());
            assert_eq!(options, original);
        }
        assert_eq!(
            ordinal_flags(b"1foo 2+3 0 word", 32, 27, &mut tunes).unwrap(),
            (14, 0)
        );
        assert_eq!(
            ordinal_flags(b"31 32 63 64", 64, 30, &mut tunes).unwrap(),
            (0x80000000, 0x80000001)
        );
        assert!(ordinal_flags(b"1", 65, 30, &mut tunes).is_err());
        assert_eq!(key(b"language"), None);
    }
}
