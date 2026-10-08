#![cfg(all(feature = "proactor", unix))]
// SPDX-License-Identifier: GPL-3.0-or-later

use espeak_ng_rs::mbrola_fill::{Fill, Read, Status};
use espeak_ng_rs::mbrola_process::{Audio, Session, AUDIO_CHUNK};
use espeak_ng_rs::mbrola_transport::COMMAND_CAPACITY;
use espeak_ng_rs::synthesis_loop::{run_on, Step};
use loadngo_proactor::{CompletionKind, CompletionPort, IoPort, Proactor, ProactorHandle};
use std::io;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(any(target_os = "linux", target_os = "android"))]
type TestPort = loadngo_proactor::EpollPort;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
type TestPort = loadngo_proactor::KqueuePort;

fn new_test_host() -> Proactor<TestPort> {
    Proactor::new(TestPort::new().unwrap())
}

fn fixture(mode: &str) -> Arc<Session> {
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/mbrola_stdio.py"
        ))
        .arg(mode);
    Arc::new(Session::spawn_command(command).unwrap())
}

#[derive(Default)]
struct Trace {
    sending: bool,
    reading: bool,
    errors: bool,
    audio_eof: bool,
    error_eof: bool,
    audio_error: Option<io::ErrorKind>,
    sent: usize,
    pcm: Vec<u8>,
    address: Option<usize>,
    renderer: Option<Rendered>,
    waiter: Option<Arc<dyn Fn() -> io::Result<bool> + Send + Sync>>,
}

// Requests can outlive one drive call (ordinary flush preserves the stream).
// Publish into the persistent owner, then wake its current waiter rather
// than a stale capability from the run that originally submitted the I/O.
fn notify(trace: &Arc<Mutex<Trace>>) {
    let waiter = trace.lock().unwrap().waiter.clone();
    if let Some(waiter) = waiter {
        waiter().unwrap();
    }
}
struct Deadline<P: CompletionPort>(Arc<Mutex<Option<ProactorHandle<P>>>>);
impl<P: CompletionPort> Deadline<P> {
    fn new(handle: &ProactorHandle<P>) -> Self {
        let owner = Arc::new(Mutex::new(Some(handle.clone())));
        let timer = Arc::clone(&owner);
        handle
            .defer_for(
                Duration::from_secs(10),
                CompletionKind::Timer,
                0,
                move |_| {
                    if let Some(handle) = timer.lock().unwrap().as_ref() {
                        handle.stop().unwrap();
                    }
                },
            )
            .unwrap();
        Self(owner)
    }
}
impl<P: CompletionPort> Drop for Deadline<P> {
    fn drop(&mut self) {
        self.0.lock().unwrap().take();
    }
}

struct Rendered {
    cursor: Fill,
    buffer: [u8; 256],
    milliseconds: i32,
    ended: bool,
}
impl Rendered {
    fn new(reference_bytes: usize) -> Self {
        // A test-only upper sample budget, derived from the independent
        // oracle, rather than a prediction of a flush's completion/length.
        // End is acknowledged exclusively by Audio::Eof after input closure.
        let milliseconds = i32::try_from(reference_bytes / 2 * 1000 / 22050 + 2).unwrap();
        let mut result = Self {
            cursor: Fill::default(),
            buffer: [0; 256],
            milliseconds,
            ended: false,
        };
        assert_eq!(
            result
                .cursor
                .fill(&mut result.buffer, 22050, milliseconds, false, 40, |_| {
                    Read::Pending
                })
                .unwrap()
                .status,
            Status::Pending
        );
        result
    }
    fn progress(&mut self, mut source: &[u8], collected: &mut Vec<u8>) {
        while !source.is_empty() {
            let outcome = self
                .cursor
                .fill(
                    &mut self.buffer,
                    22050,
                    self.milliseconds,
                    true,
                    40,
                    |target| {
                        let length = source.len().min(target.len());
                        target[..length].copy_from_slice(&source[..length]);
                        source = &source[length..];
                        Read::Samples(length / 2)
                    },
                )
                .unwrap();
            assert_eq!(outcome.status, Status::More);
            assert!(outcome.bytes > 0);
            collected.extend_from_slice(&self.buffer[..outcome.bytes]);
        }
        // A completion that has exhausted its loan is pending until another
        // fresh completion. Do not read again, infer idle, or issue a timer.
        let pending = self
            .cursor
            .fill(&mut self.buffer, 22050, self.milliseconds, true, 40, |_| {
                Read::Pending
            })
            .unwrap();
        assert_eq!(pending.status, Status::Pending);
        assert_eq!(pending.bytes, 0);
    }
    fn end(&mut self) {
        let end = self
            .cursor
            .fill(&mut self.buffer, 22050, self.milliseconds, true, 40, |_| {
                Read::End
            })
            .unwrap();
        assert_eq!(end.status, Status::End);
        assert_eq!(end.bytes, 0);
        self.ended = true;
    }
}

// Native synthesis waits on completions. The fixed watchdog is test-only;
// there is no timer, thread, short sleep or status polling in the driver.
fn drive<P: IoPort>(
    session: &Arc<Session>,
    host: &Proactor<P>,
    trace: &Arc<Mutex<Trace>>,
    finish_input: bool,
    until: impl Fn(&Trace) -> bool + Send + 'static,
) {
    let _deadline = Deadline::new(&host.handle());
    let session = Arc::clone(session);
    let trace = Arc::clone(trace);
    let mut bound = false;
    run_on(host, move |wake| {
        let handle = wake.handle().unwrap();
        let mut state = trace.lock().unwrap();
        if !bound {
            let resume = wake.clone();
            state.waiter = Some(Arc::new(move || resume.wake()));
            bound = true;
        }
        if until(&state) {
            return Step::Done;
        }
        if !state.sending && session.pending() != 0 {
            let result = Arc::clone(&trace);
            session
                .send_next(&handle, move |r| {
                    let mut state = result.lock().unwrap();
                    let count = r.unwrap();
                    assert!(count > 0 && count <= AUDIO_CHUNK);
                    state.sent += count;
                    state.sending = false;
                    drop(state);
                    notify(&result);
                })
                .unwrap();
            state.sending = true;
        }
        if finish_input && !state.sending && session.pending() == 0 {
            session.finish_input().unwrap();
        }
        if !state.reading && !state.audio_eof && state.audio_error.is_none() {
            let result = Arc::clone(&trace);
            session
                .read_audio(&handle, move |r| {
                    let mut state = result.lock().unwrap();
                    state.reading = false;
                    match r {
                        Ok(Audio::Progress { sample_rate, bytes }) => {
                            assert_eq!(bytes.len() % 2, 0);
                            assert!(sample_rate.is_none() || sample_rate == Some(22050));
                            let address = bytes.as_ptr() as usize;
                            if let Some(first) = state.address {
                                assert_eq!(address, first, "audio loan moved");
                            } else {
                                state.address = Some(address);
                            }
                            let Trace { pcm, renderer, .. } = &mut *state;
                            if let Some(renderer) = renderer {
                                renderer.progress(bytes, pcm);
                            } else {
                                pcm.extend_from_slice(bytes);
                            }
                        }
                        Ok(Audio::Eof) => {
                            if let Some(renderer) = &mut state.renderer {
                                renderer.end();
                            }
                            state.audio_eof = true;
                        }
                        Err(error) => state.audio_error = Some(error.kind()),
                    }
                    drop(state);
                    notify(&result);
                })
                .unwrap();
            state.reading = true;
        }
        if !state.errors && !state.error_eof {
            let result = Arc::clone(&trace);
            session
                .read_errors(&handle, move |r| {
                    let mut state = result.lock().unwrap();
                    state.errors = false;
                    state.error_eof = !r.unwrap();
                    drop(state);
                    notify(&result);
                })
                .unwrap();
            state.errors = true;
        }
        drop(state);
        Step::Pending
    })
    .unwrap();
}

#[test]
fn concurrent_stdio_drains_backpressure_and_reuses_audio_storage() {
    check_concurrent_stdio(new_test_host());
    #[cfg(target_os = "linux")]
    if let Ok(port) = loadngo_proactor::IoUringPort::new() {
        check_concurrent_stdio(Proactor::new(port));
    } else {
        eprintln!("io_uring unavailable; epoll coverage remains mandatory");
    }
}

fn check_concurrent_stdio<P: IoPort>(host: Proactor<P>) {
    let session = fixture("flood");
    let trace = Arc::new(Mutex::new(Trace::default()));
    let command: Vec<u8> = (0..COMMAND_CAPACITY).map(|n| (n * 73) as u8).collect();
    session.queue(&command).unwrap();
    assert_eq!(session.pending(), command.len());
    assert_eq!(
        session
            .queue(&vec![0; COMMAND_CAPACITY + 1])
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        session.queue(b"overflow").unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        session.finish_input().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    drive(&session, &host, &trace, true, |s| {
        s.audio_eof && s.error_eof
    });
    let state = trace.lock().unwrap();
    assert_eq!(state.sent, command.len());
    assert!(state.audio_error.is_none());
    let expected: Vec<u8> = command
        .iter()
        .flat_map(|&byte| (u16::from(byte) * 257).to_le_bytes())
        .collect();
    assert_eq!(state.pcm, expected);
    assert_eq!(session.pending(), 0);
    let mut warning = [0; 160];
    let count = session.last_error(&mut warning);
    assert_eq!(&warning[..count], b"latest warning without newline");
    assert_eq!(
        session.queue(b"closed").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        session
            .read_audio(&host.handle(), |_| {})
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(
        session
            .read_errors(&host.handle(), |_| {})
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn ordinary_flush_preserves_child_and_never_reports_eof() {
    let session = fixture("stream");
    let id = session.id();
    let host = new_test_host();
    let trace = Arc::new(Mutex::new(Trace::default()));
    let mut expected = Vec::new();
    for command in [b"first".as_slice(), b"second"] {
        session.queue(command).unwrap();
        session.flush().unwrap();
        expected.extend(
            command
                .iter()
                .chain(b"\n#\n")
                .flat_map(|&b| (u16::from(b) * 257).to_le_bytes()),
        );
        let expected_length = expected.len();
        drive(&session, &host, &trace, false, move |s| {
            s.pcm.len() == expected_length && !s.sending
        });
        let state = trace.lock().unwrap();
        assert!(!state.audio_eof);
        assert_eq!(state.pcm, expected);
        assert_eq!(session.id(), id);
    }
    session.finish_input().unwrap();
    drive(&session, &host, &trace, false, |s| {
        s.audio_eof && s.error_eof
    });
    assert_eq!(trace.lock().unwrap().pcm, expected);
}

#[test]
fn invalid_or_truncated_output_reports_terminal_audio_error() {
    for (mode, kind) in [
        ("invalid", io::ErrorKind::InvalidData),
        ("truncated", io::ErrorKind::UnexpectedEof),
        ("odd", io::ErrorKind::UnexpectedEof),
    ] {
        let session = fixture(mode);
        let host = new_test_host();
        let trace = Arc::new(Mutex::new(Trace::default()));
        drive(&session, &host, &trace, false, |s| {
            s.audio_error.is_some() && s.error_eof
        });
        assert_eq!(trace.lock().unwrap().audio_error, Some(kind));
        assert_eq!(
            session
                .read_audio(&host.handle(), |_| {})
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}

#[test]
fn early_child_exit_reports_send_error_without_losing_pending_input() {
    let session = fixture("exit");
    let host = new_test_host();
    let trace = Arc::new(Mutex::new(Trace::default()));
    // The fixture closes stdin before producing stderr EOF. This producer
    // ordering fences the send without a waitpid polling loop or guessed delay.
    drive(&session, &host, &trace, false, |s| {
        s.error_eof && s.audio_error.is_some()
    });
    session.queue(b"dead peer").unwrap();
    let result = Arc::new(Mutex::new(None));
    let done = Arc::clone(&result);
    session
        .send_next(&host.handle(), move |r| *done.lock().unwrap() = Some(r))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while result.lock().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        host.run_once_until(deadline).unwrap();
    }
    let kind = result.lock().unwrap().take().unwrap().unwrap_err().kind();
    assert!(matches!(
        kind,
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
    ));
    assert_eq!(session.pending(), b"dead peer".len());
}

#[test]
fn output_eof_does_not_imply_input_closed_or_child_exited() {
    let mut session = fixture("output-eof");
    let host = new_test_host();
    let trace = Arc::new(Mutex::new(Trace::default()));
    drive(&session, &host, &trace, false, |s| {
        s.error_eof && s.audio_error.is_some()
    });
    // This child has closed both outputs but still blocks on its input.
    // One event-triggered status query proves EOF is not a process-exit event.
    assert!(Arc::get_mut(&mut session)
        .unwrap()
        .try_wait()
        .unwrap()
        .is_none());
    session.queue(b"x").unwrap();
    let result = Arc::new(Mutex::new(None));
    let done = Arc::clone(&result);
    session
        .send_next(&host.handle(), move |r| *done.lock().unwrap() = Some(r))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while result.lock().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        host.run_once_until(deadline).unwrap();
    }
    assert_eq!(result.lock().unwrap().take().unwrap().unwrap(), 1);
    session.finish_input().unwrap();
}

#[test]
fn cancelled_read_and_owner_drop_still_complete_each_loan_once() {
    let session = fixture("stall");
    let host = new_test_host();
    let handle = host.handle();
    let result = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::clone(&result);
    let operation = session
        .read_audio(&handle, move |r| {
            done.lock().unwrap().push(r.err().unwrap().kind());
        })
        .unwrap();
    assert_eq!(
        session
            .read_audio(&handle, |_| panic!("second loan admitted"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    handle.cancel_io(operation).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while result.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline);
        host.run_once_until(deadline).unwrap();
    }
    assert_eq!(result.lock().unwrap().len(), 1);
    let done = Arc::clone(&result);
    session
        .read_audio(&handle, move |r| {
            done.lock().unwrap().push(r.err().unwrap().kind());
        })
        .unwrap();
    // Drop kills/reaps on the owner; callbacks retain the sockets/buffer state.
    drop(session);
    while result.lock().unwrap().len() != 2 {
        assert!(Instant::now() < deadline);
        host.run_once_until(deadline).unwrap();
    }
    assert_eq!(result.lock().unwrap()[1], io::ErrorKind::UnexpectedEof);
}

#[test]
fn borrowed_audio_stays_busy_until_callback_returns_even_on_unwind() {
    let session = fixture("stream");
    let host = new_test_host();
    let handle = host.handle();
    let callback_handle = handle.clone();
    let owner = Arc::downgrade(&session);
    session
        .read_audio(&handle, move |r| {
            assert!(matches!(
                r,
                Ok(Audio::Progress {
                    sample_rate: Some(22050),
                    ..
                })
            ));
            assert_eq!(
                owner
                    .upgrade()
                    .unwrap()
                    .read_audio(&callback_handle, |_| {})
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            panic!("exercise loan unwind");
        })
        .unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline);
            host.run_once_until(deadline).unwrap();
        }
    }));
    assert!(caught.is_err());
    // The borrowed storage is returned by its guard even when user code
    // panics. A new read remains possible, and shutdown drains it normally.
    let trace = Arc::new(Mutex::new(Trace::default()));
    session.finish_input().unwrap();
    drive(&session, &host, &trace, false, |s| {
        s.audio_eof && s.error_eof
    });
}

#[test]
#[ignore = "set ESPEAK_MBROLA_PROGRAM, ESPEAK_MBROLA_VOICE, ESPEAK_MBROLA_PHO and ESPEAK_MBROLA_WAV"]
fn upstream_mbrola_pcm_matches_direct_file_synthesis() {
    let program = std::env::var_os("ESPEAK_MBROLA_PROGRAM").expect("MBROLA program");
    let voice = std::env::var_os("ESPEAK_MBROLA_VOICE").expect("MBROLA voice");
    let source = std::fs::read(std::env::var_os("ESPEAK_MBROLA_PHO").expect("phonemes")).unwrap();
    let expected =
        std::fs::read(std::env::var_os("ESPEAK_MBROLA_WAV").expect("reference WAV")).unwrap();
    let session = Arc::new(Session::spawn(program.as_ref(), voice.as_ref(), 1.0).unwrap());
    let host = new_test_host();
    let trace = Arc::new(Mutex::new(Trace {
        renderer: Some(Rendered::new(expected.len() - 44)),
        pcm: Vec::with_capacity(expected.len() - 44),
        ..Trace::default()
    }));
    session.queue(&source).unwrap();
    session.flush().unwrap();
    drive(&session, &host, &trace, true, |s| {
        s.audio_eof && s.error_eof
    });
    let state = trace.lock().unwrap();
    assert!(state.audio_error.is_none());
    assert!(!state.pcm.is_empty());
    assert_eq!(state.pcm, expected[44..]);
    assert!(state.renderer.as_ref().unwrap().ended);
}

#[test]
#[ignore = "set ESPEAK_MBROLA_PROGRAM, ESPEAK_MBROLA_VOICE and ESPEAK_MBROLA_PHO"]
fn upstream_flushes_preserve_one_childs_pcm_history() {
    let program = std::env::var_os("ESPEAK_MBROLA_PROGRAM").expect("MBROLA program");
    let voice = std::env::var_os("ESPEAK_MBROLA_VOICE").expect("MBROLA voice");
    let source = std::fs::read(std::env::var_os("ESPEAK_MBROLA_PHO").expect("phonemes")).unwrap();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("espeak-mbr-oracle-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch(root);
    let input = scratch.0.join("two-clauses.pho");
    let output = scratch.0.join("two-clauses.wav");
    let mut combined = Vec::new();
    for _ in 0..2 {
        combined.extend_from_slice(&source);
        combined.extend_from_slice(b"\n#\n");
    }
    std::fs::write(&input, combined).unwrap();
    // Independent official binary/file input oracle. This blocking command
    // is test initialization, outside the driver's completion dispatch.
    assert!(Command::new(&program)
        .args(["-e", "-v", "1"])
        .arg(&voice)
        .arg(input)
        .arg(&output)
        .status()
        .unwrap()
        .success());
    let reference = std::fs::read(output).unwrap();
    let session = Arc::new(Session::spawn(program.as_ref(), voice.as_ref(), 1.0).unwrap());
    let id = session.id();
    let host = new_test_host();
    let trace = Arc::new(Mutex::new(Trace {
        renderer: Some(Rendered::new(reference.len() - 44)),
        pcm: Vec::with_capacity(reference.len() - 44),
        ..Trace::default()
    }));
    session.queue(&source).unwrap();
    session.flush().unwrap();
    // Observe actual audio progress without interpreting it as a clause
    // completion acknowledgement. Submit more input to the same live child.
    let first_input_length = source.len() + 3;
    drive(&session, &host, &trace, false, move |s| {
        s.sent == first_input_length && !s.pcm.is_empty()
    });
    assert!(!trace.lock().unwrap().audio_eof);
    assert_eq!(session.id(), id);
    session.queue(&source).unwrap();
    session.flush().unwrap();
    drive(&session, &host, &trace, true, |s| {
        s.audio_eof && s.error_eof
    });
    let state = trace.lock().unwrap();
    assert!(state.audio_error.is_none());
    assert_eq!(state.sent, 2 * (source.len() + 3));
    assert_eq!(state.pcm, reference[44..]);
    assert!(state.renderer.as_ref().unwrap().ended);
}
