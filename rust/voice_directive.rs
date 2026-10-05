//! Ordered active-voice directive dispatch with explicit owner actions.
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
use crate::{
    language_options,
    phoneme_data::InvalidPhonemeData as Error,
    voice::Voice,
    voice_backend::Mbrola,
    voice_setup::{Effect, Setup},
};
#[derive(Clone, Copy, Debug, Default)]
pub struct Features {
    pub klatt: bool,
    pub mbrola: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    LanguageOption(u32),
    Acoustics { update_speed: bool },
    Metadata(Effect),
    Replacement(Effect),
    Mbrola(Mbrola),
    UnsupportedMbrola,
    UnsupportedKlatt,
    Unknown,
}
/// Apply native snapshot layers in legacy file order. Language options require
/// the owner's current translator; backend/table selection and speed updates
/// are explicit actions. None of those owner operations run during parsing.
/// Each rejected directive retains voice, setup and fast snapshots.
pub fn apply(
    voice: &mut Voice,
    setup: &mut Setup,
    fast: &mut i32,
    features: Features,
    key: &[u8],
    value: &[u8],
) -> Result<Action, Error> {
    if let Some(option) = language_options::key(key) {
        return Ok(Action::LanguageOption(option));
    }
    if let Some(update_speed) = voice.apply(key, value, features.klatt, fast)? {
        return Ok(Action::Acoustics { update_speed });
    }
    if let Some(effect) = setup.apply(key, value)? {
        return Ok(if key == b"replace" {
            Action::Replacement(effect)
        } else {
            Action::Metadata(effect)
        });
    }
    match key {
        b"mbrola" if features.mbrola => Mbrola::parse(value).map(Action::Mbrola),
        b"mbrola" => Ok(Action::UnsupportedMbrola),
        b"klatt" => Ok(Action::UnsupportedKlatt),
        _ => Ok(Action::Unknown),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layered_directives_report_owner_effects_without_executing_them() {
        let mut voice = Voice::default();
        let mut setup = Setup::new(b"en", false).unwrap();
        let mut fast = 450;
        let features = Features {
            klatt: true,
            mbrola: true,
        };
        assert_eq!(
            apply(
                &mut voice,
                &mut setup,
                &mut fast,
                features,
                b"language",
                b"en-gb 5"
            )
            .unwrap(),
            Action::Metadata(Effect::SelectLanguage)
        );
        assert_eq!(
            apply(&mut voice, &mut setup, &mut fast, features, b"replace", b"1 a b").unwrap(),
            Action::Replacement(Effect::SelectPhonemes)
        );
        assert_eq!(
            apply(&mut voice, &mut setup, &mut fast, features, b"replace", b"2 a b").unwrap(),
            Action::Replacement(Effect::None)
        );
        assert_eq!(
            apply(&mut voice, &mut setup, &mut fast, features, b"speed", b"110").unwrap(),
            Action::Acoustics { update_speed: true }
        );
        assert_eq!(voice.speed_percent, 110);
        assert!(matches!(
            apply(
                &mut voice,
                &mut setup,
                &mut fast,
                features,
                b"stressLength",
                b"160 170"
            )
            .unwrap(),
            Action::LanguageOption(_)
        ));
        assert!(matches!(
            apply(
                &mut voice,
                &mut setup,
                &mut fast,
                features,
                b"mbrola",
                b"en1 table 22050"
            )
            .unwrap(),
            Action::Mbrola(_)
        ));
        assert_eq!(
            apply(
                &mut voice,
                &mut setup,
                &mut fast,
                features,
                b"unrecognized",
                b""
            )
            .unwrap(),
            Action::Unknown
        );
    }
    #[test]
    fn unavailable_backends_and_rejected_values_preserve_snapshots() {
        let mut voice = Voice::default();
        let mut setup = Setup::new(b"en", false).unwrap();
        let mut fast = 450;
        let before = (voice, setup, fast);
        assert_eq!(
            apply(
                &mut voice,
                &mut setup,
                &mut fast,
                Features::default(),
                b"klatt",
                b"999999999999999"
            )
            .unwrap(),
            Action::UnsupportedKlatt
        );
        assert_eq!(
            apply(
                &mut voice,
                &mut setup,
                &mut fast,
                Features::default(),
                b"mbrola",
                b""
            )
            .unwrap(),
            Action::UnsupportedMbrola
        );
        assert_eq!((voice, setup, fast), before);
        assert!(apply(
            &mut voice,
            &mut setup,
            &mut fast,
            Features::default(),
            b"pitch",
            b"99999999999999 100"
        )
        .is_err());
        assert!(apply(
            &mut voice,
            &mut setup,
            &mut fast,
            Features::default(),
            b"language",
            b"---"
        )
        .is_err());
        assert_eq!((voice, setup, fast), before);
    }
}
