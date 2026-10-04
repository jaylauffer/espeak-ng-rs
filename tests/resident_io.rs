#![cfg(feature = "proactor")]
// SPDX-License-Identifier: GPL-3.0-or-later
use espeak_ng_rs::{
    dictionary,
    resident::{PreparedAssets, ResidentLoader, MAX_RESIDENT_BYTES},
};
use loadngo_proactor::new_platform_proactor;
mod support;
use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use support::HostDeadline;

struct Fixture(PathBuf);
static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "espeak-resident-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut phontab = vec![1, 0, 0, 0, 1, 0, 0, 0];
        let mut name = [0; 32];
        name[0] = b'a';
        phontab.extend_from_slice(&name);
        let mut record = [0; 16];
        record[0] = b'a';
        record[10] = 42;
        record[8] = 1;
        record[14] = 24;
        record[11] = 2;
        phontab.extend_from_slice(&record);
        std::fs::write(root.join("phontab"), phontab).unwrap();
        std::fs::write(root.join("phondata"), [1, 72, 1, 0, 34, 86, 0, 0]).unwrap();
        std::fs::write(
            root.join("phonindex"),
            [0, 0, 0xf8, 0x0c, 2, 9, 0x84, 0x28, 0x11, 7, 1, 0],
        )
        .unwrap();
        std::fs::write(root.join("intonations"), []).unwrap();
        let mut dict = vec![0, 4, 0, 0, 0, 0, 0, 0];
        for bucket in 0..1024 {
            if bucket == dictionary::hash(b"cat") {
                dict.extend_from_slice(&[8, 3, b'c', b'a', b't', 42, 43, 0]);
            }
            dict.push(0);
        }
        let length = dict.len() as u32;
        dict[4..8].copy_from_slice(&length.to_le_bytes());
        // c followed by a language vowel chooses 42; otherwise fallback 43.
        dict.extend_from_slice(&[6, b'c', 0, 2, 17, b'A', 3, 42, 0, 3, 43, 0, 7, 0]);
        std::fs::write(root.join("en_dict"), dict).unwrap();
        Self(root)
    }
    fn plan(&self) -> PreparedAssets {
        PreparedAssets::open(&self.0, &["en"], MAX_RESIDENT_BYTES).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn load(
    fixture: &Fixture,
    plan: PreparedAssets,
    loader: &ResidentLoader,
    cancel: bool,
) -> io::Result<espeak_ng_rs::resident::ResidentBytes> {
    let host = new_platform_proactor().unwrap();
    let handle = host.handle();
    let completion = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let cancellation = loader
        .load(&handle, plan, move |result| {
            send.send(result).unwrap();
            completion.stop().unwrap();
        })
        .unwrap();
    assert!(loader.is_busy());
    assert_eq!(
        loader
            .clone()
            .load(&handle, fixture.plan(), |_| panic!("busy plan admitted"))
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    if cancel {
        // Runs after initial submission, while a chunk may be outstanding.
        handle.enqueue_work(move |_| cancellation.cancel()).unwrap();
    }
    let _deadline = HostDeadline::new(&handle);
    host.run_until_stopped().unwrap();
    assert!(!loader.is_busy());
    receive.try_recv().expect("resident load did not complete")
}

#[test]
fn assembled_assets_index_once_and_support_native_lookup() {
    let fixture = Fixture::new();
    let loader = ResidentLoader::new(3).unwrap();
    let data = load(&fixture, fixture.plan(), &loader, false)
        .unwrap()
        .index()
        .unwrap();
    assert_eq!(data.sample_rate(), 22050);
    assert_eq!(
        data.phonindex(),
        [0, 0, 0xf8, 0x0c, 2, 9, 0x84, 0x28, 0x11, 7, 1, 0]
    );
    assert!(data.intonations().is_empty());
    let selected = data.tables().select(data.phontab(), 0).unwrap();
    assert_eq!(
        espeak_ng_rs::phoneme_data::record(data.phontab(), selected[42])
            .unwrap()
            .code,
        42
    );
    assert_eq!(
        data.dictionary("en")
            .unwrap()
            .bucket(b"cat")
            .next()
            .unwrap()
            .phonemes,
        Some([42, 43].as_slice())
    );
    let match_result = data
        .dictionary("en")
        .unwrap()
        .lookup(
            b"cat",
            3,
            b"",
            &espeak_ng_rs::lookup::Context::default(),
            None,
        )
        .unwrap();
    assert_eq!(match_result.phonemes, Some([42, 43].as_slice()));
    assert_eq!(match_result.word_end, Some(0));

    struct RuleInputs<'a>(espeak_ng_rs::letters::LetterSet<'a>);
    impl espeak_ng_rs::rule_match::Environment for RuleInputs<'_> {
        fn is_letter(&mut self, code: u32, group: u8) -> bool {
            self.0.is_letter(code, group)
        }
        fn letter_group(&mut self, _: &[u8], _: usize, _: u8, _: bool) -> Option<usize> {
            None
        }
        fn prefix_flags(&mut self, _: &[u8]) -> [u32; 2] {
            [0; 2]
        }
    }
    let dict = data.dictionary("en").unwrap();
    let mut bits = [0; 256];
    bits[b'a' as usize] = 1;
    let mut environment = RuleInputs(espeak_ng_rs::letters::LetterSet {
        bits: &bits,
        offset: 0,
        groups: [None; 8],
    });
    let group = dict.rule_index().singles[b'c' as usize];
    let matched = dict
        .match_group(
            group,
            b" cat \0",
            1,
            1,
            &espeak_ng_rs::rule_match::Context::default(),
            &mut environment,
        )
        .unwrap();
    assert!(matched.points > 1);
    assert_eq!(matched.advance, 1);
    let offset = group + matched.phonemes.unwrap();
    assert_eq!(&dict.bytes()[offset..offset + 2], &[42, 0]);
    let fallback = dict
        .match_group(
            group,
            b" cbt \0",
            1,
            1,
            &espeak_ng_rs::rule_match::Context::default(),
            &mut environment,
        )
        .unwrap();
    assert_eq!(fallback.points, 1);
    assert_eq!(dict.bytes()[group + fallback.phonemes.unwrap()], 43);
    use espeak_ng_rs::phoneme_context::{Context, Entry, Settings, SliceStorage};
    let selected = data.tables().select(data.phontab(), 0).unwrap();
    let phoneme = espeak_ng_rs::phoneme_data::record(data.phontab(), selected[42]).unwrap();
    let program = data.phoneme_programs();
    let mut table = [None; 256];
    table[42] = Some(phoneme);
    let mut list = [Entry {
        phoneme: Some(phoneme),
        code: 42,
        stress: 4,
        word_stress: 4,
        ..Entry::default()
    }; 3];
    let settings = Settings {
        length: 3,
        current: 1,
        has_translator: 1,
        ..Settings::default()
    };
    let mut context = Context::new(
        program,
        settings,
        SliceStorage {
            list: &mut list,
            table: &table,
            previous_vowel: None,
        },
    )
    .unwrap();
    let interpreted = program.interpret(&phoneme, 0, true, &mut context).unwrap();
    assert_eq!(interpreted.parameters[10], 16);
    assert_eq!(interpreted.parameters[9], 2);
    assert_eq!(interpreted.parameters[7], 17);
    list[1].stress = 0;
    let mut context = Context::new(
        program,
        settings,
        SliceStorage {
            list: &mut list,
            table: &table,
            previous_vowel: None,
        },
    )
    .unwrap();
    assert_eq!(
        program
            .interpret(&phoneme, 0, true, &mut context)
            .unwrap()
            .parameters[7],
        0
    );
    assert!(dict
        .match_group(
            usize::MAX,
            b" cat \0",
            1,
            1,
            &espeak_ng_rs::rule_match::Context::default(),
            &mut environment
        )
        .is_err());
}

#[test]
fn cancelled_load_returns_admission_and_next_load_succeeds() {
    let fixture = Fixture::new();
    let loader = ResidentLoader::new(4).unwrap();
    assert_eq!(
        load(&fixture, fixture.plan(), &loader, true)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::Interrupted
    );
    assert!(load(&fixture, fixture.plan(), &loader, false)
        .unwrap()
        .index()
        .is_ok());
}

#[test]
fn shortened_asset_and_invalid_native_data_report_errors() {
    let fixture = Fixture::new();
    let loader = ResidentLoader::new(32).unwrap();
    let plan = fixture.plan();
    std::fs::write(fixture.0.join("phondata"), [1]).unwrap();
    assert_eq!(
        load(&fixture, plan, &loader, false).err().unwrap().kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(
        load(&fixture, fixture.plan(), &loader, false)
            .unwrap()
            .index()
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn resident_limits_and_invalid_dictionary_names_reject_before_submission() {
    let fixture = Fixture::new();
    assert!(PreparedAssets::open(&fixture.0, &[], 0).is_err());
    assert!(PreparedAssets::open(&fixture.0, &[], MAX_RESIDENT_BYTES + 1).is_err());
    assert!(PreparedAssets::open(&fixture.0, &["en"], 100).is_err());
    for names in [&["../en"][..], &["en", "en"][..], &[""][..]] {
        assert!(PreparedAssets::open(&fixture.0, names, MAX_RESIDENT_BYTES).is_err());
    }
}

#[test]
fn completion_can_start_the_next_plan_on_the_same_host() {
    let fixture = Fixture::new();
    let loader = ResidentLoader::new(64).unwrap();
    let host = new_platform_proactor().unwrap();
    let handle = host.handle();
    let second = fixture.plan();
    let next_loader = loader.clone();
    let next_handle = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    loader
        .load(&handle, fixture.plan(), move |result| {
            assert!(result.is_ok());
            assert!(!next_loader.is_busy());
            let stop = next_handle.clone();
            next_loader
                .load(&next_handle, second, move |result| {
                    send.send(result).unwrap();
                    stop.stop().unwrap();
                })
                .unwrap();
        })
        .unwrap();
    let _deadline = HostDeadline::new(&handle);
    host.run_until_stopped().unwrap();
    assert!(!loader.is_busy());
    assert!(receive.try_recv().unwrap().unwrap().index().is_ok());
}

#[test]
#[ignore = "requires CMake-built language data; CTest supplies ESPEAK_RUST_DATA_PATH"]
fn load_every_real_asset_through_the_platform_proactor() {
    let root = PathBuf::from(
        std::env::var_os("ESPEAK_RUST_DATA_PATH").expect("set ESPEAK_RUST_DATA_PATH"),
    );
    let mut dictionaries = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if let Some(name) = name.strip_suffix("_dict") {
            dictionaries.push(name.to_owned());
        }
    }
    dictionaries.sort();
    assert!(dictionaries.len() >= 100);
    let names: Vec<_> = dictionaries.iter().map(String::as_str).collect();
    let plan = PreparedAssets::open(&root, &names, MAX_RESIDENT_BYTES).unwrap();
    let total = plan.total_bytes();
    let host = new_platform_proactor().unwrap();
    let handle = host.handle();
    let stop = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let loader = ResidentLoader::new(64 * 1024).unwrap();
    loader
        .load(&handle, plan, move |result| {
            send.send(result).unwrap();
            stop.stop().unwrap();
        })
        .unwrap();
    let _deadline = HostDeadline::new(&handle);
    host.run_until_stopped().unwrap();
    let data = receive.try_recv().unwrap().unwrap().index().unwrap();
    assert!(!loader.is_busy());
    assert_eq!(data.phontab(), std::fs::read(root.join("phontab")).unwrap());
    assert_eq!(
        data.phondata(),
        std::fs::read(root.join("phondata")).unwrap()
    );
    assert_eq!(
        data.phonindex(),
        std::fs::read(root.join("phonindex")).unwrap()
    );
    assert_eq!(
        data.intonations(),
        std::fs::read(root.join("intonations")).unwrap()
    );
    for (name, dictionary) in data.dictionaries() {
        assert_eq!(
            dictionary.bytes(),
            std::fs::read(root.join(format!("{name}_dict"))).unwrap()
        );
    }
    assert_eq!(data.dictionaries().count(), dictionaries.len());
    println!("Proactor loaded {total} resident bytes, {} dictionaries and {} phoneme tables; every file byte matched", dictionaries.len(), data.tables().tables().len());
}
