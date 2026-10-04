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
        phontab.extend_from_slice(&record);
        std::fs::write(root.join("phontab"), phontab).unwrap();
        std::fs::write(root.join("phondata"), [1, 72, 1, 0, 34, 86, 0, 0]).unwrap();
        std::fs::write(root.join("phonindex"), [1, 0, 2, 0]).unwrap();
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
        dict.extend_from_slice(&[7, 0]);
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
    assert_eq!(data.phonindex(), [1, 0, 2, 0]);
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
