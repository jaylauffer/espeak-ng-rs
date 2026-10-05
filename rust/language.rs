//! Native language presets and per-instance mutable language configuration.
//!
//! Immutable tables are shared; selecting a language and applying directives
//! allocate nothing and perform no I/O. Tables retain this fork's semantics.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    language_options::Options,
    letters::{LetterSet, WideLetters},
    phoneme_data::InvalidPhonemeData,
};

#[path = "language_data.rs"]
mod data;

/// Fixed scalar translator settings, separate from mutable voice directives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Settings {
    pub break_numbers: u32,
    pub max_roman: i32,
    pub min_roman: i32,
    pub max_digits: i32,
    pub accents: i32,
    pub tone_language: i32,
    pub long_stop: i32,
    pub max_initial_consonants: i32,
    pub tone_numbers: i32,
    pub ideographs: i32,
    pub textmode: i32,
    pub dotless_i: i32,
    pub listx: i32,
    pub our_alphabet: i32,
    pub alt_alphabet: i32,
    pub alt_alphabet_lang: i32,
    pub max_lengthmod: i32,
    pub lengthen_tonic: i32,
    pub suffix_add_e: i32,
    pub transpose_min: i32,
    pub transpose_max: i32,
    pub encoding: i32,
    pub letter_bits_offset: i32,
}

#[derive(Debug)]
pub struct Profile {
    pub options: Options,
    pub settings: Settings,
    pub letter_bits: &'static [u8; 256],
    pub punct_to_tone: &'static [u8; 48],
    pub transpose_map: Option<&'static [u8]>,
    /// Includes the original 0x7fff sentinel.
    pub pairs: Option<&'static [i16]>,
    pub lengths: &'static [u8; 100],
    pub last_lengths: &'static [u8; 100],
    /// Immutable wide tables include their final NUL.
    pub apostrophe: &'static [u32],
    pub punctuation: &'static [u32],
    pub ignored: &'static [u16],
    pub groups: [Option<&'static [u32]>; 8],
    #[cfg(windows)]
    #[cfg_attr(not(feature = "c-abi"), allow(dead_code))] // UTF-16 view for the C ABI
    pub(crate) apostrophe_wide: &'static [u16],
    #[cfg(windows)]
    #[cfg_attr(not(feature = "c-abi"), allow(dead_code))]
    pub(crate) punctuation_wide: &'static [u16],
    #[cfg(windows)]
    #[cfg_attr(not(feature = "c-abi"), allow(dead_code))]
    pub(crate) groups_wide: [Option<&'static [u16]>; 8],
    pub ordinal: Option<&'static [u8]>,
    pub roman: &'static [u8],
    pub dictionary_override: &'static [u8],
}

/// Preserve the four-byte rolling language selector, with defined wrapping.
/// The compatibility string limit is checked by `Language::new`.
pub fn code(name: &[u8]) -> u32 {
    name.iter()
        .take_while(|&&byte| byte != 0)
        .fold(0_u32, |value, &byte| {
            value.wrapping_shl(8).wrapping_add(u32::from(byte))
        })
}

pub fn profile(selector: u32) -> &'static Profile {
    data::PROFILES
        .binary_search_by_key(&selector, |(key, _)| *key)
        .map_or(&data::DEFAULT, |index| &data::PROFILES[index].1)
}

#[derive(Debug, Clone)]
pub struct Language {
    pub selector: u32,
    pub options: Options,
    pub settings: Settings,
    pub preset: &'static Profile,
    dictionary: [u8; 40],
}
impl Language {
    pub fn new(name: &[u8]) -> Result<Self, InvalidPhonemeData> {
        if name.len() >= 40 || name.contains(&0) {
            return Err(InvalidPhonemeData(
                "language name must fit 39 bytes without NUL",
            ));
        }
        let selector = code(name);
        let preset = profile(selector);
        let actual_name = if preset.dictionary_override.is_empty() {
            name
        } else {
            preset.dictionary_override
        };
        let mut dictionary = [0; 40];
        dictionary[..actual_name.len()].copy_from_slice(actual_name);
        Ok(Self {
            selector,
            options: preset.options,
            settings: preset.settings,
            preset,
            dictionary,
        })
    }
    pub fn dictionary(&self) -> &[u8] {
        &self.dictionary[..self
            .dictionary
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(40)]
    }
    pub fn letters(&self) -> LetterSet<'static> {
        LetterSet {
            bits: self.preset.letter_bits,
            offset: self.settings.letter_bits_offset,
            groups: self
                .preset
                .groups
                .map(|group| group.map(|units| WideLetters::U32(&units[..units.len() - 1]))),
        }
    }
    pub fn alphabet(&self) -> crate::word_key::Alphabet<'static> {
        let pairs = self
            .preset
            .pairs
            .map_or(&[][..], |pairs| &pairs[..pairs.len() - 1]);
        crate::word_key::Alphabet {
            min: self.settings.transpose_min as u32,
            max: self.settings.transpose_max as u32,
            map: self.preset.transpose_map,
            pairs,
        }
    }
}

#[derive(Debug)]
pub struct Alphabet {
    /// Original symbol name, with a terminating NUL for compatibility export.
    pub name: &'static [u8],
    pub offset: i32,
    pub first: u32,
    pub last: u32,
    pub language: i32,
    pub flags: i32,
}
pub fn alphabet_from_char(character: i32) -> Option<&'static Alphabet> {
    let character = u32::try_from(character).ok()?;
    data::ALPHABETS
        .iter()
        .find(|alphabet| (alphabet.first..=alphabet.last).contains(&character))
}

#[cfg(feature = "c-abi")]
pub(crate) fn alphabet_index(character: i32) -> Option<usize> {
    let character = u32::try_from(character).ok()?;
    data::ALPHABETS
        .iter()
        .position(|alphabet| (alphabet.first..=alphabet.last).contains(&character))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_are_bounded_and_native_instances_are_independent() {
        assert!(data::PROFILES.windows(2).all(|pair| pair[0].0 < pair[1].0));
        for profile in data::PROFILES
            .iter()
            .map(|(_, profile)| profile)
            .chain(std::iter::once(&data::DEFAULT))
        {
            for units in profile
                .groups
                .iter()
                .flatten()
                .chain([profile.apostrophe, profile.punctuation].iter())
            {
                assert_eq!(units.last(), Some(&0));
                assert!(!units[..units.len() - 1].contains(&0));
            }
            if let Some(map) = profile.transpose_map {
                assert_eq!(
                    map.len(),
                    (profile.settings.transpose_max - profile.settings.transpose_min + 1) as usize
                );
            }
            if let Some(pairs) = profile.pairs {
                assert_eq!(pairs.last(), Some(&0x7fff));
            }
        }
        let mut en = Language::new(b"en").unwrap();
        let second = Language::new(b"en").unwrap();
        en.options.stress_lengths[0] = 1;
        assert_ne!(en.options.stress_lengths, second.options.stress_lengths);
        assert!(std::ptr::eq(en.preset, second.preset));
        assert_eq!(Language::new(b"sr").unwrap().dictionary(), b"hbs");
        assert_eq!(
            Language::new(b"prefixen").unwrap().dictionary(),
            b"prefixen"
        );
        assert!(Language::new(&[b'a'; 40]).is_err());
        assert!(Language::new(b"en\0").is_err());
        assert_eq!(Language::new(b"xex").unwrap().preset.punctuation, &[39, 0]);
        assert_eq!(Language::new(b"unknown").unwrap().options.parameters[4], 1);
    }
    #[test]
    fn alphabet_ranges_and_language_adapters_preserve_boundaries() {
        for alphabet in data::ALPHABETS {
            assert!(std::ptr::eq(
                alphabet_from_char(alphabet.first as i32).unwrap(),
                alphabet
            ));
            assert!(std::ptr::eq(
                alphabet_from_char(alphabet.last as i32).unwrap(),
                alphabet
            ));
        }
        assert!(alphabet_from_char(-1).is_none());
        assert!(alphabet_from_char(0x110000).is_none());
        assert_eq!(alphabet_from_char(0x43a).unwrap().name, b"_cyr\0");
        assert_eq!(code(b"prefixcmn"), code(b"xcmn"));
        let vi = Language::new(b"vi").unwrap();
        assert!(vi.letters().is_letter('ờ' as u32, 0));
        let ru = Language::new(b"ru").unwrap();
        assert!(ru.letters().is_letter('а' as u32, 0));
        assert_eq!(ru.alphabet().min, 0x430);
        assert_eq!(ru.alphabet().pairs.len(), 29);
    }
}
