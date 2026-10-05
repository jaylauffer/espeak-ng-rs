//! Per-instance current voice metadata and initial ordered setup snapshots.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{phoneme_data::InvalidPhonemeData as Error, voice_request, voice_setup::Setup};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Current {
    pub identifier: [u8; 40],
    pub name: [u8; 40],
    pub languages: [u8; 100],
}
impl Default for Current {
    fn default() -> Self {
        Self {
            identifier: [0; 40],
            name: [0; 40],
            languages: [0; 100],
        }
    }
}
impl Current {
    /// Snapshot before resetting acoustic/backend resources. Ordinary loads clear
    /// visible name/language metadata; variants keep both and replace the suffix.
    /// Failed identifiers/fallbacks leave this per-instance storage untouched.
    pub fn prepare(
        &mut self,
        requested: &[u8],
        fallback: &[u8],
        tone_only: bool,
        gender: u8,
        age: u8,
        language: &[u8; 20],
    ) -> Result<Setup, Error> {
        let mut next = *self;
        next.identifier = voice_request::identifier(&self.identifier, requested, tone_only)?;
        if !tone_only {
            next.name[0] = 0;
            next.languages[0] = 0;
        }
        let mut setup = Setup::new(fallback, tone_only)?;
        setup.name = next.name;
        setup.languages = next.languages;
        setup.language = *language;
        setup.gender = gender;
        setup.age = age;
        *self = next;
        Ok(setup)
    }
    pub fn commit(&mut self, setup: &Setup) {
        self.name = setup.name;
        self.languages = setup.languages;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_owners_preserve_variant_metadata_and_reject_without_mutation() {
        let mut first = Current::default();
        let second = Current::default();
        let mut language = [0; 20];
        language[..2].copy_from_slice(b"en");
        let mut setup = first
            .prepare(b"en", b"en", false, 1, 30, &language)
            .unwrap();
        setup.apply(b"name", b"Native").unwrap();
        setup.apply(b"language", b"en-gb 5").unwrap();
        first.commit(&setup);
        let before = first;
        let variant = first
            .prepare(b"!v/m2", b"en", true, 2, 40, &language)
            .unwrap();
        assert_eq!(&first.identifier[..6], b"en+m2\0");
        assert_eq!(first.name, before.name);
        assert_eq!(first.languages, before.languages);
        assert_eq!(
            (
                variant.gender,
                variant.age,
                variant.tone_only,
                variant.language_length
            ),
            (2, 40, 1, 0)
        );
        let saved = first;
        assert!(first.prepare(b"m", b"en", true, 0, 0, &language).is_err());
        assert_eq!(first, saved);
        assert!(first
            .prepare(b"en", &[b'a'; 40], false, 0, 0, &language)
            .is_err());
        assert_eq!(first, saved);
        first.prepare(b"fr", b"fr", false, 0, 0, &language).unwrap();
        assert_eq!(first.name[0], 0);
        assert_eq!(first.languages[0], 0);
        assert_eq!(second, Current::default());
    }
}
