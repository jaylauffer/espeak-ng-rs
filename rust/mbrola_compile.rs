//! Complete MBROLA translation compiler with bounded retained storage.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2016 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    common_text::string_word, engine_io, spect_compile::errno, voice_storage::compat_path,
};

const MAX_SOURCE: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 255; // The legacy 256-row array also contains a terminator.
#[derive(Default)]
pub struct Workspace {
    source: Vec<u8>,
    output: Vec<u8>,
}

fn space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 11 | 12)
}
fn skip(bytes: &mut &[u8]) {
    let n = bytes.iter().position(|b| !space(*b)).unwrap_or(bytes.len());
    *bytes = &bytes[n..];
}
fn number(bytes: &mut &[u8]) -> Result<Option<i32>, i32> {
    skip(bytes);
    let negative = bytes.first() == Some(&b'-');
    if matches!(bytes.first(), Some(b'-' | b'+')) {
        *bytes = &bytes[1..];
    }
    let mut value = 0i64;
    let mut digits = false;
    while let Some(b'0'..=b'9') = bytes.first() {
        digits = true;
        value = value * 10 + (bytes[0] - b'0') as i64;
        if value > i32::MAX as i64 + i64::from(negative) {
            return Err(22);
        }
        *bytes = &bytes[1..];
    }
    Ok(digits.then_some(if negative { -value } else { value } as i32))
}
fn token<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
    skip(bytes);
    let n = bytes.iter().position(|b| space(*b)).unwrap_or(bytes.len());
    let value = &bytes[..n];
    *bytes = &bytes[n..];
    (!value.is_empty()).then_some(value)
}

impl Workspace {
    /// Match fgets(..., 40): long physical lines are separate 39-byte records.
    /// Windows text streams collapse CRLF and stop at Ctrl-Z before framing.
    fn parse(&mut self, windows_text: bool) -> Result<usize, i32> {
        self.output.clear();
        self.output.try_reserve(4 + 256 * 24).map_err(|_| 12)?;
        self.output.extend_from_slice(&20i32.to_le_bytes());
        let mut count = 0;
        let mut offset = 0;
        while offset < self.source.len() {
            let mut line = [0; 39];
            let mut n = 0;
            while n < line.len() && offset < self.source.len() {
                let mut b = self.source[offset];
                offset += 1;
                if windows_text && b == 26 {
                    offset = self.source.len();
                    break;
                }
                if windows_text && b == b'\r' && self.source.get(offset) == Some(&b'\n') {
                    b = b'\n';
                    offset += 1;
                }
                line[n] = b;
                n += 1;
                if b == b'\n' {
                    break;
                }
            }
            let line = &line[..n];
            let nul = line.iter().position(|b| *b == 0).unwrap_or(n);
            let line = &line[..nul];
            let comment = line
                .windows(2)
                .position(|b| b == b"//")
                .unwrap_or(line.len());
            let mut line = &line[..comment];
            if let Some(mut volume) = line.strip_prefix(b"volume") {
                let volume = number(&mut volume)?.unwrap_or(0);
                self.output[..4].copy_from_slice(&volume.to_le_bytes());
                continue;
            }
            let Some(control) = number(&mut line)? else {
                continue;
            };
            let Some(name) = token(&mut line) else {
                continue;
            };
            let Some(next) = token(&mut line) else {
                continue;
            };
            let Some(percent) = number(&mut line)? else {
                continue;
            };
            let Some(first) = token(&mut line) else {
                continue;
            };
            let second = token(&mut line);
            if count == MAX_ROWS {
                return Err(22);
            }
            let words = [
                string_word(name),
                match next {
                    b"NULL" => 0,
                    b"VWL" => 2,
                    _ => string_word(next),
                },
                if first == b"NULL" {
                    0
                } else {
                    string_word(first)
                },
                second.map(string_word).unwrap_or(0),
                percent as u32,
                control as u32,
            ];
            for word in words {
                self.output.extend_from_slice(&word.to_le_bytes());
            }
            count += 1;
        }
        self.output.extend_from_slice(&[0; 24]);
        Ok(count)
    }

    /// Source is fully read/parsed before the destination is opened. Errors
    /// identify which path failed; completed writes precede success/logging.
    pub fn compile(
        &mut self,
        source: &[u8],
        destination: &[u8],
        windows_text: bool,
    ) -> Result<usize, (i32, bool)> {
        let path = compat_path(source).map_err(|e| (errno(&e), false))?;
        engine_io::read_file_into(&path, MAX_SOURCE, &mut self.source)
            .map_err(|e| (errno(&e), false))?;
        let rows = self.parse(windows_text).map_err(|e| (e, false))?;
        let path = compat_path(destination).map_err(|e| (errno(&e), true))?;
        let mut writer =
            engine_io::Writer::create_compiler_output(&path).map_err(|e| (errno(&e), true))?;
        writer.write(&self.output).map_err(|e| (errno(&e), true))?;
        writer.finish().map_err(|e| (errno(&e), true))?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_text_framing_and_unix_raw_bytes_are_distinct() {
        let mut w = Workspace {
            source: b"volume 31\r\n0 a NULL 50 b\r\n\x1a0 c NULL 60 d\n".to_vec(),
            ..Default::default()
        };
        assert_eq!(w.parse(true), Ok(1));
        assert_eq!(&w.output[..4], &31i32.to_le_bytes());
        assert_eq!(w.parse(false), Ok(1)); // Ctrl-Z makes the second row invalid.
        w.source = [vec![b' '; 38], b"\r\n0 a NULL 50 b\n".to_vec()].concat();
        assert_eq!(w.parse(true), Ok(1));
        w.source = b"0 a NULL 50 b\x1a0 c NULL 60 d\n".to_vec();
        assert_eq!(w.parse(true), Ok(1));
        assert_eq!(&w.output[12..16], &string_word(b"b").to_le_bytes());
        assert_eq!(w.parse(false), Ok(1));
        assert_eq!(&w.output[12..16], &string_word(b"b\x1a0").to_le_bytes());
    }
    #[test]
    fn bounds_reject_legacy_overflow_and_reuse_storage() {
        let mut w = Workspace {
            source: b"0 a NULL 50 b\n".repeat(255),
            ..Default::default()
        };
        assert_eq!(w.parse(false), Ok(255));
        let address = w.output.as_ptr();
        for _ in 0..64 {
            assert_eq!(w.parse(false), Ok(255));
            assert_eq!(w.output.as_ptr(), address);
        }
        w.source.extend_from_slice(b"0 a NULL 50 b\n");
        assert_eq!(w.parse(false), Err(22));
        for input in [
            b"volume2147483648".as_slice(),
            b"-2147483649 a NULL 0 b",
            b"0 a NULL 2147483648 b",
        ] {
            w.source.clear();
            w.source.extend_from_slice(input);
            assert_eq!(w.parse(false), Err(22));
        }
        w.source = b"volume-2147483648\n0 a NULL 20foo bar".to_vec();
        assert_eq!(w.parse(false), Ok(1));
        assert_eq!(&w.output[..4], &i32::MIN.to_le_bytes());
        assert_eq!(&w.output[12..16], &string_word(b"foo").to_le_bytes());
    }
}
