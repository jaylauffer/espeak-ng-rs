//! Persistent Unix MBROLA stdio on the caller's loadngo completion port.
//!
//! Three socketpairs make stdin/stdout/stderr usable by `IoPort::send/recv`.
//! Each direction has one reusable loan. The owner admits commands before I/O
//! and consumes only acknowledged prefixes. Audio reads report progress, not
//! an inferred utterance boundary: upstream `#` has no completion reply.
//! Spawn and final process reaping belong to initialization/shutdown, outside
//! completion callbacks. Drive/drain the host after cancelling outstanding IDs.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::mbrola_transport::{sample_rate, Transport, COMMAND_CAPACITY};
use loadngo_proactor::{IoBuf, IoOpId, IoPort, IoResult, ProactorHandle};
use std::io;
use std::net::Shutdown;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};

pub const AUDIO_CHUNK: usize = 16 * 1024;
const ERROR_CHUNK: usize = 4096;

#[derive(Clone, Copy)]
enum Direction {
    Command,
    Audio,
    Error,
}

struct State {
    transport: Transport,
    command: Option<Vec<u8>>,
    audio: Option<Vec<u8>>,
    error: Option<Vec<u8>>,
    input_closed: bool,
    decoder: Decoder,
    error_eof: bool,
}
impl State {
    fn loan(&mut self, direction: Direction) -> &mut Option<Vec<u8>> {
        match direction {
            Direction::Command => &mut self.command,
            Direction::Audio => &mut self.audio,
            Direction::Error => &mut self.error,
        }
    }
}

fn buffer(direction: Direction) -> Vec<u8> {
    let length = match direction {
        Direction::Error => ERROR_CHUNK,
        _ => AUDIO_CHUNK,
    };
    // Audio needs one extra slot when a sample was split across completions.
    let mut bytes = Vec::with_capacity(length + usize::from(matches!(direction, Direction::Audio)));
    bytes.resize(length, 0);
    bytes
}

struct ReturnBuffer {
    state: Arc<Mutex<State>>,
    direction: Direction,
    bytes: Option<Vec<u8>>,
}
impl Drop for ReturnBuffer {
    fn drop(&mut self) {
        let mut bytes = self.bytes.take().unwrap_or_else(|| buffer(self.direction));
        bytes.resize(
            match self.direction {
                Direction::Error => ERROR_CHUNK,
                _ => AUDIO_CHUNK,
            },
            0,
        );
        *self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .loan(self.direction) = Some(bytes);
    }
}

/// Initialized little-endian sample pairs, borrowed only during completion.
/// A header fragment can yield progress with no rate or audio yet. Only `Eof`
/// proves the stdout stream ended; a normal flush does not end this stream.
pub enum Audio<'a> {
    Progress {
        sample_rate: Option<i32>,
        bytes: &'a [u8],
    },
    Eof,
}

struct Decoder {
    header: [u8; 44],
    header_length: usize,
    rate: Option<i32>,
    carry: Option<u8>,
    ended: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            header: [0; 44],
            header_length: 0,
            rate: None,
            carry: None,
            ended: false,
        }
    }
}

impl Decoder {
    fn decode<'a>(&mut self, bytes: &'a mut Vec<u8>) -> io::Result<Audio<'a>> {
        if bytes.is_empty() {
            self.ended = true;
            return if self.rate.is_some() && self.carry.is_none() {
                Ok(Audio::Eof)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated MBROLA WAV stream",
                ))
            };
        }
        let header = (44 - self.header_length).min(bytes.len());
        let start = self.header_length;
        self.header[start..start + header].copy_from_slice(&bytes[..header]);
        self.header_length += header;
        bytes.copy_within(header.., 0);
        bytes.truncate(bytes.len() - header);
        if self.header_length == 44 && self.rate.is_none() {
            match sample_rate(&self.header) {
                Ok(rate) => self.rate = Some(rate),
                Err(_) => {
                    self.ended = true;
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid MBROLA WAV header",
                    ));
                }
            }
        }
        if let Some(carry) = self.carry.take() {
            let length = bytes.len();
            bytes.push(0);
            bytes.copy_within(..length, 1);
            bytes[0] = carry;
        }
        if bytes.len() % 2 != 0 {
            self.carry = bytes.pop();
        }
        Ok(Audio::Progress {
            sample_rate: self.rate,
            bytes,
        })
    }
}

struct Pipes {
    input: UnixStream,
    audio: UnixStream,
    error: UnixStream,
}

/// One child/voice lifetime. Commands, stderr and PCM loans are independently
/// bounded. The caller's proactor owns every operation until its completion;
/// captured `Arc`s retain descriptors even if this owner is dropped meanwhile.
pub struct Session {
    child: Child,
    pipes: Arc<Pipes>,
    state: Arc<Mutex<State>>,
}

impl Session {
    pub fn spawn(program: &Path, voice: &Path, volume: f32) -> io::Result<Self> {
        if !volume.is_finite() || volume < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid MBROLA volume",
            ));
        }
        let mut command = Command::new(program);
        command
            .args(["-e", "-v"])
            .arg(volume.to_string())
            .arg(voice)
            .args(["-", "-.wav"]);
        Self::spawn_command(command)
    }

    /// Configure stdio for a command implementing MBROLA's WAV/phoneme protocol.
    /// Command construction/spawn is initialization work, not a paint callback.
    pub fn spawn_command(mut command: Command) -> io::Result<Self> {
        let (input, child_input) = UnixStream::pair()?;
        let (audio, child_audio) = UnixStream::pair()?;
        let (error, child_error) = UnixStream::pair()?;
        // The pinned loadngo send backends suppress SIGPIPE per operation.
        // No process-global signal disposition is changed here.
        command
            .stdin(Stdio::from(OwnedFd::from(child_input)))
            .stdout(Stdio::from(OwnedFd::from(child_audio)))
            .stderr(Stdio::from(OwnedFd::from(child_error)));
        let child = command.spawn()?;
        Ok(Self {
            child,
            pipes: Arc::new(Pipes {
                input,
                audio,
                error,
            }),
            state: Arc::new(Mutex::new(State {
                transport: Transport::new(COMMAND_CAPACITY).expect("fixed capacity"),
                command: Some(buffer(Direction::Command)),
                audio: Some(buffer(Direction::Audio)),
                error: Some(buffer(Direction::Error)),
                input_closed: false,
                decoder: Decoder::default(),
                error_eof: false,
            })),
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Whole-command admission. `WouldBlock` means retry after write progress;
    /// an individual command exceeding the fixed bound is invalid input.
    pub fn queue(&self, bytes: &[u8]) -> io::Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.input_closed {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if bytes.len() > COMMAND_CAPACITY {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        state
            .transport
            .queue(bytes)
            .map_err(|_| io::ErrorKind::WouldBlock.into())
    }

    pub fn flush(&self) -> io::Result<()> {
        self.queue(b"\n#\n")
    }

    pub fn pending(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .transport
            .pending()
    }

    /// Submit one bounded FIFO extent. The completion restores the command
    /// loan before `done`, allowing another send from that callback. No retry
    /// is scheduled without an actual completion. Empty/busy is `WouldBlock`.
    pub fn send_next<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        done: impl FnOnce(io::Result<usize>) + Send + 'static,
    ) -> io::Result<IoOpId> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.input_closed {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if state.transport.pending() == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let mut bytes = state.command.take().ok_or(io::ErrorKind::WouldBlock)?;
        let front = state.transport.front();
        let length = front.len().min(AUDIO_CHUNK);
        bytes[..length].copy_from_slice(&front[..length]);
        bytes.truncate(length);
        drop(state);
        let shared = Arc::clone(&self.state);
        let pipes = Arc::clone(&self.pipes);
        let result = handle.send(
            pipes.input.as_raw_fd(),
            IoBuf::from_vec(bytes),
            move |result: IoResult| {
                let _pipes = pipes;
                let mut returned = ReturnBuffer {
                    state: Arc::clone(&shared),
                    direction: Direction::Command,
                    bytes: None,
                };
                let result = result.and_then(|transfer| {
                    let count = transfer.bytes_transferred as usize;
                    returned.bytes = Some(transfer.buf.into_vec());
                    if count == 0 {
                        return Err(io::ErrorKind::WriteZero.into());
                    }
                    shared
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .transport
                        .consume(count)
                        .map_err(|_| io::Error::other("invalid MBROLA write progress"))?;
                    Ok(count)
                });
                drop(returned);
                done(result);
            },
        );
        if result.is_err() {
            self.restore(Direction::Command);
        }
        result
    }

    /// Close input only after all queued/in-flight writes complete. This is an
    /// explicit end of the child's input, not a normal reusable flush.
    pub fn finish_input(&self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.transport.pending() != 0 || state.command.is_none() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        if !state.input_closed {
            self.pipes.input.shutdown(Shutdown::Write)?;
            state.input_closed = true;
        }
        Ok(())
    }

    /// One audio read. The loan remains busy during `done`; post the next read
    /// after it returns. Header and odd sample fragments survive short reads.
    pub fn read_audio<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        done: impl FnOnce(io::Result<Audio<'_>>) + Send + 'static,
    ) -> io::Result<IoOpId> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.decoder.ended {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let bytes = state.audio.take().ok_or(io::ErrorKind::WouldBlock)?;
        drop(state);
        let shared = Arc::clone(&self.state);
        let pipes = Arc::clone(&self.pipes);
        let result = handle.recv(
            pipes.audio.as_raw_fd(),
            IoBuf::from_vec(bytes),
            move |result: IoResult| {
                let _pipes = pipes;
                let mut returned = ReturnBuffer {
                    state: Arc::clone(&shared),
                    direction: Direction::Audio,
                    bytes: None,
                };
                match result {
                    Err(error) => done(Err(error)),
                    Ok(transfer) => {
                        returned.bytes = Some(transfer.buf.into_vec());
                        let bytes = returned.bytes.as_mut().expect("returned audio");
                        let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
                        let result = state.decoder.decode(bytes);
                        drop(state);
                        done(result);
                    }
                }
            },
        );
        if result.is_err() {
            self.restore(Direction::Audio);
        }
        result
    }

    /// Read one stderr extent; return true for progress, false for actual EOF.
    /// The latest warning can be copied with `last_error` outside the callback.
    pub fn read_errors<P: IoPort>(
        &self,
        handle: &ProactorHandle<P>,
        done: impl FnOnce(io::Result<bool>) + Send + 'static,
    ) -> io::Result<IoOpId> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.error_eof {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let bytes = state.error.take().ok_or(io::ErrorKind::WouldBlock)?;
        drop(state);
        let shared = Arc::clone(&self.state);
        let pipes = Arc::clone(&self.pipes);
        let result = handle.recv(
            pipes.error.as_raw_fd(),
            IoBuf::from_vec(bytes),
            move |result: IoResult| {
                let _pipes = pipes;
                let mut returned = ReturnBuffer {
                    state: Arc::clone(&shared),
                    direction: Direction::Error,
                    bytes: None,
                };
                let result = result.map(|transfer| {
                    let bytes = transfer.buf.into_vec();
                    let progress = !bytes.is_empty();
                    let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
                    state.transport.stderr(&bytes, !progress);
                    state.error_eof = !progress;
                    returned.bytes = Some(bytes);
                    progress
                });
                done(result);
            },
        );
        if result.is_err() {
            self.restore(Direction::Error);
        }
        result
    }

    pub fn last_error(&self, output: &mut [u8]) -> usize {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let error = state.transport.error();
        let length = error.len().min(output.len());
        output[..length].copy_from_slice(&error[..length]);
        length
    }

    /// Nonblocking status query, intended after stream EOF or termination.
    /// The caller must not repeatedly query it without a host/process event.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Request termination; outstanding stdio loans still require completion.
    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    fn restore(&self, direction: Direction) {
        *self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .loan(direction) = Some(buffer(direction));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Child is deliberately not captured by callbacks. Final reaping is a
        // shutdown operation on the owner, never on the completion dispatcher.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav() -> Vec<u8> {
        let mut bytes = vec![0; 44];
        bytes[..4].copy_from_slice(b"RIFF");
        bytes[8..16].copy_from_slice(b"WAVEfmt ");
        bytes[24..28].copy_from_slice(&22050u32.to_le_bytes());
        bytes.extend((0..1025).map(|n| (n * 37) as u8));
        bytes.push(0x95); // whole samples, including one split at every odd read
        bytes
    }

    fn verify_fragments(source: &[u8], fragments: impl Iterator<Item = usize>) {
        let mut decoder = Decoder::default();
        let mut loan = buffer(Direction::Audio);
        let address = loan.as_ptr();
        let mut actual = Vec::new();
        let mut start = 0;
        for length in fragments {
            assert!(length > 0);
            loan.clear();
            loan.extend_from_slice(&source[start..start + length]);
            match decoder.decode(&mut loan).unwrap() {
                Audio::Progress { sample_rate, bytes } => {
                    assert_eq!(sample_rate, (start + length >= 44).then_some(22050));
                    assert_eq!(bytes.len() % 2, 0);
                    actual.extend_from_slice(bytes);
                }
                Audio::Eof => panic!("progress mistaken for EOF"),
            }
            assert_eq!(loan.as_ptr(), address);
            assert!(!decoder.ended);
            start += length;
        }
        assert_eq!(start, source.len());
        assert_eq!(actual, source[44..]);
        loan.clear();
        assert!(matches!(decoder.decode(&mut loan), Ok(Audio::Eof)));
        assert!(decoder.ended);
    }

    #[test]
    fn every_two_way_header_or_sample_split_preserves_pcm() {
        let source = wav();
        for split in 1..source.len() {
            verify_fragments(&source, [split, source.len() - split].into_iter());
        }
    }

    #[test]
    fn repeated_short_reads_and_carry_use_the_original_loan() {
        let source = wav();
        for step in 1..=103 {
            verify_fragments(&source, source.chunks(step).map(<[u8]>::len));
        }
        // Largest read following an odd carry still fits the extra slot.
        let mut source = wav()[..45].to_vec();
        source.extend(std::iter::repeat_n(0x51, AUDIO_CHUNK));
        source.push(0x61);
        verify_fragments(&source, [45, AUDIO_CHUNK, 1].into_iter());
    }

    #[test]
    fn truncated_header_or_sample_ends_with_error() {
        let source = wav();
        for length in (0..44).chain([45, source.len() - 1]) {
            let mut decoder = Decoder::default();
            if length != 0 {
                assert!(decoder.decode(&mut source[..length].to_vec()).is_ok());
            }
            assert_eq!(
                decoder.decode(&mut Vec::new()).err().unwrap().kind(),
                io::ErrorKind::UnexpectedEof
            );
            assert!(decoder.ended);
        }
    }

    #[test]
    fn invalid_signature_and_rates_end_decoding() {
        for rate in [0, i32::MAX as u32 + 1, u32::MAX] {
            let mut source = wav();
            source[24..28].copy_from_slice(&rate.to_le_bytes());
            let mut decoder = Decoder::default();
            assert_eq!(
                decoder.decode(&mut source).err().unwrap().kind(),
                io::ErrorKind::InvalidData
            );
            assert!(decoder.ended);
        }
        let mut source = wav();
        source[0] = b'X';
        let mut decoder = Decoder::default();
        assert_eq!(
            decoder.decode(&mut source).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(decoder.ended);
    }
}
