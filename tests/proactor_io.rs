#![cfg(feature = "proactor")]
// SPDX-License-Identifier: GPL-3.0-or-later

use espeak_ng_rs::data_io::{open_data_file, DataReader};
use loadngo_proactor::new_platform_proactor;
mod support;
use std::io;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};
use support::HostDeadline;

struct Fixture(std::path::PathBuf);
static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "espeak-data-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        std::fs::write(&path, b"abcdefghij").unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn real_host_io_reuses_buffer_bounds_admission_and_retains_file() {
    let fixture = Fixture::new();
    let reader = DataReader::new(4).unwrap();
    let mut first_address = None;
    for (offset, expected) in [
        (0, b"abcd".as_slice()),
        (8, b"ij".as_slice()),
        (10, b"".as_slice()),
    ] {
        let proactor = new_platform_proactor().unwrap();
        let handle = proactor.handle();
        let file = open_data_file(&fixture.0).unwrap();
        let weak_file = Arc::downgrade(&file);
        let (send, receive) = mpsc::sync_channel(1);
        let completion_handle = handle.clone();
        let busy_reader = reader.clone();
        let operation = reader
            .read(&handle, file, offset, move |result| {
                assert!(busy_reader.is_busy());
                let bytes = result.unwrap();
                send.send((bytes.to_vec(), bytes.as_ptr() as usize))
                    .unwrap();
                completion_handle.stop().unwrap();
            })
            .unwrap();
        assert!(reader.is_busy());
        assert!(weak_file.upgrade().is_some());
        let other_file = open_data_file(&fixture.0).unwrap();
        assert_eq!(
            reader
                .read(&handle, other_file, 0, |_| panic!("busy read admitted"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        let _deadline = HostDeadline::new(&handle);
        let _operation = operation; // retained for callers that request cancel_io
        proactor.run_until_stopped().unwrap();
        let (actual, address) = receive
            .try_recv()
            .expect("read did not complete before deadline");
        assert_eq!(actual, expected);
        assert!(!reader.is_busy());
        assert!(weak_file.upgrade().is_none());
        if let Some(first) = first_address {
            assert_eq!(address, first, "buffer was reallocated");
        } else {
            first_address = Some(address);
        }
    }
}

#[test]
fn proactor_loaded_ssml_is_parsed_on_owner_after_completion() {
    use espeak_ng_rs::ssml::{self, Attribute, Wide};
    let fixture = Fixture::new();
    std::fs::write(&fixture.0, b" name='/Alice Bob' time='2S' /").unwrap();
    let reader = DataReader::new(64).unwrap();
    let host = new_platform_proactor().unwrap();
    let handle = host.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    reader
        .read(
            &handle,
            open_data_file(&fixture.0).unwrap(),
            0,
            move |result| {
                let source = result.unwrap();
                let mut bytes = [0u8; 64];
                bytes[..source.len()].copy_from_slice(source);
                send.send(bytes).unwrap();
                stop.stop().unwrap();
            },
        )
        .unwrap();
    let _deadline = HostDeadline::new(&handle);
    host.run_until_stopped().unwrap();
    let bytes = receive.try_recv().unwrap();
    let units = bytes.map(u32::from);
    let space = |c| matches!(c, 9..=13 | 32);
    let Some(Attribute::Value(name)) =
        ssml::attribute(Wide::U32(&units), 1, b"name", space).unwrap()
    else {
        panic!("name")
    };
    let mut output = [0xa5; 40];
    let length = ssml::copy_plan(
        Wide::U32(&units[name..]),
        units[name - 1],
        output.len(),
        space,
    )
    .unwrap()
    .write(&mut output)
    .unwrap();
    assert_eq!(&output[..length + 1], b"/Alice Bob\0");
    assert_eq!(output[length + 1], 0xa5);
    let Some(Attribute::Value(time)) =
        ssml::attribute(Wide::U32(&units), 1, b"time", space).unwrap()
    else {
        panic!("time")
    };
    assert_eq!(
        ssml::attribute_number(Some(Wide::U32(&units[time..])), 0, true),
        Ok(2000)
    );
    use espeak_ng_rs::ssml_parameters::{self, Frame, PARAMETERS, STACK};
    let mut frames = [Frame {
        kind: 0,
        values: [-1; PARAMETERS],
    }; STACK];
    frames[0].values[1] = 100;
    let mut count = 1;
    let nested = ssml_parameters::push(&mut frames, &mut count, 3).unwrap();
    frames[nested].values[1] = 200;
    let base = ssml_parameters::parameters(&frames[..1], &[-1; PARAMETERS], 0, 0).unwrap();
    let nested = ssml_parameters::parameters(&frames[..count], &base.values, 0, 0).unwrap();
    assert_eq!(nested.publish_commands(&mut output), Ok(5));
    assert_eq!(&output[..6], b"\x01200S\0");
    let closed = ssml_parameters::pop(&frames[..count], 35, &nested.values, 0, 0).unwrap();
    assert_eq!(closed.count, 1);
    assert_eq!(closed.publish_commands(&mut output), Ok(5));
    assert_eq!(&output[..6], b"\x01100S\0");
    use espeak_ng_rs::ssml_voice;
    let mut voice_frame = ssml_voice::Frame {
        kind: 2,
        variant: 0,
        gender: 0,
        age: 0,
        name: [0; 40],
        language: [0; 20],
    };
    voice_frame.name[..length].copy_from_slice(b"/Alice Bob");
    voice_frame.language[..2].copy_from_slice(b"en");
    let selected = ssml_voice::choice(&[voice_frame], b"\x05en-gb\0\x08en\0\0", &[0; 40], |name| {
        assert_eq!(&name[..length], b"/Alice Bob");
        let mut identifier = [0; 40];
        identifier[..6].copy_from_slice(b"gmw/en");
        Ok(Some(identifier))
    })
    .unwrap();
    assert_eq!(&selected.language[..6], b"en-gb\0");
    assert_eq!(&selected.identifier[..7], b"gmw/en\0");
    let variant = ssml_voice::base_variant(b"gmw/en", selected.gender as u8, 1, b"m2").unwrap();
    assert_eq!(&variant[..10], b"gmw/en+m2\0");
    let mut prosody = [0u32; 10];
    for (unit, byte) in prosody.iter_mut().zip(b"+12st'") {
        *unit = u32::from(*byte);
    }
    assert_eq!(
        espeak_ng_rs::ssml_prosody::parameter(3, Wide::U32(&prosody), 100, 50, 46, space),
        Ok(100)
    );
    assert!(!reader.is_busy());
}

#[test]
fn invalid_chunk_sizes_are_rejected_before_allocation() {
    assert!(DataReader::new(0).is_err());
    assert!(DataReader::new(1024 * 1024 + 1).is_err());
}

#[test]
fn proactor_loaded_sound_icon_owns_pcm_after_completion() {
    let fixture = Fixture::new();
    let mut wave = [0_u8; 48];
    wave[20..24].copy_from_slice(&0x10001_u32.to_le_bytes());
    wave[24..28].copy_from_slice(&22050_u32.to_le_bytes());
    wave[28..32].copy_from_slice(&44100_u32.to_le_bytes());
    wave[40..44].copy_from_slice(&4_u32.to_le_bytes());
    wave[44..].copy_from_slice(&[2, 0, 3, 0]);
    std::fs::write(&fixture.0, wave).unwrap();
    let reader = DataReader::new(128).unwrap();
    let host = new_platform_proactor().unwrap();
    let handle = host.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    reader
        .read(
            &handle,
            open_data_file(&fixture.0).unwrap(),
            0,
            move |result| {
                send.send(result.unwrap().to_vec()).unwrap();
                stop.stop().unwrap();
            },
        )
        .unwrap();
    let _deadline = HostDeadline::new(&handle);
    host.run_until_stopped().unwrap();
    let bytes = receive.try_recv().unwrap();
    // Parsing/alignment belongs to the owner after completion, not its callback.
    let mut icons = espeak_ng_rs::sound_icons::Catalog::new(128).unwrap();
    let index = icons.define(33, b"host.wav").unwrap();
    assert_eq!(icons.resident(b"host.wav", &bytes, 22050).unwrap(), index);
    let address = icons.icon(index).unwrap().bytes.as_ptr();
    for _ in 0..50 {
        assert_eq!(icons.resident(b"host.wav", &bytes, 22050).unwrap(), index);
    }
    let icon = icons.icon(index).unwrap();
    assert_eq!(icon.samples, 2);
    assert_eq!(icon.bytes[44..], [2, 0, 3, 0]);
    assert_eq!(icon.bytes.as_ptr(), address);
    assert_eq!((address as usize + 44) % 2, 0);
    let mut pcm = [0; 4];
    pcm.copy_from_slice(&icon.bytes[44..]);
    espeak_ng_rs::mbrola_output::scale_pcm(&mut pcm, 80).unwrap();
    assert_eq!(pcm, [4, 0, 6, 0]);
    assert_eq!(icon.bytes[44..], [2, 0, 3, 0]);
}

#[test]
fn proactor_loaded_voice_configures_native_acoustics_outside_completion() {
    use espeak_ng_rs::voice::{Voice, DEFAULT_TONE};
    let fixture = Fixture::new();
    let configuration = b"name native\nlanguage en 5\npitch 100 140\nformant 2 90 80 120\nbreath 10 20\nklatt 1 2 3 4 5 60\nspeed 110\nstressLength 160 170\nnumbers 2 3 33\nintonation 9\nreplace 1 a b\nmbrola en1 table 16000\n";
    std::fs::write(&fixture.0, configuration).unwrap();
    // Request discovery/opening belongs to initialization, before host I/O.
    let request = espeak_ng_rs::voice_request::Request::prepare(
        b"",
        Some(fixture.0.to_str().unwrap().as_bytes()),
        16,
        if cfg!(windows) { b'\\' } else { b'/' },
        4096,
        |path| {
            std::fs::metadata(std::str::from_utf8(path).unwrap())
                .map_or(-1, |metadata| metadata.len() as i64)
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(request.path(), fixture.0.to_str().unwrap().as_bytes());
    let reader = DataReader::new(256).unwrap();
    let proactor = new_platform_proactor().unwrap();
    let handle = proactor.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    reader
        .read(
            &handle,
            open_data_file(std::path::Path::new(
                std::str::from_utf8(request.path()).unwrap(),
            ))
            .unwrap(),
            0,
            move |result| {
                send.send(result.unwrap().to_vec()).unwrap();
                stop.stop().unwrap();
            },
        )
        .unwrap();
    let _deadline = HostDeadline::new(&handle);
    proactor.run_until_stopped().unwrap();
    let bytes = receive.try_recv().unwrap();
    assert_eq!(bytes, configuration);
    let mut voice = Voice::default();
    let mut points = DEFAULT_TONE;
    let (mut fast, rates) = voice.reset(22050, &mut points).unwrap();
    let metadata = espeak_ng_rs::voice_selection::Metadata::parse(&bytes).unwrap();
    let description = metadata.view(b"native/id").unwrap();
    assert_eq!(description.name, b"native");
    assert_eq!(description.languages, b"\x05en\0\0");
    let mut catalogue = espeak_ng_rs::voice_catalog::Workspace::new(1).unwrap();
    let mut owned = espeak_ng_rs::voice_storage::Catalog::new(1).unwrap();
    assert!(owned
        .insert(b"native/id", &bytes, false, |_, _| {})
        .unwrap());
    for _ in 0..50 {
        let selected = catalogue
            .select(
                &owned,
                espeak_ng_rs::voice_catalog::Properties {
                    language: Some(b"en"),
                    variant: 3,
                    ..Default::default()
                },
                false,
                b'/',
                b"en",
            )
            .unwrap()
            .unwrap();
        assert!(selected.found);
        assert_eq!(selected.index, 0);
        assert_eq!(&selected.suffix[..3], b"m3\0");
    }
    let mut language = espeak_ng_rs::language::Language::new(&description.languages[1..3]).unwrap();
    let baseline = language.options;
    assert!(language.letters().is_letter('a' as u32, 0));
    assert_eq!(language.dictionary(), b"en");
    let options = &mut language.options;
    let mut current = espeak_ng_rs::voice_current::Current::default();
    let mut active = current
        .prepare(
            request.path(),
            b"en",
            false,
            description.gender,
            description.age,
            &voice.language,
        )
        .unwrap();
    let phonemes = [
        espeak_ng_rs::phoneme::Phoneme {
            mnemonic: u32::from(b'a'),
            code: 10,
            ..Default::default()
        },
        espeak_ng_rs::phoneme::Phoneme {
            mnemonic: u32::from(b'b'),
            code: 11,
            ..Default::default()
        },
    ];
    let table = phonemes.each_ref().map(Some);
    let mut replacements = [espeak_ng_rs::voice_backend::Replacement::default(); 60];
    let mut replacement_count = 0;
    struct VoiceHost<'a> {
        options: &'a mut espeak_ng_rs::language_options::Options,
        replacements: &'a mut [espeak_ng_rs::voice_backend::Replacement; 60],
        count: &'a mut usize,
        table: [Option<&'a espeak_ng_rs::phoneme::Phoneme>; 2],
        speed_updates: usize,
        speed: espeak_ng_rs::speed::State,
        other: usize,
        table_changes: usize,
        backend_requests: usize,
        final_steps: usize,
    }
    impl espeak_ng_rs::voice_load::Host for VoiceHost<'_> {
        fn directive(
            &mut self,
            voice: &mut Voice,
            fast: &mut i32,
            _: &espeak_ng_rs::voice_setup::Setup,
            _: &[u8],
            value: &[u8],
            action: espeak_ng_rs::voice_directive::Action,
        ) -> bool {
            use espeak_ng_rs::voice_directive::Action;
            match action {
                Action::Metadata(_) => {}
                Action::Replacement(effect) => {
                    if effect == espeak_ng_rs::voice_setup::Effect::SelectPhonemes {
                        self.table_changes += 1;
                    }
                    espeak_ng_rs::voice_backend::replace(
                        self.replacements,
                        self.count,
                        value,
                        |word| espeak_ng_rs::phoneme::code(self.table, word),
                    )
                    .unwrap();
                }
                Action::Mbrola(request) => {
                    assert_eq!(request.sample_rate, 16000);
                    self.backend_requests += 1;
                }
                Action::LanguageOption(key) => self
                    .options
                    .apply(key, value, &mut espeak_ng_rs::language_options::Tunes(&[]))
                    .unwrap(),
                Action::Acoustics { update_speed } => {
                    self.speed_updates += usize::from(update_speed);
                    if update_speed {
                        self.speed.factors.fast_settings = *fast;
                        let effects = self.speed.configure(voice, 175, 175, 3, false).unwrap();
                        assert_eq!(effects.count, 0);
                    }
                }
                Action::Unknown => self.other += 1,
                Action::UnsupportedMbrola | Action::UnsupportedKlatt => {
                    panic!("fixture backends are enabled")
                }
            }
            true
        }
        fn invalid(&mut self, _: &[u8]) {
            panic!("fixture contains valid directives");
        }
        fn ensure_translator(&mut self, setup: &espeak_ng_rs::voice_setup::Setup) {
            assert!(setup.translator.starts_with(b"en\0"));
            self.final_steps += 1;
        }
        fn select_table(&mut self, name: &[u8]) -> i32 {
            assert_eq!(name, b"en");
            self.final_steps += 1;
            0
        }
        fn unknown_table(&mut self, _: &[u8]) {
            panic!("fixture table exists");
        }
        fn phoneme_index(&mut self, index: i32) {
            assert_eq!(index, 0);
            self.final_steps += 1;
        }
        fn dictionary(&mut self, name: &[u8], quiet: bool) -> bool {
            assert_eq!(name, b"en");
            assert!(!quiet);
            self.final_steps += 1;
            true
        }
    }
    let mut host = VoiceHost {
        options: &mut *options,
        replacements: &mut replacements,
        count: &mut replacement_count,
        table,
        speed_updates: 0,
        speed: espeak_ng_rs::speed::State::default(),
        other: 0,
        table_changes: 0,
        backend_requests: 0,
        final_steps: 0,
    };
    let mut stream = espeak_ng_rs::voice_reader::Reader::new(
        std::io::Cursor::new(&bytes),
        4096,
        espeak_ng_rs::voice_reader::TextMode::platform(),
    )
    .unwrap();
    let completion = espeak_ng_rs::voice_load::configure(
        Some(&mut stream),
        &mut active,
        &mut voice,
        &mut fast,
        espeak_ng_rs::voice_directive::Features {
            klatt: true,
            mbrola: true,
        },
        0,
        &mut host,
    )
    .unwrap();
    assert!(!completion.read_error);
    assert_eq!(host.other, 0);
    assert_eq!(host.table_changes, 1);
    assert_eq!(host.backend_requests, 1);
    assert_eq!(host.speed_updates, 1);
    assert_eq!(host.speed.lengths, [74, 68, 67]);
    assert_eq!(host.final_steps, 4);
    current.commit(&active);
    assert_eq!(current.name, active.name);
    assert_eq!(current.languages, active.languages);
    assert_eq!(
        &current.identifier[..request.path().len().min(39)],
        &request.path()[..request.path().len().min(39)]
    );
    assert_eq!(replacement_count, 1);
    assert_eq!(
        replacements[0],
        espeak_ng_rs::voice_backend::Replacement {
            old: 10,
            new: 11,
            flags: 1
        }
    );
    assert!(active.translator.starts_with(b"en\0"));
    assert!(active.name.starts_with(b"native\0"));
    assert_eq!(voice.pitch_base, (100 - 9) * 4096);
    assert_eq!(voice.pitch_range, 40 * 108);
    assert_eq!(voice.frequency[2], 230);
    assert_eq!(voice.breath[1], -10);
    assert_eq!(voice.breath[2], 20);
    assert_eq!(voice.klatt[5], 20);
    assert_eq!(voice.speed_percent, 110);
    let settings = voice.formant_settings(2, false, 0);
    assert_eq!(settings.formant_factor, 270);
    assert_eq!(settings.klatt, 1);
    let embedded = espeak_ng_rs::synthesis_parameters::Embedded {
        pitch: 50,
        tone: 0,
        range: 50,
    };
    let calibrated = espeak_ng_rs::synthesis_parameters::pitch(&voice, 10, 30, embedded).unwrap();
    assert_eq!(
        calibrated,
        espeak_ng_rs::synthesis_parameters::pitch(&voice, 30, 10, embedded).unwrap()
    );
    let general = espeak_ng_rs::synthesis_parameters::general_amplitude(100, 4).unwrap();
    let amplitude = espeak_ng_rs::synthesis_parameters::amplitude(
        256,
        16,
        general,
        voice.voiced_consonant_amplitude,
    )
    .unwrap();
    assert_eq!(amplitude.increment, 8192);
    assert_eq!(amplitude.value, 75);
    assert_eq!(rates[..6], [240, 170, 170, 170, 170, 170]);
    assert_eq!(
        options.stress_lengths[..3],
        [160, 170, baseline.stress_lengths[2]]
    );
    assert_eq!(options.numbers, baseline.numbers | 12);
    assert_eq!(options.numbers2, baseline.numbers2 | 2);
    assert_eq!(options.decimal_separator, i32::from(b','));
    assert_eq!(options.thousands_separator, 0);
    assert_eq!(options.intonation_group, 9);
    let context = espeak_ng_rs::suffix::Context {
        language: u32::from_be_bytes([0, 0, b'e', b'n']),
        added_character: 101,
        expect_verb: 0,
        signed_bytes: 1,
        preceding: [b' '; 4],
    };
    let mut word = *b"making   ";
    let stem =
        espeak_ng_rs::suffix::remove(&mut word, 0x903, &context, &language.letters()).unwrap();
    assert_eq!(&word[..5], b"make ");
    assert_eq!(stem.effects.expect_verb, 1);
    assert_eq!(&stem.original[..7], b"making\0");
    let pause = espeak_ng_rs::phoneme::Phoneme::default();
    let vowel = espeak_ng_rs::phoneme::Phoneme {
        code: 40,
        kind: 2,
        ..Default::default()
    };
    let mut table = [None; 256];
    table[0] = Some(&pause);
    table[40] = Some(&vowel);
    let settings =
        espeak_ng_rs::word_stress::Settings::from_options(context.language, &language.options, 0);
    let stressed =
        espeak_ng_rs::word_stress::assign(&[40, 40, 0], &table, 41, &settings, Some(0), -1, 0)
            .unwrap();
    assert_eq!(&stressed.phonemes[..stressed.length + 1], &[6, 40, 40, 0]);
    assert_eq!(stressed.previous, -1);
    let mut word = [0; 8];
    let mut counts = espeak_ng_rs::phoneme_word::Counts::default();
    assert!(
        espeak_ng_rs::phoneme_word::append(&mut word, &[40, 40, 0], &table, 41, &mut counts)
            .unwrap()
    );
    assert_eq!(
        counts,
        espeak_ng_rs::phoneme_word::Counts {
            vowels: 2,
            stressed: 2
        }
    );
    let changed = espeak_ng_rs::word_stress::change(&word, &table, settings.flags, 6).unwrap();
    assert_eq!(&changed.phonemes[..changed.length + 1], &[26, 40, 40, 0]);
}

#[test]
fn real_host_mbrola_bytes_install_and_map_after_completion() {
    let fixture = Fixture::new();
    let mut bytes = Vec::new();
    for word in [
        20u32,
        b'a' as u32,
        0,
        b'A' as u32,
        b'B' as u32,
        60,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ] {
        bytes.extend(word.to_le_bytes());
    }
    std::fs::write(&fixture.0, &bytes).unwrap();
    let reader = DataReader::new(bytes.len()).unwrap();
    let proactor = new_platform_proactor().unwrap();
    let handle = proactor.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let operation = reader
        .read(
            &handle,
            open_data_file(&fixture.0).unwrap(),
            0,
            move |result| {
                send.send(result.unwrap().to_vec()).unwrap();
                stop.stop().unwrap();
            },
        )
        .unwrap();
    let _deadline = HostDeadline::new(&handle);
    let _operation = operation;
    proactor.run_until_stopped().unwrap();
    let bytes = receive.try_recv().unwrap();
    let mut table = espeak_ng_rs::mbrola::Table::new(96).unwrap();
    table.replace(&bytes).unwrap();
    let current = espeak_ng_rs::phoneme::Phoneme {
        mnemonic: b'a' as u32,
        ..Default::default()
    };
    let result = espeak_ng_rs::mbrola::select(
        table.mappings(),
        &current,
        None,
        None,
        None,
        &espeak_ng_rs::mbrola::Context::default(),
    );
    assert_eq!(result.name, b'A' as i32);
    assert_eq!(result.second, b'B' as i32);
    assert_eq!(result.percent, 60);
    table.replace(&bytes).unwrap();
    let addresses = [table.mappings().as_ptr()];
    let reserved = table.reserved_bytes();
    for _ in 0..50 {
        table.replace(&bytes).unwrap();
        table.replace(&bytes).unwrap();
        assert_eq!(table.mappings().as_ptr(), addresses[0]);
        assert_eq!(table.reserved_bytes(), reserved);
    }
}

#[test]
fn cancellation_releases_the_loan_only_after_completion() {
    let fixture = Fixture::new();
    let reader = DataReader::new(4).unwrap();
    let proactor = new_platform_proactor().unwrap();
    let handle = proactor.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let operation = reader
        .read(
            &handle,
            open_data_file(&fixture.0).unwrap(),
            0,
            move |result| {
                // A completed read may win the cancellation race.
                send.send(result.map(|bytes| bytes.to_vec())).unwrap();
                stop.stop().unwrap();
            },
        )
        .unwrap();
    let _ = handle.cancel_io(operation);
    assert!(reader.is_busy());
    let _deadline = HostDeadline::new(&handle);
    proactor.run_until_stopped().unwrap();
    let result = receive
        .try_recv()
        .expect("missing cancellation/read completion");
    if let Ok(bytes) = result {
        assert_eq!(bytes, b"abcd");
    }
    assert!(!reader.is_busy());
    assert!(receive.try_recv().is_err());
}

#[test]
fn failed_read_returns_admission_and_a_usable_buffer() {
    let fixture = Fixture::new();
    let reader = DataReader::new(4).unwrap();
    let proactor = new_platform_proactor().unwrap();
    let handle = proactor.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    // Valid file, deliberately opened without read permission. Backends may
    // report this either at submission or as an asynchronous completion.
    let file = Arc::new(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&fixture.0)
            .unwrap(),
    );
    let submitted = reader.read(&handle, file, 0, move |result| {
        send.send(result.is_err()).unwrap();
        stop.stop().unwrap();
    });
    if submitted.is_ok() {
        let _deadline = HostDeadline::new(&handle);
        proactor.run_until_stopped().unwrap();
        assert!(receive.try_recv().expect("missing error completion"));
    } else {
        assert!(receive.try_recv().is_err());
    }
    assert!(!reader.is_busy());
    // A fresh host can use the same reader after either error path.
    let next = new_platform_proactor().unwrap();
    let stop = next.handle();
    reader
        .read(
            &next.handle(),
            open_data_file(&fixture.0).unwrap(),
            0,
            move |result| {
                assert_eq!(result.unwrap(), b"abcd");
                stop.stop().unwrap();
            },
        )
        .unwrap();
    let _deadline = HostDeadline::new(&next.handle());
    next.run_until_stopped().unwrap();
    assert!(!reader.is_busy());
}
