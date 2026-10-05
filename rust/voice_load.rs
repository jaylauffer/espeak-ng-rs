//! Ordered active voice loading with explicit backend/translator operations.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    voice::Voice,
    voice_directive::{self, Action, Features},
    voice_reader::Reader,
    voice_setup::Setup,
};
use std::io::Read;

/// Host operations run on the caller's initialization/worker path. Snapshot
/// arguments are borrowed only during each operation and cannot be retained.
/// The C compatibility host still owns translators, dictionaries and backends.
pub trait Host {
    #[allow(clippy::too_many_arguments)]
    fn directive(
        &mut self,
        voice: &mut Voice,
        fast: &mut i32,
        setup: &Setup,
        key: &[u8],
        value: &[u8],
        action: Action,
    ) -> bool;
    fn invalid(&mut self, key: &[u8]);
    fn ensure_translator(&mut self, setup: &Setup);
    fn select_table(&mut self, name: &[u8]) -> i32;
    fn unknown_table(&mut self, name: &[u8]);
    fn phoneme_index(&mut self, index: i32);
    fn dictionary(&mut self, name: &[u8], quiet: bool) -> bool;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Backend,
    Dictionary,
    InvalidSetup,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Completion {
    pub read_error: bool,
}
fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())]
}

/// Dispatch reusable streamed directives, then finish translator/table/dictionary
/// loading. Tone-only loads skip final language work; compilation skips table and
/// dictionary I/O. Rejected directives retain snapshots and continue, while a
/// backend failure stops immediately. Legacy partial-file I/O errors finish with
/// the accepted prefix; the native result reports that condition explicitly.
pub fn configure<R: Read>(
    mut reader: Option<&mut Reader<R>>,
    setup: &mut Setup,
    voice: &mut Voice,
    fast: &mut i32,
    features: Features,
    control: u32,
    host: &mut impl Host,
) -> Result<Completion, Error> {
    if setup.language_length >= 100
        || (setup.tone_only != 0) != (control & 2 != 0)
        || [
            &setup.translator[..],
            &setup.dictionary[..],
            &setup.phonemes[..],
            &setup.name[..],
            &setup.language[..],
        ]
        .iter()
        .any(|name| !name.contains(&0))
    {
        return Err(Error::InvalidSetup);
    }
    let mut completion = Completion::default();
    if let Some(reader) = reader.as_mut() {
        loop {
            let (key, value) = match reader.next_directive() {
                Ok(Some(directive)) => directive,
                Ok(None) => break,
                Err(_) => {
                    completion.read_error = true;
                    break;
                }
            };
            let action = match voice_directive::apply(voice, setup, fast, features, key, value) {
                Ok(action) => action,
                Err(_) => {
                    host.invalid(key);
                    continue;
                }
            };
            if !host.directive(voice, fast, setup, key, value, action) {
                return Err(Error::Backend);
            }
        }
    }
    if control & 2 == 0 {
        host.ensure_translator(setup);
        let index = if control & 8 != 0 {
            0
        } else {
            let name = string(&setup.phonemes);
            let index = host.select_table(name);
            if index < 0 {
                host.unknown_table(name);
                0
            } else {
                index
            }
        };
        host.phoneme_index(index);
        if control & 8 == 0 && !host.dictionary(string(&setup.dictionary), control & 4 != 0) {
            return Err(Error::Dictionary);
        }
        setup.languages[setup.language_length as usize] = 0;
    }
    Ok(completion)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Broken<'a>(&'a [u8]);
    impl Read for Broken<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() {
                return Err(std::io::Error::other("fixture read failure"));
            }
            let count = self.0.len().min(output.len());
            output[..count].copy_from_slice(&self.0[..count]);
            self.0 = &self.0[count..];
            Ok(count)
        }
    }
    #[derive(Default)]
    struct Owner {
        events: Vec<String>,
        backend_failure: bool,
        dictionary_failure: bool,
    }
    impl Host for Owner {
        fn directive(
            &mut self,
            _: &mut Voice,
            _: &mut i32,
            _: &Setup,
            key: &[u8],
            _: &[u8],
            action: Action,
        ) -> bool {
            self.events.push(format!(
                "directive:{}:{action:?}",
                String::from_utf8_lossy(key)
            ));
            !(self.backend_failure && matches!(action, Action::Mbrola(_)))
        }
        fn invalid(&mut self, key: &[u8]) {
            self.events
                .push(format!("invalid:{}", String::from_utf8_lossy(key)));
        }
        fn ensure_translator(&mut self, _: &Setup) {
            self.events.push("translator".into());
        }
        fn select_table(&mut self, name: &[u8]) -> i32 {
            self.events
                .push(format!("table:{}", String::from_utf8_lossy(name)));
            -1
        }
        fn unknown_table(&mut self, _: &[u8]) {
            self.events.push("unknown".into());
        }
        fn phoneme_index(&mut self, index: i32) {
            self.events.push(format!("index:{index}"));
        }
        fn dictionary(&mut self, name: &[u8], quiet: bool) -> bool {
            self.events.push(format!(
                "dictionary:{}:{quiet}",
                String::from_utf8_lossy(name)
            ));
            !self.dictionary_failure
        }
    }
    #[test]
    fn ordinary_tone_compilation_and_failed_backends_preserve_operation_order() {
        let bytes = b"language en-gb 5\npitch 999999999999 118\nspeed 110\nmbrola en1 table\n";
        for control in [0, 2, 4, 8, 10] {
            let mut stream = Reader::new(
                std::io::Cursor::new(bytes),
                4096,
                crate::voice_reader::TextMode::Binary,
            )
            .unwrap();
            let mut setup = Setup::new(b"en", control & 2 != 0).unwrap();
            setup.tone_only = control & 2; // Compatibility ABI uses the mask value 2.
            let mut voice = Voice::default();
            let mut fast = 450;
            let mut host = Owner::default();
            configure(
                Some(&mut stream),
                &mut setup,
                &mut voice,
                &mut fast,
                Features {
                    klatt: true,
                    mbrola: true,
                },
                control,
                &mut host,
            )
            .unwrap();
            assert_eq!(voice.speed_percent, 110);
            assert!(host.events[1].starts_with("invalid:pitch"));
            if control & 2 != 0 {
                assert_eq!(host.events.len(), 4);
            } else if control & 8 != 0 {
                assert_eq!(&host.events[4..], ["translator", "index:0"]);
            } else {
                assert_eq!(
                    &host.events[4..],
                    [
                        "translator",
                        "table:en",
                        "unknown",
                        "index:0",
                        if control & 4 != 0 {
                            "dictionary:en:true"
                        } else {
                            "dictionary:en:false"
                        }
                    ]
                );
            }
        }
        let mut stream = Reader::new(
            std::io::Cursor::new(bytes),
            4096,
            crate::voice_reader::TextMode::Binary,
        )
        .unwrap();
        let mut setup = Setup::new(b"en", false).unwrap();
        let mut voice = Voice::default();
        let mut fast = 450;
        let mut host = Owner {
            backend_failure: true,
            ..Owner::default()
        };
        assert_eq!(
            configure(
                Some(&mut stream),
                &mut setup,
                &mut voice,
                &mut fast,
                Features {
                    klatt: true,
                    mbrola: true
                },
                0,
                &mut host
            ),
            Err(Error::Backend)
        );
        assert_eq!(host.events.len(), 4);
        host.events.clear();
        host.dictionary_failure = true;
        assert_eq!(
            configure::<std::io::Cursor<&[u8]>>(
                None,
                &mut setup,
                &mut voice,
                &mut fast,
                Features::default(),
                0,
                &mut host
            ),
            Err(Error::Dictionary)
        );
    }
    #[test]
    fn partial_io_errors_finish_the_accepted_prefix_and_invalid_setup_has_no_effects() {
        let mut setup = Setup::new(b"en", false).unwrap();
        let mut voice = Voice::default();
        let mut fast = 450;
        let mut host = Owner::default();
        let mut reader = Reader::new(
            Broken(b"speed 110\npitch 100"),
            4096,
            crate::voice_reader::TextMode::Binary,
        )
        .unwrap();
        let result = configure(
            Some(&mut reader),
            &mut setup,
            &mut voice,
            &mut fast,
            Features::default(),
            0,
            &mut host,
        )
        .unwrap();
        assert!(result.read_error);
        assert_eq!(voice.speed_percent, 110);
        assert_eq!(voice.pitch_base, 0);
        assert_eq!(host.events.len(), 6);
        assert!(host.events[0].starts_with("directive:speed:"));
        host.events.clear();
        setup.language_length = 100;
        let before = (setup, voice, fast);
        assert_eq!(
            configure::<std::io::Cursor<&[u8]>>(
                None,
                &mut setup,
                &mut voice,
                &mut fast,
                Features::default(),
                0,
                &mut host
            ),
            Err(Error::InvalidSetup)
        );
        assert_eq!((setup, voice, fast), before);
        assert!(host.events.is_empty());
    }
}
