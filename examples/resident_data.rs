// SPDX-License-Identifier: GPL-3.0-or-later
use espeak_ng_rs::resident::{PreparedAssets, ResidentLoader, MAX_RESIDENT_BYTES};
use loadngo_proactor::new_platform_proactor;
use std::{io, path::PathBuf, sync::mpsc};

const HELP: &str = "Load and index resident eSpeak assets using loadngo's platform proactor.
This inspects native data; it does not synthesize audio.

Usage: resident_data --data-dir DIR [--dictionary NAME] [--max-bytes N]
  --data-dir DIR     Required: directory containing phontab, phondata and dictionaries.
  --dictionary NAME  Optional, repeatable: dictionary to load (default: en).
  --max-bytes N      Optional: combined resident byte limit (default: 134217728).
  --help, -h         Optional: print this help and exit successfully.

Example:
  cargo run --features proactor --example resident_data -- \
    --data-dir build-rust-core/espeak-ng-data --dictionary en --dictionary si";

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let mut root = None;
    let mut dictionaries = Vec::new();
    let mut limit = MAX_RESIDENT_BYTES;
    let mut args = args.iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => {
                root = Some(PathBuf::from(args.next().ok_or("--data-dir requires DIR")?))
            }
            "--dictionary" => {
                dictionaries.push(args.next().ok_or("--dictionary requires NAME")?.as_str())
            }
            "--max-bytes" => limit = args.next().ok_or("--max-bytes requires N")?.parse()?,
            _ => return Err(format!("unknown argument: {flag}").into()),
        }
    }
    let root = root.ok_or("--data-dir is required")?;
    if dictionaries.is_empty() {
        dictionaries.push("en");
    }
    // The standalone executable is the host. Applications supply their host.
    let plan = PreparedAssets::open(&root, &dictionaries, limit)?;
    let bytes = plan.total_bytes();
    let proactor = new_platform_proactor()?;
    let handle = proactor.handle();
    let completion_handle = handle.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let loader = ResidentLoader::new(64 * 1024)?;
    let _cancellation = loader.load(&handle, plan, move |result| {
        let _ = send.send(result);
        let _ = completion_handle.stop();
    })?;
    proactor.run_until_stopped()?;
    let data = receive
        .try_recv()
        .map_err(|_| io::Error::other("host stopped before resident load completed"))??;
    // Index after the host has drained, during initialization, not in a callback.
    let data = data.index()?;
    println!(
        "Loaded {bytes} resident bytes; {} phoneme tables; {} dictionaries; {} Hz",
        data.tables().tables().len(),
        data.dictionaries().count(),
        data.sample_rate()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}; pass --help for usage");
        std::process::exit(1);
    }
}
