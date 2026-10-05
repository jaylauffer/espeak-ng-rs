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
fn invalid_chunk_sizes_are_rejected_before_allocation() {
    assert!(DataReader::new(0).is_err());
    assert!(DataReader::new(1024 * 1024 + 1).is_err());
}

#[test]
fn proactor_loaded_voice_configures_native_acoustics_outside_completion() {
    use espeak_ng_rs::voice::{Directives, Voice, DEFAULT_TONE};
    let fixture = Fixture::new();
    let configuration = b"name native\nlanguage en 5\npitch 100 140\nformant 2 90 80 120\nbreath 10 20\nklatt 1 2 3 4 5 60\nspeed 110\nstressLength 160 170\nnumbers 2 3 33\nintonation 9\n";
    std::fs::write(&fixture.0, configuration).unwrap();
    let reader = DataReader::new(256).unwrap();
    let proactor = new_platform_proactor().unwrap();
    let handle = proactor.handle();
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
    proactor.run_until_stopped().unwrap();
    let bytes = receive.try_recv().unwrap();
    assert_eq!(bytes, configuration);
    let mut voice = Voice::default();
    let mut points = DEFAULT_TONE;
    let (mut fast, rates) = voice.reset(22050, &mut points).unwrap();
    let mut speed_updates = 0;
    let mut other = 0;
    let metadata = espeak_ng_rs::voice_selection::Metadata::parse(&bytes).unwrap();
    let description = metadata.view(b"native/id").unwrap();
    assert_eq!(description.name, b"native");
    assert_eq!(description.languages, b"\x05en\0\0");
    let mut catalogue = espeak_ng_rs::voice_catalog::Workspace::new(1).unwrap();
    for _ in 0..50 {
        let selected = catalogue
            .select(
                &[description][..],
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
    let mut tunes = espeak_ng_rs::language_options::Tunes(&[]);
    for (key, value) in Directives::new(&bytes, 4096).unwrap() {
        if key == b"language" {
            continue;
        }
        if let Some(key) = espeak_ng_rs::language_options::key(key) {
            options.apply(key, value, &mut tunes).unwrap();
            continue;
        }
        match voice.apply(key, value, true, &mut fast).unwrap() {
            Some(speed) => speed_updates += usize::from(speed),
            None => other += 1,
        }
    }
    assert_eq!(other, 1);
    assert_eq!(speed_updates, 1);
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
