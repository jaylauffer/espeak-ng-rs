//! Bounded MBROLA command and stderr state, independent of the I/O driver.
//! Storage is allocated once per owner and reused across writes and resets.
// SPDX-License-Identifier: GPL-3.0-or-later

pub const COMMAND_CAPACITY: usize = 256 * 1024;
const LINE_CAPACITY: usize = 256;
const ERROR_CAPACITY: usize = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Progress,
    Header,
}

pub struct Transport {
    commands: Box<[u8]>,
    head: usize,
    pending: usize,
    line: [u8; LINE_CAPACITY],
    line_length: usize,
    error: [u8; ERROR_CAPACITY],
    error_length: usize,
}

impl Transport {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if capacity == 0 || capacity > COMMAND_CAPACITY {
            return Err(Error::Capacity);
        }
        Ok(Self {
            commands: vec![0; capacity].into_boxed_slice(),
            head: 0,
            pending: 0,
            line: [0; LINE_CAPACITY],
            line_length: 0,
            error: [0; ERROR_CAPACITY],
            error_length: 0,
        })
    }

    /// Admit the entire command before any I/O. A full queue changes nothing;
    /// the caller may consume audio/write completions and retry the command.
    pub fn queue(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > self.commands.len() - self.pending {
            return Err(Error::Capacity);
        }
        let tail = (self.head + self.pending) % self.commands.len();
        let first = bytes.len().min(self.commands.len() - tail);
        self.commands[tail..tail + first].copy_from_slice(&bytes[..first]);
        self.commands[..bytes.len() - first].copy_from_slice(&bytes[first..]);
        self.pending += bytes.len();
        Ok(())
    }

    /// The next contiguous FIFO extent; a short write consumes only its prefix.
    pub fn front(&self) -> &[u8] {
        &self.commands[self.head..self.head + self.pending.min(self.commands.len() - self.head)]
    }

    pub fn consume(&mut self, count: usize) -> Result<(), Error> {
        if count > self.front().len() {
            return Err(Error::Progress);
        }
        self.pending -= count;
        self.head = (self.head + count) % self.commands.len();
        if self.pending == 0 {
            self.head = 0;
        }
        Ok(())
    }

    pub fn pending(&self) -> usize {
        self.pending
    }

    /// Cancel queued commands without allocating or overwriting unused storage.
    pub fn clear_commands(&mut self) {
        self.head = 0;
        self.pending = 0;
    }

    /// Start another process without carrying a partial stderr line into it.
    pub fn clear_stderr(&mut self) {
        self.line_length = 0;
    }

    /// Retain the latest non-control line. Fragmentation and overlong lines do
    /// not lose framing: excess bytes are discarded through the next newline.
    /// Upstream's reset diagnostics are informational, not flush acknowledgments.
    pub fn stderr(&mut self, bytes: &[u8], eof: bool) -> usize {
        let mut messages = 0;
        for &byte in bytes {
            if byte == b'\n' {
                messages += usize::from(self.finish_line());
            } else if self.line_length < self.line.len() {
                self.line[self.line_length] = byte;
                self.line_length += 1;
            }
        }
        if eof && self.line_length != 0 {
            messages += usize::from(self.finish_line());
        }
        messages
    }

    fn finish_line(&mut self) -> bool {
        let line = &self.line[..self.line_length];
        let keep =
            !line.starts_with(b"Got a reset signal") && !line.starts_with(b"Input Flush Signal");
        if keep {
            self.error_length = line.len().min(self.error.len() - 1);
            self.error[..self.error_length].copy_from_slice(&line[..self.error_length]);
        }
        self.line_length = 0;
        keep
    }

    pub fn error(&self) -> &[u8] {
        &self.error[..self.error_length]
    }
}

/// Parse the fixed 44-byte WAV header emitted by the MBROLA standalone binary.
/// Preserve the legacy signature check; reject rates unrepresentable by its API.
pub fn sample_rate(header: &[u8]) -> Result<i32, Error> {
    if header.len() != 44 || &header[..4] != b"RIFF" || &header[8..16] != b"WAVEfmt " {
        return Err(Error::Header);
    }
    let rate = u32::from_le_bytes(header[24..28].try_into().expect("fixed header extent"));
    if rate == 0 || rate > i32::MAX as u32 {
        return Err(Error::Header);
    }
    Ok(rate as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_writes_wrap_fifo_and_full_admission_is_atomic() {
        let mut transport = Transport::new(8).unwrap();
        transport.queue(b"abcdef").unwrap();
        transport.consume(4).unwrap();
        transport.queue(b"ghijkl").unwrap();
        assert_eq!(transport.queue(b"x"), Err(Error::Capacity));
        assert_eq!(transport.consume(5), Err(Error::Progress));
        assert_eq!(transport.front(), b"efgh");
        transport.consume(3).unwrap();
        assert_eq!(transport.front(), b"h");
        transport.consume(1).unwrap();
        assert_eq!(transport.front(), b"ijkl");
        transport.consume(4).unwrap();
        assert_eq!(transport.pending(), 0);
        transport.queue(b"12345678").unwrap();
        transport.clear_commands();
        transport.queue(b"abcdefgh").unwrap();
        assert_eq!(transport.front(), b"abcdefgh");
    }

    #[test]
    fn stderr_fragments_controls_overlong_lines_and_eof_keep_framing() {
        let mut transport = Transport::new(8).unwrap();
        let stream = b"warning one\nGot a reset signal !\nInput Flush Signal\nwarning two";
        for byte in stream {
            transport.stderr(&[*byte], false);
        }
        assert_eq!(transport.error(), b"warning one");
        assert_eq!(transport.stderr(&[], true), 1);
        assert_eq!(transport.error(), b"warning two");
        assert_eq!(transport.stderr(&[b'x'; 1024], false), 0);
        assert_eq!(transport.stderr(b"\nlast\n", false), 2);
        assert_eq!(transport.error(), b"last");
        transport.stderr(b"unfinished", false);
        transport.clear_stderr();
        transport.stderr(b"new process\n", false);
        assert_eq!(transport.error(), b"new process");
    }

    #[test]
    fn randomized_queue_matches_independent_fifo_and_reuses_storage() {
        use std::collections::VecDeque;
        let mut transport = Transport::new(17).unwrap();
        let address = transport.commands.as_ptr();
        let mut expected: VecDeque<u8> = VecDeque::new();
        let mut seed = 1u32;
        for _ in 0..200_000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            if seed % 127 == 0 {
                transport.clear_commands();
                expected.clear();
            } else if seed & 1 == 0 {
                let length = (seed as usize >> 8) % 21;
                let bytes = [seed as u8; 20];
                let admitted = length <= 17 - expected.len();
                assert_eq!(transport.queue(&bytes[..length]).is_ok(), admitted);
                if admitted {
                    expected.extend(&bytes[..length]);
                }
            } else {
                let front = transport.front();
                assert!(front.iter().eq(expected.iter().take(front.len())));
                let count = (seed as usize >> 8) % (front.len() + 1);
                transport.consume(count).unwrap();
                expected.drain(..count);
            }
            assert_eq!(transport.pending(), expected.len());
            assert_eq!(transport.commands.as_ptr(), address);
        }
    }

    #[test]
    fn independent_owners_and_invalid_headers() {
        assert!(Transport::new(0).is_err());
        assert!(Transport::new(COMMAND_CAPACITY + 1).is_err());
        let mut first = Transport::new(8).unwrap();
        let second = Transport::new(8).unwrap();
        first.queue(b"first").unwrap();
        first.stderr(b"error\n", false);
        assert!(second.front().is_empty());
        assert!(second.error().is_empty());
        let mut header = [0; 44];
        header[..4].copy_from_slice(b"RIFF");
        header[8..16].copy_from_slice(b"WAVEfmt ");
        for rate in [1, 16000, 22050, 48000, i32::MAX as u32] {
            header[24..28].copy_from_slice(&rate.to_le_bytes());
            assert_eq!(sample_rate(&header), Ok(rate as i32));
        }
        for rate in [0u32, u32::MAX] {
            header[24..28].copy_from_slice(&rate.to_le_bytes());
            assert_eq!(sample_rate(&header), Err(Error::Header));
        }
        assert_eq!(sample_rate(&header[..43]), Err(Error::Header));
    }
}
