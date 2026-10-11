//! Owned phoneme/intonation compiler sources and bounded token scanning.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{engine_io, voice_reader::TextMode};
use std::{io, path::Path};

pub const DEPTH: usize = 13;
pub const MAX_FILE: usize = 16 * 1024 * 1024;
pub const MAX_RETAINED: usize = 64 * 1024 * 1024;
pub const TOKEN_BYTES: usize = 256;

/// The caller's locale classifiers, copied before reading any sources.
pub struct Classes {
    pub spaces: [u8; 256],
    pub digits: [u8; 256],
}
impl Classes {
    fn space(&self, c: u8) -> bool {
        self.spaces[c as usize] != 0
    }
    fn digit(&self, c: u8) -> bool {
        self.digits[c as usize] != 0
    }
}

/// A fixed result; `length == -1` leaves the previous native token untouched.
/// Status: 0 token, 1 expected '(' matched, -1 unexpected '(', -2 EOF,
/// -3 oversized token. Diagnostic: 1 unsigned, 2 number, 3 numeric overflow.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct Token {
    pub status: i32,
    pub line: i32,
    pub terminator: i32,
    pub length: i32,
    pub value: i32,
    pub diagnostic: i32,
    pub bytes: [u8; TOKEN_BYTES],
}

#[derive(Default)]
pub struct Source {
    bytes: Vec<u8>,
    position: usize,
    ended: bool,
    loaded: bool,
    line: i32,
    bookmark: (usize, i32),
    pushed: Option<u8>,
}
impl Source {
    fn reset(&mut self) {
        self.position = 0;
        self.ended = false;
        self.line = 1;
        self.bookmark = (0, 1);
        self.pushed = None;
    }
    fn text(&mut self, mode: TextMode) {
        if mode == TextMode::Windows {
            let mut target = 0;
            let mut source = 0;
            while source < self.bytes.len() && self.bytes[source] != 0x1a {
                if self.bytes[source] == b'\r' && self.bytes.get(source + 1) == Some(&b'\n') {
                    source += 1;
                }
                self.bytes[target] = self.bytes[source];
                target += 1;
                source += 1;
            }
            self.bytes.truncate(target);
        }
    }
    fn get(&mut self) -> i32 {
        if let Some(byte) = self
            .pushed
            .take()
            .or_else(|| self.bytes.get(self.position).copied())
        {
            self.position += 1;
            if byte == b'\n' {
                self.line = self.line.saturating_add(1);
            }
            i32::from(byte)
        } else {
            self.ended = true;
            -1
        }
    }
    fn unget(&mut self, c: i32) {
        // The scanner only pushes back the byte it just read. EOF is not a byte.
        if c != -1 && self.position != 0 {
            self.position -= 1;
            self.ended = false;
            self.pushed = Some(c as u8);
            if c == i32::from(b'\n') {
                self.line = self.line.saturating_sub(1);
            }
        }
    }
    pub fn ended(&self) -> bool {
        !self.loaded || self.ended
    }
    pub fn rewind(&mut self) {
        self.reset();
    }
    pub fn unget_item(&mut self) -> i32 {
        (self.position, self.line) = self.bookmark;
        self.pushed = None;
        self.ended = false;
        self.line
    }
    pub fn check_next(&mut self, line: i32) -> (i32, i32) {
        self.line = line;
        let c = loop {
            let c = self.get();
            if c != i32::from(b' ') && c != i32::from(b'\t') {
                break c;
            }
        };
        self.unget(c);
        (c, self.line)
    }
    /// fgets semantics: width includes the NUL; returned bytes exclude it.
    pub fn chunk(&mut self, output: &mut [u8]) -> Option<usize> {
        if output.len() < 2 || !self.loaded {
            return None;
        }
        let mut n = 0;
        while n < output.len() - 1 {
            let c = self.get();
            if c == -1 {
                break;
            }
            output[n] = c as u8;
            n += 1;
            if c == i32::from(b'\n') {
                break;
            }
        }
        if n == 0 {
            return None;
        }
        output[n] = 0;
        Some(n)
    }
    /// Kind uses the existing compiler discriminants: string=2, number=3,
    /// signed number=4, opening bracket=6. Other kinds return a raw token.
    pub fn next(&mut self, kind: i32, line: i32, classes: &Classes) -> Token {
        self.line = line;
        self.bookmark = (self.position, line);
        let mut token = Token {
            status: -2,
            line,
            terminator: -1,
            length: -1,
            value: 0,
            diagnostic: 0,
            bytes: [0; TOKEN_BYTES],
        };
        if !self.loaded {
            return token;
        }
        // C's local byte narrows EOF to 255, while feof retains the distinction.
        let mut c = 0;
        while !self.ended {
            c = self.get() as u8;
            if c == b'/' {
                let c2 = self.get() as u8;
                if c2 == b'/' {
                    while !self.ended {
                        c = self.get() as u8;
                        if c == b'\n' {
                            break;
                        }
                    }
                } else {
                    // Legacy narrows EOF to 255 here, then ungetc clears EOF.
                    self.unget(i32::from(c2));
                }
            }
            if !classes.space(c) {
                break;
            }
        }
        if self.ended {
            token.line = self.line;
            return token;
        }
        if c == b'(' {
            token.status = if kind == 6 { 1 } else { -1 };
            token.line = self.line;
            return token;
        }
        let mut n = 0;
        let mut overflow = false;
        while !self.ended && !classes.space(c) && !b"(),".contains(&c) {
            if c == b'\\' {
                c = self.get() as u8;
            }
            if n < TOKEN_BYTES - 1 {
                token.bytes[n] = c;
                n += 1;
            } else {
                overflow = true;
            }
            let previous = c;
            c = self.get() as u8;
            if self.ended || previous == b'=' {
                break;
            }
        }
        while classes.space(c) {
            c = self.get() as u8;
            if self.ended {
                break;
            }
        }
        token.terminator = if b"(),".contains(&c) {
            i32::from(c)
        } else {
            i32::from(b' ')
        };
        if !self.ended && !b"),".contains(&c) {
            self.unget(i32::from(c));
        }
        token.length = n as i32;
        token.line = self.line;
        token.status = if overflow { -3 } else { 0 };
        if !overflow && (kind == 3 || kind == 4) {
            let mut start = 0;
            let negative = kind == 4 && token.bytes[0] == b'-';
            if negative {
                start += 1;
            }
            if !classes.digit(token.bytes[start]) {
                token.diagnostic = if kind == 3 && token.bytes[start] == b'-' {
                    1
                } else {
                    2
                };
            }
            let mut acc = 0i32;
            for &byte in &token.bytes[start..n] {
                if !classes.digit(byte) {
                    break;
                }
                let Some(value) = acc
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(i32::from(byte) - 48))
                else {
                    token.diagnostic = 3;
                    acc = 0;
                    break;
                };
                acc = value;
            }
            token.value = if negative {
                acc.checked_neg().unwrap_or_else(|| {
                    token.diagnostic = 3;
                    0
                })
            } else {
                acc
            };
        }
        token
    }
}

/// Stable include slots. Repeated opens overwrite retained storage in place.
pub struct Sources {
    slots: [Source; DEPTH],
    pub classes: Classes,
}
impl Sources {
    pub fn new(classes: Classes) -> Self {
        Self {
            slots: std::array::from_fn(|_| Source::default()),
            classes,
        }
    }
    pub fn open(&mut self, depth: usize, path: &Path, mode: TextMode) -> io::Result<()> {
        if depth >= DEPTH {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let other: usize = self
            .slots
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != depth)
            .map(|(_, s)| s.bytes.capacity())
            .sum();
        let limit = MAX_FILE.min(MAX_RETAINED.saturating_sub(other));
        let slot = &mut self.slots[depth];
        slot.loaded = false;
        engine_io::read_file_into(path, limit, &mut slot.bytes)?;
        slot.text(mode);
        slot.reset();
        slot.loaded = true;
        Ok(())
    }
    pub fn source(&mut self, depth: usize) -> Option<&mut Source> {
        self.slots.get_mut(depth).filter(|s| s.loaded)
    }
    pub fn next(&mut self, depth: usize, kind: i32, line: i32) -> Option<Token> {
        let source = self.slots.get_mut(depth).filter(|s| s.loaded)?;
        Some(source.next(kind, line, &self.classes))
    }
    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.bytes.clear();
            slot.loaded = false;
            slot.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn classes() -> Classes {
        Classes {
            spaces: std::array::from_fn(|i| u8::from((i as u8).is_ascii_whitespace() || i == 11)),
            digits: std::array::from_fn(|i| u8::from((i as u8).is_ascii_digit())),
        }
    }
    fn source(bytes: &[u8], mode: TextMode) -> Source {
        let mut source = Source {
            bytes: bytes.to_vec(),
            loaded: true,
            ..Source::default()
        };
        source.text(mode);
        source.reset();
        source
    }
    #[test]
    fn replay_comments_escapes_equals_brackets_and_eof_pushback() {
        let c = classes();
        let mut s = source(b" // comment\n len= 12, a\\(b) /", TextMode::Binary);
        let t = s.next(2, 1, &c);
        assert_eq!(
            (&t.bytes[..t.length as usize], t.line, t.terminator),
            (b"len=".as_slice(), 2, 32)
        );
        assert_eq!(s.unget_item(), 1);
        assert_eq!(s.next(2, 1, &c), t);
        let t = s.next(3, 2, &c);
        assert_eq!((t.value, t.terminator, t.diagnostic), (12, 44, 0));
        let t = s.next(2, 2, &c);
        assert_eq!(
            (&t.bytes[..t.length as usize], t.terminator),
            (b"a(b".as_slice(), 41)
        );
        let t = s.next(2, 2, &c);
        assert_eq!(&t.bytes[..t.length as usize], b"/\xff");
        assert!(s.ended());
        assert_eq!(s.next(2, 2, &c).status, -2);
        s.unget_item();
        // The last EOF request bookmarked EOF, so replay remains EOF.
        assert_eq!(s.next(2, 2, &c).status, -2);
    }
    #[test]
    fn bounded_tokens_integer_admission_and_progress_after_rejection() {
        let c = classes();
        for n in [254, 255, 256, 10000] {
            let mut bytes = vec![b'a'; n];
            bytes.extend_from_slice(b" next");
            let mut s = source(&bytes, TextMode::Binary);
            let t = s.next(2, 1, &c);
            assert_eq!(t.status, if n < 256 { 0 } else { -3 });
            assert_eq!(t.length, n.min(255) as i32);
            assert_eq!(&s.next(2, 1, &c).bytes[..4], b"next");
        }
        let mut s = source(
            b"2147483647 -2147483647 2147483648 -12 -x 12xyz",
            TextMode::Binary,
        );
        assert_eq!(s.next(3, 1, &c).value, i32::MAX);
        assert_eq!(s.next(4, 1, &c).value, -i32::MAX);
        assert_eq!(s.next(3, 1, &c).diagnostic, 3);
        let t = s.next(3, 1, &c);
        assert_eq!((t.value, t.diagnostic), (0, 1));
        assert_eq!(s.next(4, 1, &c).diagnostic, 2);
        assert_eq!(s.next(3, 1, &c).value, 12);
    }
    #[test]
    fn windows_text_chunks_keep_lone_cr_and_rewind_replays_translated_bytes() {
        let mut s = source(b"tune a\r\nhead\r 2\r\n\x1atune hidden", TextMode::Windows);
        let mut out = [0x55; 12];
        assert_eq!(s.chunk(&mut out), Some(7));
        assert_eq!(&out[..8], b"tune a\n\0");
        assert_eq!(s.chunk(&mut out), Some(8));
        assert_eq!(&out[..9], b"head\r 2\n\0");
        assert_eq!(s.chunk(&mut out), None);
        s.rewind();
        let t = s.next(2, 1, &classes());
        assert_eq!(&t.bytes[..4], b"tune");
        assert!(!s.ended());
    }
    #[test]
    fn real_multichunk_reads_reuse_include_storage_and_failed_opens_are_unavailable() {
        let directory =
            std::env::temp_dir().join(format!("espeak-compiler-source-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("source");
        let bytes = vec![b'a'; engine_io::CHUNK_BYTES * 2 + 123];
        std::fs::write(&path, &bytes).unwrap();
        let mut sources = Sources::new(classes());
        sources.open(0, &path, TextMode::Binary).unwrap();
        sources.open(12, &path, TextMode::Binary).unwrap();
        let allocation = sources.slots[12].bytes.as_ptr();
        let retained = sources.slots[12].bytes.capacity();
        for _ in 0..16 {
            sources.open(12, &path, TextMode::Binary).unwrap();
            assert_eq!(sources.slots[12].bytes.as_ptr(), allocation);
            assert_eq!(sources.slots[12].bytes, bytes);
        }
        assert!(sources.open(13, &path, TextMode::Binary).is_err());
        assert!(sources
            .open(12, &directory.join("missing"), TextMode::Binary)
            .is_err());
        assert!(sources.source(12).is_none());
        assert_eq!(sources.source(0).unwrap().bytes, bytes);
        let large = std::fs::File::create(directory.join("large")).unwrap();
        large.set_len(MAX_FILE as u64 + 1).unwrap();
        assert_eq!(
            sources
                .open(12, &directory.join("large"), TextMode::Binary)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(sources.slots[12].bytes.capacity(), retained);
        sources.clear();
        assert!(sources.source(0).is_none());
        assert_eq!(sources.slots[12].bytes.as_ptr(), allocation);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
