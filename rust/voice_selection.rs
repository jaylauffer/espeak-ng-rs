//! Voice metadata parsing and ranking on borrowed, bounded configuration.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme_data::InvalidPhonemeData as Error,
    voice::{numbers, Directives},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Metadata {
    pub name: [u8; 80],
    pub gender_name: [u8; 80],
    pub languages: [u8; 300],
    pub language_length: u32,
    pub language_count: u32,
    pub age: i32,
    pub variants: i32,
}
impl Default for Metadata {
    fn default() -> Self {
        Self {
            name: [0; 80],
            gender_name: [0; 80],
            languages: [0; 300],
            language_length: 0,
            language_count: 0,
            age: 0,
            variants: 4,
        }
    }
}
fn space(byte: &u8) -> bool {
    byte.is_ascii_whitespace() || *byte == 0x0b
}
fn token(bytes: &[u8]) -> (&[u8], &[u8]) {
    let start = bytes
        .iter()
        .position(|byte| !space(byte))
        .unwrap_or(bytes.len());
    let bytes = &bytes[start..];
    let end = bytes.iter().position(space).unwrap_or(bytes.len());
    (&bytes[..end], &bytes[end..])
}
fn set_string<const N: usize>(output: &mut [u8; N], bytes: &[u8]) {
    let count = bytes.len().min(N - 1);
    output.fill(0);
    output[..count].copy_from_slice(&bytes[..count]);
}
impl Metadata {
    /// Feed one parsed directive; returns whether a gender directive occurred.
    /// Reject overflowing legacy `%s` tokens and numeric input before mutation.
    pub fn apply(&mut self, key: &[u8], value: &[u8]) -> Result<bool, Error> {
        match key {
            b"name" => {
                let start = value
                    .iter()
                    .position(|byte| !space(byte))
                    .unwrap_or(value.len());
                set_string(&mut self.name, &value[start..]);
            }
            b"language" => {
                let (language, remainder) = token(value);
                if language.len() >= 80 {
                    return Err(Error("voice language token exceeds 79 bytes"));
                }
                let (priority, count) = numbers::<1>(remainder)?;
                let priority = if count > 0 { priority[0] } else { 5 };
                let used = self.language_length as usize;
                let length = language.len() + 2;
                if used > 298 {
                    return Err(Error("invalid voice metadata language bound"));
                }
                if length < 300 - used - 1 {
                    self.languages[used] = priority as u8;
                    self.languages[used + 1..used + 1 + language.len()].copy_from_slice(language);
                    self.languages[used + 1 + language.len()] = 0;
                    self.language_length += length as u32;
                    self.language_count += 1;
                }
            }
            b"gender" => {
                let (gender, remainder) = token(value);
                if gender.len() >= 80 {
                    return Err(Error("voice gender token exceeds 79 bytes"));
                }
                let (age, count) = numbers::<1>(remainder)?;
                if !gender.is_empty() {
                    set_string(&mut self.gender_name, gender);
                }
                if count > 0 {
                    self.age = age[0];
                }
                return Ok(true);
            }
            b"variants" => {
                let (values, count) = numbers::<1>(value)?;
                if count > 0 {
                    self.variants = values[0];
                }
            }
            _ => {}
        }
        Ok(false)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut metadata = Self::default();
        for (key, value) in Directives::new(bytes, 120)? {
            metadata.apply(key, value)?;
        }
        Ok(metadata)
    }
    pub fn gender(&self) -> u8 {
        if self.gender_name.starts_with(b"female\0") {
            2
        } else {
            1
        }
    }
    pub fn view<'a>(&'a self, identifier: &'a [u8]) -> Option<Voice<'a>> {
        if self.language_count == 0 || self.language_length >= 300 {
            return None;
        }
        let name_len = self.name.iter().position(|byte| *byte == 0).unwrap_or(80);
        let name = if name_len == 0 {
            identifier
        } else {
            &self.name[..name_len]
        };
        Some(Voice {
            name,
            identifier,
            languages: &self.languages[..self.language_length as usize + 1],
            gender: self.gender(),
            age: self.age as u8,
            variants: self.variants as u8,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Voice<'a> {
    pub name: &'a [u8],
    pub identifier: &'a [u8],
    /// Sequence of priority bytes and NUL-terminated names; final zero byte.
    pub languages: &'a [u8],
    pub gender: u8,
    pub age: u8,
    pub variants: u8,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Selector<'a> {
    pub name: Option<&'a [u8]>,
    pub gender: u8,
    pub age: u8,
}
pub fn score(
    spec: Selector<'_>,
    language: &[u8],
    parts: i32,
    voice: Voice<'_>,
) -> Result<i32, Error> {
    if parts > 80 || language.len() > 79 {
        return Err(Error("voice selector exceeds language bound"));
    }
    if parts < 0 {
        return Ok(if voice.identifier.starts_with(language) {
            100
        } else {
            0
        });
    }
    let mut score = 0;
    if parts == 0 || (voice.languages.first() == Some(&0) && language == b"variants") {
        score = 100;
    } else {
        let mut cursor = 0;
        loop {
            let priority = *voice
                .languages
                .get(cursor)
                .ok_or(Error("unterminated voice language list"))?;
            if priority == 0 {
                break;
            }
            cursor += 1;
            let remaining = &voice.languages[cursor..];
            let length = remaining
                .iter()
                .position(|byte| *byte == 0)
                .ok_or(Error("unterminated voice language name"))?;
            let candidate = &remaining[..length];
            let mut candidate_parts = 1;
            let mut matching = true;
            let mut matches = 0;
            for index in 0..=candidate.len() {
                let c1 = language
                    .get(index)
                    .copied()
                    .filter(|byte| *byte != b'-')
                    .unwrap_or(0);
                let c2 = candidate
                    .get(index)
                    .copied()
                    .filter(|byte| *byte != b'-')
                    .unwrap_or(0);
                if c1 != c2 {
                    matching = false;
                }
                if candidate.get(index) == Some(&b'-') {
                    candidate_parts += 1;
                    if matching {
                        matches += 1;
                    }
                }
            }
            matches += i32::from(matching);
            if matches > 0 {
                let x = (5 - (parts - matches).max(0) - (candidate_parts - matches).max(0)) * 100
                    - i32::from(priority as std::ffi::c_char) * 2;
                score = score.max(x);
            }
            cursor += length + 1;
        }
    }
    if score == 0 {
        return Ok(0);
    }
    if let Some(name) = spec.name {
        if name == voice.name {
            score += 500;
        } else if name == voice.identifier {
            score += 400;
        }
    }
    if matches!(spec.gender, 1 | 2) && matches!(voice.gender, 1 | 2) {
        score += if spec.gender == voice.gender { 50 } else { -50 };
    }
    if spec.age <= 12 && voice.gender == 2 && voice.age > 12 {
        score += 5;
    }
    if voice.age != 0 {
        let required = if spec.age == 0 {
            30
        } else {
            i32::from(spec.age)
        };
        let mut ratio = required * 100 / i32::from(voice.age);
        if ratio < 100 {
            if ratio == 0 {
                return Err(Error("voice age ratio would divide by zero"));
            }
            ratio = 10000 / ratio;
        }
        ratio = (ratio - 100) / 10;
        score += (5 - ratio).min(0);
        if spec.age > 0 {
            score += 10;
        }
    }
    Ok(score.max(1))
}

/// Case-insensitive visible name wins first; exact identifiers and final
/// path components retain their last candidate's precedence. Short paths
/// are compared safely instead of reading before the identifier allocation.
pub fn by_name<'a>(
    voices: impl IntoIterator<Item = Voice<'a>>,
    name: &[u8],
    separator: u8,
) -> Option<usize> {
    let name = &name[..name.len().min(39)];
    let mut identifier = None;
    let mut suffix = None;
    for (index, voice) in voices.into_iter().enumerate() {
        if voice.name.eq_ignore_ascii_case(name) {
            return Some(index);
        }
        if voice.identifier.eq_ignore_ascii_case(name) {
            identifier = Some(index);
        } else if voice.identifier.len() > name.len() {
            let start = voice.identifier.len() - name.len();
            if voice.identifier[start - 1] == separator
                && voice.identifier[start..].eq_ignore_ascii_case(name)
            {
                suffix = Some(index);
            }
        }
    }
    identifier.or(suffix)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Variant {
    pub base_length: usize,
    suffix: [u8; 40],
}
impl Variant {
    pub fn suffix(&self) -> &[u8] {
        &self.suffix[..self.suffix.iter().position(|byte| *byte == 0).unwrap_or(40)]
    }
    pub fn terminated_suffix(&self) -> &[u8; 40] {
        &self.suffix
    }
}
pub fn variant(
    name: Option<&[u8]>,
    mut number: i32,
    directory: bool,
    separator: u8,
) -> Result<Variant, Error> {
    let name = name.unwrap_or_default();
    let mut base_length = name.len();
    let mut named = &[][..];
    let mut named_present = false;
    if let Some(plus) = name.iter().position(|byte| *byte == b'+') {
        base_length = plus;
        let tail = &name[plus + 1..];
        number = 0;
        if tail.first().is_some_and(u8::is_ascii_digit) {
            let (values, count) = numbers::<1>(tail)?;
            if count > 0 {
                number = values[0];
            }
        } else {
            named = tail;
            named_present = true;
        }
    }
    let mut suffix = [0; 40];
    let mut used = 0;
    if named_present || number > 0 {
        if directory {
            suffix[..3].copy_from_slice(&[b'!', b'v', separator]);
            used = 3;
        }
        if number > 0 {
            suffix[used] = if number < 10 { b'm' } else { b'f' };
            used += 1;
            let mut number = if number < 10 { number } else { number - 10 } as u32;
            let mut digits = [0; 10];
            let mut count = 0;
            loop {
                digits[count] = (number % 10) as u8 + b'0';
                count += 1;
                number /= 10;
                if number == 0 {
                    break;
                }
            }
            for digit in digits[..count].iter().rev() {
                suffix[used] = *digit;
                used += 1;
            }
        } else {
            if used + named.len() >= 40 {
                return Err(Error("voice variant suffix exceeds 39 bytes"));
            }
            suffix[used..used + named.len()].copy_from_slice(named);
        }
    }
    Ok(Variant {
        base_length,
        suffix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_is_bounded_and_preserves_partial_assignments() {
        let metadata=Metadata::parse(b"name Native voice // suffix\nlanguage en 2\ngender female 15\ngender male nope\nvariants -1\nlanguage\n").unwrap();
        assert_eq!(metadata.view(b"en/native").unwrap().name, b"Native voice ");
        assert_eq!(metadata.gender(), 1);
        assert_eq!(metadata.age, 15);
        assert_eq!(metadata.variants, -1);
        assert_eq!(&metadata.languages[..7], b"\x02en\0\x05\0\0");
        let mut copy = metadata;
        assert!(copy.apply(b"language", &[b'a'; 80]).is_err());
        assert_eq!(copy, metadata);
        assert!(copy.apply(b"gender", b"male 999999999999").is_err());
        assert_eq!(copy, metadata);
        assert!(Metadata::parse(b"name alone")
            .unwrap()
            .view(b"id")
            .is_none());
    }
    #[test]
    fn score_dialect_prefix_and_name_precedence_are_bounded() {
        let voice = Voice {
            name: b"Native",
            identifier: b"x/en",
            languages: b"\x05en-gb\0\x02en\0\0",
            gender: 1,
            age: 30,
            variants: 4,
        };
        assert_eq!(score(Selector::default(), b"en", 1, voice).unwrap(), 496);
        assert_eq!(
            score(
                Selector {
                    name: Some(b"Native"),
                    gender: 1,
                    age: 30
                },
                b"en",
                1,
                voice
            )
            .unwrap(),
            1056
        );
        assert_eq!(score(Selector::default(), b"x/", -1, voice).unwrap(), 100);
        assert_eq!(by_name([voice], b"EN", b'/'), Some(0));
        assert_eq!(by_name([voice], b"Native", b'/'), Some(0));
        assert_eq!(by_name([voice], b"longer-than-identifier", b'/'), None);
        assert!(score(
            Selector::default(),
            b"en",
            1,
            Voice {
                languages: b"\x05en",
                ..voice
            }
        )
        .is_err());
    }
    #[test]
    fn variants_preserve_numeric_prefixes_and_reject_oversized_suffixes() {
        assert_eq!(
            variant(Some(b"en+"), 7, true, b'/').unwrap().suffix(),
            b"!v/"
        );
        assert_eq!(
            variant(Some(b"en+12junk"), 3, true, b'/').unwrap().suffix(),
            b"!v/f2"
        );
        assert_eq!(variant(None, 1, false, b'/').unwrap().suffix(), b"m1");
        assert_eq!(
            variant(Some(b"en+named"), 7, false, b'/').unwrap().suffix(),
            b"named"
        );
        assert_eq!(
            variant(Some(b"en+0"), 7, false, b'/').unwrap().suffix(),
            b""
        );
        assert_eq!(
            variant(Some(b"en+"), 7, false, b'/').unwrap().base_length,
            2
        );
        assert_eq!(
            variant(Some(b"en+-3"), 7, false, b'/').unwrap().suffix(),
            b"-3"
        );
        assert!(variant(Some(b"en+9999999999999999"), 0, false, b'/').is_err());
        let mut long = b"en+".to_vec();
        long.extend_from_slice(&[b'a'; 40]);
        assert!(variant(Some(&long), 0, false, b'/').is_err());
    }
}
