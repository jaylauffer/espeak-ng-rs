//! Owned phondata manifest, with retained bounded proactor input and rows.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    engine_io,
    voice_reader::{Reader, TextMode},
};
use std::{io, path::Path};
pub const EMPTY: i32 = 0x10000dff;
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_LINES: usize = 131072;
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub value: u32,
    pub name: [u8; 120],
}
#[derive(Default)]
pub struct Workspace {
    bytes: Vec<u8>,
    rows: Vec<Row>,
    keep_prefix_x: bool,
}
impl Workspace {
    /// The host scanf primitive may leave 'x' after a zero without hex digits.
    /// Snapshot this libc policy once; valid hex numbers use the same parser.
    pub fn prefix_policy(&mut self, keep_x: bool) {
        self.keep_prefix_x = keep_x;
    }
    pub fn parse_bytes(
        &mut self,
        bytes: &[u8],
        width: usize,
        mode: TextMode,
        alpha: &[u8; 256],
        spaces: &[u8; 256],
    ) -> io::Result<i32> {
        if bytes.len() > MAX_BYTES {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.bytes.clear();
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        self.bytes.extend_from_slice(bytes);
        self.parse(width, mode, alpha, spaces)
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    /// Bounds reject before publication. No per-name allocation or foreign view.
    pub fn read(
        &mut self,
        path: &Path,
        width: usize,
        mode: TextMode,
        alpha: &[u8; 256],
        spaces: &[u8; 256],
    ) -> io::Result<i32> {
        self.rows.clear();
        engine_io::read_file_into(path, MAX_BYTES, &mut self.bytes)?;
        self.parse(width, mode, alpha, spaces)
    }
    pub fn parse(
        &mut self,
        width: usize,
        mode: TextMode,
        alpha: &[u8; 256],
        spaces: &[u8; 256],
    ) -> io::Result<i32> {
        self.rows.clear();
        let mut reader = Reader::new(io::Cursor::new(&self.bytes), width, mode)?;
        let mut count = 0;
        while reader.next_chunk()?.is_some() {
            count += 1;
            if count > MAX_LINES {
                return Err(io::ErrorKind::InvalidData.into());
            }
        }
        if count == 0 {
            return Ok(EMPTY);
        }
        self.rows
            .try_reserve_exact(count)
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        let mut reader = Reader::new(io::Cursor::new(&self.bytes), width, mode)?;
        while let Some(chunk) = reader.next_chunk()? {
            if alpha[chunk[0] as usize] == 0 || chunk.len() < 3 {
                continue;
            }
            let chunk = &chunk[..chunk.iter().position(|b| *b == 0).unwrap_or(chunk.len())];
            if chunk.len() < 3 {
                continue;
            }
            match row(&chunk[2..], spaces, self.keep_prefix_x) {
                Ok(Some(row)) => self.rows.push(row),
                Ok(None) => {}
                Err(error) => {
                    self.rows.clear();
                    return Err(error);
                }
            }
        }
        Ok(0)
    }
}
fn row(bytes: &[u8], spaces: &[u8; 256], keep_prefix_x: bool) -> io::Result<Option<Row>> {
    let mut at = 0;
    while at < bytes.len() && spaces[bytes[at] as usize] != 0 {
        at += 1;
    }
    let negative = bytes.get(at) == Some(&b'-');
    if negative || bytes.get(at) == Some(&b'+') {
        at += 1;
    }
    if bytes.get(at) == Some(&b'0') && matches!(bytes.get(at + 1), Some(b'x' | b'X')) {
        at += 2;
        // Preserve the host's bounded scanf probe for incomplete prefixes.
        if !bytes.get(at).is_some_and(u8::is_ascii_hexdigit) {
            if keep_prefix_x {
                at -= 1;
            }
            return name(bytes, at, 0, spaces);
        }
    }
    let start = at;
    let mut value: u32 = 0;
    while let Some(&b) = bytes.get(at) {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => break,
        };
        value = value
            .checked_mul(16)
            .and_then(|v| v.checked_add(u32::from(digit)))
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        at += 1;
    }
    if start == at {
        return Ok(None);
    }
    if negative {
        value = value.wrapping_neg();
    }
    name(bytes, at, value, spaces)
}
fn name(bytes: &[u8], mut at: usize, value: u32, spaces: &[u8; 256]) -> io::Result<Option<Row>> {
    while at < bytes.len() && spaces[bytes[at] as usize] != 0 {
        at += 1;
    }
    let start = at;
    while at < bytes.len() && spaces[bytes[at] as usize] == 0 {
        at += 1;
    }
    if at == start {
        return Ok(None);
    }
    if at - start >= 120 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut row = Row {
        value,
        name: [0; 120],
    };
    row.name[..at - start].copy_from_slice(&bytes[start..at]);
    Ok(Some(row))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn maps() -> ([u8; 256], [u8; 256]) {
        (
            std::array::from_fn(|i| u8::from((i as u8).is_ascii_alphabetic())),
            std::array::from_fn(|i| u8::from((i as u8).is_ascii_whitespace())),
        )
    }
    #[test]
    fn manifest_preserves_text_chunks_nul_signed_hex_and_no_entries() {
        let (a, s) = maps();
        let mut w = Workspace {
            bytes: b"# header\nW -1 abc\nS 0xF name\0junk\nE 1g\n".to_vec(),
            rows: Vec::new(),
            keep_prefix_x: false,
        };
        assert_eq!(w.parse(1024, TextMode::Binary, &a, &s).unwrap(), 0);
        assert_eq!(
            w.rows.iter().map(|r| r.value).collect::<Vec<_>>(),
            [u32::MAX, 15, 1]
        );
        assert_eq!(&w.rows[2].name[..2], b"g\0");
        w.bytes = b"# comments only\n".to_vec();
        assert_eq!(w.parse(1024, TextMode::Binary, &a, &s).unwrap(), 0);
        assert!(w.rows.is_empty());
        w.bytes = b"\x1aW 1 a".to_vec();
        assert_eq!(w.parse(260, TextMode::Windows, &a, &s).unwrap(), EMPTY);
    }
    #[test]
    fn manifest_reuses_rows_and_rejects_old_unbounded_names_and_numbers() {
        let (a, s) = maps();
        let mut w = Workspace {
            bytes: b"W 1 a\r\nW 2 b\r\n\x1aW 3 c\n".to_vec(),
            ..Workspace::default()
        };
        assert_eq!(w.parse(260, TextMode::Windows, &a, &s).unwrap(), 0);
        assert_eq!(w.rows.len(), 2);
        let p = w.rows.as_ptr();
        let cap = w.rows.capacity();
        assert_eq!(w.parse(260, TextMode::Windows, &a, &s).unwrap(), 0);
        assert_eq!((w.rows.as_ptr(), w.rows.capacity()), (p, cap));
        w.bytes = [b"W 1 valid\nW 1 ".as_slice(), &[b'x'; 120], b"\n"].concat();
        assert_eq!(
            w.parse(1024, TextMode::Binary, &a, &s).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(w.rows.is_empty());
        w.bytes = b"W 100000000 a\n".to_vec();
        assert!(w.parse(1024, TextMode::Binary, &a, &s).is_err());
    }
    #[test]
    fn manifest_host_prefix_probe_preserves_both_consumption_policies() {
        let (a, s) = maps();
        let mut w = Workspace::default();
        w.prefix_policy(true);
        w.parse_bytes(b"W 0x name\nW 0Xg word\n", 1024, TextMode::Binary, &a, &s)
            .unwrap();
        assert_eq!(&w.rows[0].name[..2], b"x\0");
        assert_eq!(&w.rows[1].name[..3], b"Xg\0");
        w.prefix_policy(false);
        w.parse_bytes(b"W 0x name\nW 0Xg word\n", 1024, TextMode::Binary, &a, &s)
            .unwrap();
        assert_eq!(&w.rows[0].name[..5], b"name\0");
        assert_eq!(&w.rows[1].name[..2], b"g\0");
    }
    #[test]
    fn manifest_reopens_distinct_files_through_engine_io_with_retained_storage() {
        let (a, s) = maps();
        let mut w = Workspace::default();
        let root =
            std::env::temp_dir().join(format!("espeak-manifest-reopen-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let first = root.join("first");
        let second = root.join("second");
        std::fs::write(&first, b"W 1 first\n").unwrap();
        std::fs::write(&second, b"E ff other\n").unwrap();
        w.read(&first, 1024, TextMode::Binary, &a, &s).unwrap();
        w.read(&second, 1024, TextMode::Binary, &a, &s).unwrap();
        let bytes = w.bytes.as_ptr();
        let rows = w.rows.as_ptr();
        for _ in 0..50 {
            assert_eq!(w.read(&first, 1024, TextMode::Binary, &a, &s).unwrap(), 0);
            assert_eq!(w.rows[0].value, 1);
            assert_eq!(&w.rows[0].name[..6], b"first\0");
            assert_eq!(w.read(&second, 1024, TextMode::Binary, &a, &s).unwrap(), 0);
            assert_eq!(w.rows[0].value, 255);
            assert_eq!(&w.rows[0].name[..6], b"other\0");
            assert_eq!((w.bytes.as_ptr(), w.rows.as_ptr()), (bytes, rows));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
