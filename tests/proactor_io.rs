#![cfg(feature = "proactor")]
// SPDX-License-Identifier: GPL-3.0-or-later

use espeak_ng_rs::data_io::{open_data_file, DataReader};
use loadngo_proactor::{new_platform_proactor, CompletionKind};
use std::io;
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("espeak-data-{}-{stamp}", std::process::id()));
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
        let timeout_handle = handle.clone();
        handle
            .defer_for(
                Duration::from_secs(10),
                CompletionKind::Timer,
                0,
                move |_| {
                    timeout_handle.stop().unwrap();
                },
            )
            .unwrap();
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
    let timeout_handle = handle.clone();
    handle
        .defer_for(
            Duration::from_secs(10),
            CompletionKind::Timer,
            0,
            move |_| {
                timeout_handle.stop().unwrap();
            },
        )
        .unwrap();
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
        let timeout_handle = handle.clone();
        handle
            .defer_for(
                Duration::from_secs(10),
                CompletionKind::Timer,
                0,
                move |_| {
                    timeout_handle.stop().unwrap();
                },
            )
            .unwrap();
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
    let timeout_handle = next.handle();
    next.handle()
        .defer_for(
            Duration::from_secs(10),
            CompletionKind::Timer,
            0,
            move |_| {
                timeout_handle.stop().unwrap();
            },
        )
        .unwrap();
    next.run_until_stopped().unwrap();
    assert!(!reader.is_busy());
}
