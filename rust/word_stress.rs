//! Native extraction and language-specific assignment of word stress.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme::Phoneme;

pub const WORD_BYTES: usize = 200;
pub const SYLLABLES: usize = WORD_BYTES / 2;
pub type Table<'a> = [Option<&'a Phoneme>; 256];
const NONSYLLABIC: u32 = 1 << 20;
const UNSTRESSED: u32 = 1 << 1;
const LONG: u32 = 1 << 21;
const VOWEL: u8 = 2;
const LENGTHEN: u8 = 12;
const SYLLABIC: u8 = 20;
pub const STRESS_CODES: [u8; 7] = [3, 2, 4, 5, 6, 7, 26];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Terminator,
    Table,
    Stress,
    Syllables,
}

pub struct Extracted {
    pub phonemes: [u8; WORD_BYTES],
    pub length: usize,
    pub stress: [i8; SYLLABLES],
    /// One past the final syllable, retaining the compatibility sentinel.
    pub count: usize,
    pub primary: usize,
    pub maximum: i32,
}
/// Strip stress markers and unknown codes into bounded owned planning storage.
/// The input and sparse selected table remain immutable; no callbacks occur.
pub fn extract(
    input: &[u8],
    table: &Table<'_>,
    flags: u32,
    requested: i32,
    control: u32,
) -> Result<Extracted, Error> {
    let end = input
        .iter()
        .take(WORD_BYTES)
        .position(|c| *c == 0)
        .ok_or(Error::Terminator)?;
    let mut result = Extracted {
        phonemes: [0; WORD_BYTES],
        length: 0,
        stress: [0; SYLLABLES],
        count: 1,
        primary: 0,
        maximum: -1,
    };
    result.stress[0] = 1;
    let mut stress = -1;
    for &code in &input[..end] {
        if result.count >= SYLLABLES - 1 {
            break;
        }
        let Some(ph) = table[code as usize] else {
            continue;
        };
        if ph.kind == 1 && ph.program == 0 {
            if code == 8 {
                let mut j = result.count - 1;
                while j > 0 && requested == 0 && result.stress[j] < 4 {
                    if result.stress[j] != 0 && result.stress[j] != 1 {
                        result.stress[j] = 4;
                        if result.maximum < 4 {
                            result.maximum = 4;
                            result.primary = j;
                        }
                        for prior in &mut result.stress[1..j] {
                            if *prior == 4 {
                                *prior = 3;
                            }
                        }
                        break;
                    }
                    j -= 1;
                }
            } else if ph.standard_length < 4 || requested == 0 {
                stress = i32::from(ph.standard_length);
                result.maximum = result.maximum.max(stress);
            }
            continue;
        }
        if ph.kind == VOWEL && ph.flags & NONSYLLABIC == 0 {
            result.stress[result.count] = stress as i8;
            if stress >= 4 && stress >= result.maximum {
                result.primary = result.count;
                result.maximum = stress;
            }
            if stress < 0 && control & 1 != 0 && ph.flags & UNSTRESSED != 0 {
                result.stress[result.count] = 1;
            }
            result.count += 1;
            stress = -1;
        } else if code == SYLLABIC {
            result.stress[result.count] = if stress < 0 && control & 1 != 0 {
                1
            } else {
                stress as i8
            };
            result.count += 1;
        }
        result.phonemes[result.length] = code;
        result.length += 1;
    }
    result.stress[result.count] = 1;
    if requested > 0 {
        result.primary = (requested as usize).min(result.count - 1);
        result.stress[result.primary] = 4;
        result.maximum = 4;
    }
    if result.maximum == 5 {
        for index in 1..result.count {
            if result.stress[index] == 4 {
                result.stress[index] = if flags & 0x20000 != 0 { 1 } else { 3 };
            }
            if result.stress[index] == 5 {
                result.stress[index] = 4;
                result.primary = index;
            }
        }
        result.maximum = 4;
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Settings {
    pub language: u32,
    pub flags: u32,
    pub rule: i32,
    pub unstressed_one: i32,
    pub unstressed_many: i32,
    pub vowel_pause: i32,
    pub lengthen: i32,
    pub previous: i32,
}
impl Settings {
    pub fn from_options(
        language: u32,
        options: &crate::language_options::Options,
        previous: i32,
    ) -> Self {
        Self {
            language,
            flags: options.stress_flags,
            rule: options.stress_rule,
            unstressed_one: options.unstressed_single,
            unstressed_many: options.unstressed_multiple,
            vowel_pause: options.vowel_pause,
            lengthen: options.parameters[1],
            previous,
        }
    }
}
pub struct Assigned {
    pub phonemes: [u8; WORD_BYTES],
    pub length: usize,
    pub previous: i32,
}
fn record<'a>(table: &'a Table<'_>, code: u8) -> Result<&'a Phoneme, Error> {
    table[code as usize].ok_or(Error::Table)
}
fn consonant(kind: u8) -> bool {
    (3..=9).contains(&kind)
}

/// Plan the complete result before publication. Unknown/gap input codes become
/// schwa, as in C; missing records, invalid stresses and unterminated words fail
/// without mutations. Output is clipped by the same 197-byte loop admission.
/// All scratch is fixed stack storage, independent of the number of requests.
pub fn assign(
    input: &[u8],
    table: &Table<'_>,
    table_count: usize,
    settings: &Settings,
    dictionary: Option<u32>,
    mut tonic: i32,
    control: u32,
) -> Result<Assigned, Error> {
    if table_count > 256 {
        return Err(Error::Table);
    }
    if tonic > 6 {
        return Err(Error::Stress);
    }
    let end = input
        .iter()
        .take(WORD_BYTES)
        .position(|c| *c == 0)
        .ok_or(Error::Terminator)?;
    let mut output = Assigned {
        phonemes: [0; WORD_BYTES],
        length: 0,
        previous: settings.previous,
    };
    if end == 0 {
        return Ok(output);
    }
    let mut phonetic = [0; WORD_BYTES];
    for (index, &code) in input[..end].iter().enumerate() {
        phonetic[index] = if code as usize >= table_count || table[code as usize].is_none() {
            13
        } else {
            code
        };
        record(table, phonetic[index])?;
    }
    let final_ph = record(table, phonetic[end - 1])?;
    let final_ph2 = record(table, phonetic[end.saturating_sub(2)])?;
    let flags = settings.flags;
    let dflags = dictionary.unwrap_or(0);
    let unstressed = dflags & 8 != 0;
    let requested = (dflags & if unstressed { 3 } else { 7 }) as i32;
    let mut extracted = extract(&phonetic, table, flags, requested, 1)?;
    let count = extracted.count;
    let mut primary = extracted.primary;
    let mut max_stress = extracted.maximum;
    let max_input = max_stress;
    if max_stress < 0 && dictionary.is_some() {
        max_stress = 0;
    }
    let stress = &mut extracted.stress;
    let phs = &extracted.phonemes;
    let mut weight = [0; SYLLABLES];
    let mut length = [0; SYLLABLES];
    let mut index = 0;
    let mut syllable = 1;
    // C expects an initialized pause record at code zero. The owned zero tail
    // provides deterministic lookahead without reading past the input word.
    while index < extracted.length {
        let ph = record(table, phs[index])?;
        if ph.kind == VOWEL && ph.flags & NONSYLLABIC == 0 {
            if syllable >= SYLLABLES {
                return Err(Error::Syllables);
            }
            let next = record(table, *phs.get(index + 1).unwrap_or(&0))?;
            let lengthened = next.code == LENGTHEN;
            let mut value = i32::from(lengthened || ph.flags & LONG != 0);
            length[syllable] = value;
            if lengthened {
                index += 1;
            }
            let next = record(table, *phs.get(index + 1).unwrap_or(&0))?;
            let after = record(table, *phs.get(index + 2).unwrap_or(&0))?;
            if consonant(next.kind) && (after.kind != VOWEL || next.flags & LONG != 0) {
                value += 1;
            }
            weight[syllable] = value;
            syllable += 1;
        }
        index += 1;
    }
    match settings.rule {
        1 | 8 if settings.rule == 1 || (weight[1] == 0 && weight[2] > 0) => {
            if primary == 0 && count > 2 {
                primary = 2;
                if max_stress == 0 {
                    stress[primary] = 4;
                }
                max_stress = 4;
            }
        }
        2 if primary == 0 => {
            max_stress = 4;
            primary = if count > 2 { count - 2 } else { 1 };
            if count > 2 {
                if flags & 0x200 != 0 && final_ph.kind != VOWEL {
                    let mnemonic = final_ph.mnemonic;
                    let final_stress = match settings.language {
                        0x616e | 0x6361 => {
                            (mnemonic != b's' as u32 && mnemonic != b'n' as u32)
                                || final_ph2.kind != VOWEL
                        }
                        0x6961 => mnemonic != b's' as u32 || final_ph2.kind != VOWEL,
                        _ => {
                            !(mnemonic == b's' as u32 && final_ph2.kind == 8)
                                && ((final_ph.kind != 8 && mnemonic != b's' as u32)
                                    || final_ph2.kind != VOWEL)
                        }
                    };
                    if final_stress {
                        primary = count - 1;
                    }
                }
                if flags & 0x80000 != 0 && length[count - 1] > length[count - 2] {
                    primary = count - 1;
                }
                if stress[primary] == 0 || stress[primary] == 1 {
                    if primary > 1 {
                        primary -= 1;
                    } else {
                        primary += 1;
                    }
                }
            }
            if stress[primary] < 0 && (stress[primary - 1] < 4 || stress[primary + 1] < 4) {
                stress[primary] = max_stress as i8;
            }
        }
        3 if primary == 0 => {
            primary = count - 1;
            while primary > 0 {
                if stress[primary] < 0 {
                    stress[primary] = 4;
                    break;
                }
                primary -= 1;
            }
            max_stress = 4;
        }
        4 if primary == 0 => {
            primary = count.saturating_sub(3).max(1);
            if max_stress == 0 {
                stress[primary] = 4;
            }
            max_stress = 4;
        }
        5 if primary == 0 => {
            const GUESS: [usize; 16] = [0, 0, 1, 1, 2, 3, 3, 4, 5, 6, 7, 7, 8, 9, 10, 11];
            const GUESS_V: [usize; 16] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 5, 6, 7, 7, 8, 9, 10];
            const GUESS_T: [usize; 16] = [0, 0, 1, 2, 3, 3, 3, 4, 5, 6, 7, 7, 7, 8, 9, 10];
            primary = if count < 16 {
                match final_ph.kind {
                    VOWEL => GUESS_V[count],
                    4 => GUESS_T[count],
                    _ => GUESS[count],
                }
            } else {
                count - 3
            };
            stress[primary] = 4;
            max_stress = 4;
        }
        6 if primary == 0 => {
            let mut max_weight = -1;
            for i in 1..count - 1 {
                if stress[i] < 0 && weight[i] >= max_weight {
                    max_weight = weight[i];
                    primary = i;
                }
            }
            if weight[count - 1] == 2 && max_weight < 2 {
                primary = count - 1;
            } else if max_weight <= 0 {
                primary = 1;
            }
            stress[primary] = 4;
            max_stress = 4;
        }
        7 if primary == 0 => {
            primary = count - 1;
            for (i, value) in stress.iter().enumerate().take(count).skip(1) {
                if *value == 1 {
                    primary = i - 1;
                    break;
                }
            }
            stress[primary] = 4;
            max_stress = 4;
        }
        9 => {
            for value in &mut stress[1..count] {
                if *value < 0 {
                    *value = 4;
                }
            }
        }
        12 => {
            let mut long = 0;
            for i in 1..count {
                if stress[i] == 4 {
                    stress[i] = 3;
                }
                if length[i] > 0 {
                    long = i;
                    stress[i] = 3;
                }
            }
            if primary == 0 {
                primary = if long > 0 {
                    long
                } else if count > 5 {
                    count - 3
                } else {
                    count - 1
                };
            }
            stress[primary] = 4;
            max_stress = 4;
        }
        13 if primary == 0 => {
            primary = if length[1] == 0 && count > 2 && length[2] > 0 {
                2
            } else {
                1
            };
            stress[primary] = 4;
            max_stress = 4;
        }
        15 if primary == 0 && count > 2 => {
            stress[1..count].fill(0);
            primary = 2;
            if max_stress == 0 {
                stress[primary] = 4;
            }
            max_stress = 4;
            if count > 3 {
                stress[count - 1] = 3;
            }
        }
        _ => {}
    }
    if flags & 0x100 != 0
        && control & 2 == 0
        && count > 2
        && max_input < 3
        && stress[count - 1] == 4
        && final_ph.kind == VOWEL
    {
        stress[count - 1] = 1;
        stress[count - 2] = 4;
    }
    let mut inferred = if max_stress < 4 { 4 } else { 3 };
    if !unstressed {
        if flags & 0x1000 != 0 && count == 3 {
            if stress[1] == 4 {
                stress[2] = 3;
            }
            if stress[2] == 4 {
                stress[1] = 3;
            }
        }
        if flags & 0x2000 != 0 && stress[1] < 0 && count > 3 && stress[2] >= 4 {
            stress[1] = 3;
        }
    }
    let mut done = false;
    let mut first_primary = 0;
    for v in 1..count {
        if stress[v] < 0 {
            if flags & 0x10 != 0 && inferred < 4 && v == count - 1 {
            }
            // final secondary disabled
            else if flags & 0x8000 != 0 && !done {
                stress[v] = inferred;
                done = true;
                inferred = 3;
            } else if stress[v - 1] <= 1
                && (stress[v + 1] <= 1 || (inferred == 4 && stress[v + 1] <= 2))
            {
                if inferred == 3 && flags & 0x20 != 0 {
                    continue;
                }
                if v > 1
                    && flags & 0x40 != 0
                    && weight[v] == 0
                    && (weight[v..count - 1].iter().any(|w| *w > 0) || weight[v + 1] > 0)
                {
                    continue;
                }
                stress[v] = inferred;
                done = true;
                inferred = 3;
            }
        }
        if stress[v] >= 4 {
            if first_primary == 0 {
                first_primary = v;
            } else if flags & 0x80 != 0 {
                stress[v] = 3;
            }
        }
    }
    if unstressed && tonic < 0 {
        tonic = if count <= 2 {
            settings.unstressed_one
        } else {
            settings.unstressed_many
        };
    }
    if tonic > 6 {
        return Err(Error::Stress);
    }
    max_stress = 0;
    let mut maximum_position = 0;
    for (v, value) in stress.iter().enumerate().take(count).skip(1) {
        if i32::from(*value) >= max_stress {
            max_stress = i32::from(*value);
            maximum_position = v;
        }
    }
    if tonic >= 0 {
        if tonic > max_stress || max_stress <= 4 {
            stress[maximum_position] = tonic as i8;
        }
        max_stress = tonic;
    }
    index = 0;
    if control & 1 == 0 {
        while index < extracted.length {
            let ph = record(table, phs[index])?;
            if ph.kind != 1 && phs[index] != 15 {
                break;
            }
            index += 1;
        }
        if settings.vowel_pause & 0x30 != 0 && record(table, phs[index])?.kind == VOWEL {
            output.phonemes[0] = if settings.vowel_pause & 0x20 != 0 && stress[1] >= 4 {
                11
            } else {
                23
            };
            output.length = 1;
        }
    }
    index = 0;
    let mut v = 1;
    while index < extracted.length && output.length < WORD_BYTES - 3 {
        let code = phs[index];
        index += 1;
        let ph = record(table, code)?;
        if ph.kind == 0 {
            output.previous = 0;
        } else if (ph.kind == VOWEL && ph.flags & NONSYLLABIC == 0) || phs[index] == SYLLABIC {
            if v > count {
                return Err(Error::Syllables);
            }
            let mut value = stress[v];
            output.previous = i32::from(value);
            if value <= 1 {
                if v > 1 && max_stress >= 2 && flags & 4 != 0 && v == count - 1 {
                    value = 0;
                } else if flags & 2 != 0
                    || v == 1
                    || v == count - 1
                    || (v == count.saturating_sub(2) && stress[count - 1] <= 1)
                {
                    value = 1;
                } else if stress[v - 1] < 0 || flags & 0x10000 == 0 {
                    value = 0;
                    stress[v] = value;
                }
            }
            if value == 0 || value > 1 {
                let marker = STRESS_CODES.get(value as usize).ok_or(Error::Stress)?;
                output.phonemes[output.length] = *marker;
                output.length += 1;
            }
            max_stress = max_stress.max(i32::from(stress[v]));
            if phs[index] == LENGTHEN && settings.lengthen & 1 != 0 {
                let shorten = if settings.lengthen & 0x10 != 0 {
                    v != maximum_position
                } else {
                    value < 4
                };
                if shorten {
                    index += 1;
                }
            }
            v += 1;
        }
        if code != 1 {
            output.phonemes[output.length] = code;
            output.length += 1;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn records() -> [Phoneme; 256] {
        let mut records = [Phoneme::default(); 256];
        for (code, record) in records.iter_mut().enumerate() {
            record.code = code as u8;
        }
        for (level, code) in STRESS_CODES.iter().enumerate() {
            records[*code as usize].kind = 1;
            records[*code as usize].standard_length = level as u8;
        }
        records[8].kind = 1;
        records[13].kind = VOWEL;
        records[40].kind = VOWEL;
        records[41].kind = VOWEL;
        records[41].flags = UNSTRESSED;
        records[42].kind = 8;
        records[42].mnemonic = b'n' as u32;
        records
    }
    #[test]
    fn priority_previous_and_forced_syllables_preserve_order() {
        let records = records();
        let table = records.each_ref().map(Some);
        let result = extract(&[6, 40, 7, 40, 41, 42, 20, 0], &table, 0x20000, 0, 1).unwrap();
        assert_eq!(&result.phonemes[..result.length], &[40, 40, 41, 42, 20]);
        assert_eq!(&result.stress[..result.count + 1], &[1, 1, 4, 1, 1, 1]);
        assert_eq!(result.primary, 2);
        let result = extract(&[40, 40, 8, 0], &table, 0, 0, 0).unwrap();
        assert_eq!(&result.stress[..4], &[1, -1, 4, 1]);
        let result = extract(&[40, 0], &table, 0, 20, 0).unwrap();
        assert_eq!(result.primary, 1);
    }
    #[test]
    fn language_rules_emit_owner_effects_and_validate_before_publication() {
        let records = records();
        let mut table = records.each_ref().map(Some);
        let mut settings = Settings {
            rule: 2,
            vowel_pause: 0x20,
            previous: 4,
            ..Settings::default()
        };
        let result = assign(&[40, 42, 40, 0], &table, 256, &settings, Some(0), -1, 0).unwrap();
        assert_eq!(
            &result.phonemes[..result.length + 1],
            &[11, 6, 40, 42, 40, 0]
        );
        assert_eq!(result.previous, -1);
        settings.rule = 3;
        let result = assign(&[40, 42, 40, 0], &table, 256, &settings, Some(0), 6, 1).unwrap();
        assert_eq!(&result.phonemes[..result.length + 1], &[40, 42, 26, 40, 0]);
        assert!(matches!(
            assign(&[40; 200], &table, 256, &settings, None, -1, 0),
            Err(Error::Terminator)
        ));
        assert!(matches!(
            assign(&[40, 0], &table, 256, &settings, None, 7, 0),
            Err(Error::Stress)
        ));
        table[42] = None;
        table[13] = None;
        assert!(matches!(
            assign(&[42, 0], &table, 256, &settings, None, -1, 0),
            Err(Error::Table)
        ));
    }
    #[test]
    fn maximal_vowels_and_output_are_bounded() {
        let records = records();
        let table = records.each_ref().map(Some);
        let mut input = [40; 200];
        input[199] = 0;
        let extracted = extract(&input, &table, 0, 0, 0).unwrap();
        assert_eq!(extracted.count, 99);
        assert_eq!(extracted.length, 98);
        let result = assign(&input, &table, 256, &Settings::default(), None, -1, 0).unwrap();
        assert!(result.length < 200);
    }
}
