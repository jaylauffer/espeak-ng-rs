//! Bounded active voice streams, shared by native byte and compatibility files.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::voice::Directives;
use std::io::{self, Read};

/// Windows text files translate CRLF and stop at CTRL-Z. Native callers loading
/// binary bytes through the host choose the source platform explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextMode {
    Binary,
    Windows,
}
impl TextMode {
    pub const fn platform() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Binary
        }
    }
}
/// Reuses one 8 KiB input buffer and one fixed directive buffer. Opening and
/// reading belong to initialization or the caller's worker, outside proactor
/// completion delivery. `Cursor<&[u8]>` accepts already-resident host I/O bytes.
pub struct Reader<R> {
    source: R,
    scratch: [u8; 8192],
    position: usize,
    available: usize,
    line: [u8; 4096],
    width: usize,
    mode: TextMode,
    pending: Option<u8>,
    ended: bool,
}
impl<R: Read> Reader<R> {
    pub fn new(source: R, width: usize, mode: TextMode) -> io::Result<Self> {
        if !(2..=4096).contains(&width) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "voice line width must be 2..=4096",
            ));
        }
        Ok(Self {
            source,
            scratch: [0; 8192],
            position: 0,
            available: 0,
            line: [0; 4096],
            width,
            mode,
            pending: None,
            ended: false,
        })
    }
    fn raw(&mut self) -> io::Result<Option<u8>> {
        if let Some(byte) = self.pending.take() {
            return Ok(Some(byte));
        }
        while self.position == self.available {
            match self.source.read(&mut self.scratch) {
                Ok(0) => return Ok(None),
                Ok(length) => {
                    self.position = 0;
                    self.available = length;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        let byte = self.scratch[self.position];
        self.position += 1;
        Ok(Some(byte))
    }
    fn byte(&mut self) -> io::Result<Option<u8>> {
        if self.ended {
            return Ok(None);
        }
        let Some(byte) = self.raw()? else {
            self.ended = true;
            return Ok(None);
        };
        if self.mode == TextMode::Windows {
            if byte == 0x1a {
                self.ended = true;
                return Ok(None);
            }
            if byte == b'\r' {
                return match self.raw()? {
                    Some(b'\n') => Ok(Some(b'\n')),
                    other => {
                        self.pending = other;
                        Ok(Some(b'\r'))
                    }
                };
            }
        }
        Ok(Some(byte))
    }
    fn fill(&mut self) -> io::Result<usize> {
        let mut used = 0;
        while used < self.width - 1 {
            let Some(byte) = self.byte()? else {
                break;
            };
            self.line[used] = byte;
            used += 1;
            if byte == b'\n' {
                break;
            }
        }
        self.line[used] = 0;
        Ok(used)
    }
    /// Raw fgets-width text chunks, retaining whitespace/newlines. Borrowed
    /// until the next read; useful for configuration's legacy prefix grammar.
    pub fn next_chunk(&mut self) -> io::Result<Option<&[u8]>> {
        let used = self.fill()?;
        Ok((used != 0).then_some(&self.line[..used]))
    }
    /// Returned strings are borrowed until the next read or destruction. No
    /// allocation occurs per directive; unreadable streams report the I/O error.
    pub fn next_directive(&mut self) -> io::Result<Option<(&[u8], &[u8])>> {
        loop {
            let used = self.fill()?;
            if used == 0 {
                return Ok(None);
            }
            let spans = {
                let mut directives =
                    Directives::new(&self.line[..used], self.width).map_err(io::Error::other)?;
                directives.next().map(|(key, value)| {
                    (
                        key.len(),
                        if value.is_empty() {
                            key.len()
                        } else {
                            value.as_ptr() as usize - self.line.as_ptr() as usize
                        },
                        value.len(),
                    )
                })
            };
            let Some((key_length, value_start, value_length)) = spans else {
                continue;
            };
            // Empty values can refer to the static empty slice from Directives;
            // only nonempty values need their source-buffer offset.
            self.line[key_length] = 0;
            self.line[value_start + value_length] = 0;
            return Ok(Some((
                &self.line[..key_length],
                &self.line[value_start..value_start + value_length],
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Chunks<'a> {
        bytes: &'a [u8],
        width: usize,
        interrupted: bool,
    }
    impl Read for Chunks<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = self.bytes.len().min(self.width).min(output.len());
            output[..count].copy_from_slice(&self.bytes[..count]);
            self.bytes = &self.bytes[count..];
            Ok(count)
        }
    }
    #[test]
    fn arbitrary_read_boundaries_preserve_chunks_comments_and_embedded_nul() {
        let mut bytes=b"# ignored\npitch 82 118 // comment\n name ignored\ntone\nname\0 ignored\nbreath\x0b1 2\n".to_vec();
        bytes.extend_from_slice(&[b'a'; 8200]);
        bytes.extend_from_slice(b"\nname final");
        for width in [2, 5, 120, 260, 1024, 4096] {
            let expected: Vec<_> = Directives::new(&bytes, width).unwrap().collect();
            for chunk in [1, 2, 3, 127, 8192] {
                let mut stream = Reader::new(
                    Chunks {
                        bytes: &bytes,
                        width: chunk,
                        interrupted: false,
                    },
                    width,
                    TextMode::Binary,
                )
                .unwrap();
                for (key, value) in &expected {
                    assert_eq!(stream.next_directive().unwrap(), Some((*key, *value)));
                }
                assert_eq!(stream.next_directive().unwrap(), None);
            }
        }
    }
    #[test]
    fn windows_text_conversion_spans_reads_and_keeps_lone_cr() {
        for chunk in 1..=9 {
            let mut stream = Reader::new(
                Chunks {
                    bytes: b"name one\r\nlanguage en\r\npitch 82\r 118\r\ntone\r\x1aname hidden",
                    width: chunk,
                    interrupted: false,
                },
                260,
                TextMode::Windows,
            )
            .unwrap();
            assert_eq!(
                stream.next_directive().unwrap(),
                Some((b"name".as_slice(), b"one".as_slice()))
            );
            assert_eq!(
                stream.next_directive().unwrap(),
                Some((b"language".as_slice(), b"en".as_slice()))
            );
            assert_eq!(
                stream.next_directive().unwrap(),
                Some((b"pitch".as_slice(), b"82\r 118".as_slice()))
            );
            assert_eq!(
                stream.next_directive().unwrap(),
                Some((b"tone".as_slice(), b"".as_slice()))
            );
            assert_eq!(stream.next_directive().unwrap(), None);
        }
    }
}
