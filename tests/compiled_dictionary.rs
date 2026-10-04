// SPDX-License-Identifier: GPL-3.0-or-later
use espeak_ng_rs::dictionary::Dictionary;

#[test]
#[ignore = "requires CMake-built language data; CTest runs this with ESPEAK_RUST_DATA_PATH"]
fn parse_every_real_compiled_dictionary() {
    let root = std::path::PathBuf::from(
        std::env::var_os("ESPEAK_RUST_DATA_PATH").expect("set ESPEAK_RUST_DATA_PATH"),
    );
    let mut count = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if !path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("_dict")
        {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let dict =
            Dictionary::parse(&bytes).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        assert!(!dict.rules().is_empty());
        dict.rule_index()
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        count += 1;
    }
    assert!(
        count >= 100,
        "expected the full multilingual data build, got {count}"
    );
    println!("Validated {count} real compiled dictionaries with the native Rust parser");
    let bytes = std::fs::read(root.join("phontab")).unwrap();
    let index = espeak_ng_rs::phoneme_data::TableIndex::parse(&bytes).unwrap();
    for number in 0..index.tables().len() {
        index.select(&bytes, number).unwrap();
    }
    assert_eq!(
        espeak_ng_rs::phoneme_data::sample_rate(&std::fs::read(root.join("phondata")).unwrap())
            .unwrap(),
        22050
    );
    println!(
        "Validated {} phoneme tables and inheritance chains",
        index.tables().len()
    );
}
