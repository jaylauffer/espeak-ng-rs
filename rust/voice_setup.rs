//! Ordered metadata directives for an active voice, distinct from catalogue metadata.
//!
//! Produce explicit owner effects while retaining bounded per-instance state.
//! Acoustic and language-option directives use their native modules; this layer
//! handles language selection, dictionary/table overrides and current-voice fields.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{phoneme_data::InvalidPhonemeData as Error, voice::numbers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Setup {
    pub translator: [u8; 40],
    pub dictionary: [u8; 40],
    pub phonemes: [u8; 40],
    pub name: [u8; 40],
    pub language: [u8; 20],
    pub languages: [u8; 100],
    pub language_length: u32,
    pub language_set: u32,
    pub phonemes_set: u32,
    pub tone_only: u32,
    pub gender: u8,
    pub age: u8,
}
impl Default for Setup {
    fn default() -> Self {
        Self {
            translator: [0; 40],
            dictionary: [0; 40],
            phonemes: [0; 40],
            name: [0; 40],
            language: [0; 20],
            languages: [0; 100],
            language_length: 0,
            language_set: 0,
            phonemes_set: 0,
            tone_only: 0,
            gender: 0,
            age: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    SelectLanguage,
    SelectPhonemes,
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
fn string<const N: usize>(output: &mut [u8; N], bytes: &[u8]) {
    output.fill(0);
    let count = bytes.len().min(N - 1);
    output[..count].copy_from_slice(&bytes[..count]);
}
impl Setup {
    pub fn new(fallback: &[u8], tone_only: bool) -> Result<Self, Error> {
        if fallback.len() >= 40 || fallback.contains(&0) {
            return Err(Error("voice fallback identifier exceeds bound"));
        }
        let mut state = Self {
            tone_only: u32::from(tone_only),
            ..Self::default()
        };
        string(&mut state.translator, fallback);
        string(&mut state.dictionary, fallback);
        Ok(state)
    }
    /// Recognized attributes return an explicit owner effect; unknown attributes
    /// belong to the other configuration layers. Reject before snapshot commit.
    pub fn apply(&mut self, key: &[u8], value: &[u8]) -> Result<Option<Effect>, Error> {
        let value = &value[..value
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(value.len())];
        let mut next = *self;
        let mut effect = Effect::None;
        match key {
            b"language" => {
                if next.tone_only != 0 {
                    return Ok(Some(effect));
                }
                let (language, remainder) = token(value);
                if language.len() >= 40 {
                    return Err(Error("active voice language exceeds 39 bytes"));
                }
                if language == b"variant" {
                    return Ok(Some(effect));
                }
                let (priority, count) = numbers::<1>(remainder)?;
                let priority = if count > 0 { priority[0] } else { 5 };
                let used = next.language_length as usize;
                if used > 98 {
                    return Err(Error("invalid active voice language bound"));
                }
                let length = language.len() + 2;
                if length < 100 - used - 1 {
                    next.languages[used] = priority as u8;
                    next.languages[used + 1..used + language.len() + 1].copy_from_slice(language);
                    next.languages[used + language.len() + 1] = 0;
                    next.language_length += length as u32;
                    next.languages[next.language_length as usize] = 0;
                }
                if next.language_set == 0 {
                    // C strtok skips leading separators, then ends the primary
                    // language at the first separator after a nonempty token.
                    let start = language
                        .iter()
                        .position(|byte| *byte != b'-')
                        .ok_or(Error("voice language has no primary identifier"))?;
                    let rest = &language[start..];
                    let end = rest
                        .iter()
                        .position(|byte| *byte == b'-')
                        .unwrap_or(rest.len());
                    let primary = &rest[..end];
                    string(&mut next.translator, primary);
                    string(&mut next.dictionary, primary);
                    string(&mut next.phonemes, primary);
                    // The legacy acoustic name uses the original buffer after
                    // strtok, including any skipped leading '-' bytes.
                    string(&mut next.language, &language[..start + end]);
                    next.language_set = 1;
                    effect = Effect::SelectLanguage;
                }
            }
            b"name" => {
                if next.tone_only == 0 {
                    let start = value
                        .iter()
                        .position(|byte| !space(byte))
                        .unwrap_or(value.len());
                    string(&mut next.name, &value[start..]);
                }
            }
            b"gender" => {
                let (gender, remainder) = token(value);
                if gender.is_empty() || gender.len() >= 80 {
                    return Err(Error("active voice gender token is missing or overlong"));
                }
                let (age, count) = numbers::<1>(remainder)?;
                next.gender = if gender == b"female" { 2 } else { 1 };
                next.age = if count > 0 { age[0] as u8 } else { 0 };
            }
            b"dictionary" | b"phonemes" => {
                let (name, _) = token(value);
                if name.len() >= 40 {
                    return Err(Error("voice dictionary or phoneme name exceeds 39 bytes"));
                }
                if !name.is_empty() {
                    if key == b"dictionary" {
                        string(&mut next.dictionary, name);
                    } else {
                        string(&mut next.phonemes, name);
                    }
                }
            }
            b"maintainer" | b"status" => {}
            b"replace" => {
                if next.phonemes_set == 0 {
                    next.phonemes_set = 1;
                    effect = Effect::SelectPhonemes;
                }
            }
            _ => return Ok(None),
        }
        *self = next;
        Ok(Some(effect))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_language_and_ordered_overrides_preserve_active_metadata() {
        let mut setup = Setup::new(b"en", false).unwrap();
        assert_eq!(
            setup.apply(b"language", b"en-gb 2").unwrap(),
            Some(Effect::SelectLanguage)
        );
        assert!(setup.translator.starts_with(b"en\0"));
        assert!(setup.language.starts_with(b"en\0"));
        setup.apply(b"dictionary", b"custom").unwrap();
        setup.apply(b"phonemes", b"en-us").unwrap();
        assert_eq!(
            setup.apply(b"language", b"de 7").unwrap(),
            Some(Effect::None)
        );
        assert!(setup.dictionary.starts_with(b"custom\0"));
        assert!(setup.phonemes.starts_with(b"en-us\0"));
        assert_eq!(&setup.languages[..12], b"\x02en-gb\0\x07de\0\0");
        setup.apply(b"gender", b"female 20").unwrap();
        setup.apply(b"gender", b"female malformed").unwrap();
        assert_eq!(setup.age, 0);
        assert_eq!(setup.gender, 2);
        let original = setup;
        assert!(setup.apply(b"dictionary", &[b'a'; 40]).is_err());
        assert_eq!(setup, original);
        let mut tone = Setup::new(b"en", true).unwrap();
        tone.apply(b"language", b"de").unwrap();
        tone.apply(b"name", b"other").unwrap();
        assert_eq!(tone.language_set, 0);
        assert_eq!(tone.name, [0; 40]);
    }
    #[test]
    fn malformed_primary_languages_fail_without_partial_admission() {
        let mut setup = Setup::new(b"en", false).unwrap();
        let original = setup;
        for language in [b"".as_slice(), b"---", b"en 9999999999999999"] {
            assert!(setup.apply(b"language", language).is_err());
            assert_eq!(setup, original);
        }
        setup.apply(b"language", b"--en-gb").unwrap();
        assert!(setup.translator.starts_with(b"en\0"));
        assert!(setup.language.starts_with(b"--en\0"));
    }
}
