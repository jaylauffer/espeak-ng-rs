//! Complete owned intonation compiler: two-pass parsing and bounded output.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_source::{Classes, Sources},
    engine_io::Writer,
    voice_reader::TextMode,
};
use std::{
    io::{self, Write},
    path::Path,
};
pub const FAILED: i32 = 0x100001ff;
pub const MAX_TUNES: usize = 100;
pub const RECORD_BYTES: usize = 68;
const PRESETS: [&[u8]; 4] = [b"s1", b"c1", b"q1", b"e1"];
const KEYWORDS: [&[u8]; 11] = [
    b"tune",
    b"endtune",
    b"prehead",
    b"onset",
    b"head",
    b"headenv",
    b"headextend",
    b"headlast",
    b"nucleus0",
    b"nucleus",
    b"split",
];
const ENVELOPES: [&[u8]; 10] = [
    b"fall",
    b"rise",
    b"fall-rise",
    b"fall-rise2",
    b"rise-fall",
    b"fall-rise3",
    b"fall-rise4",
    b"fall2",
    b"rise2",
    b"rise-fall-rise",
];
const fn default_record() -> [u8; RECORD_BYTES] {
    let mut r = [0; RECORD_BYTES];
    r[17] = 40;
    r[18] = 24;
    r[19] = 8;
    let values = [
        46, 57, 0, 16, 0, 0, 255, 78, 50, 255, 3, 5, 249, 249, 249, 249, 249, 249, 0, 64, 8, 0, 70,
        18, 24, 12, 0, 70, 18, 24, 12, 0,
    ];
    let mut i = 0;
    while i < values.len() {
        r[24 + i] = values[i];
        i += 1;
    }
    r
}
fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn name(bytes: &[u8]) -> [u8; 12] {
    let mut out = [0; 12];
    let n = string(bytes).len().min(11);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}
struct Scratch {
    bytes: [u8; 512],
    length: usize,
}
impl Scratch {
    fn new() -> Self {
        Self {
            bytes: [0; 512],
            length: 0,
        }
    }
    fn add(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(self.bytes.len() - self.length);
        self.bytes[self.length..self.length + n].copy_from_slice(&bytes[..n]);
        self.length += n;
    }
    fn view(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
impl Write for Scratch {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.add(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub trait Host {
    /// Initialized raw diagnostic/summary bytes, borrowed only during this call.
    fn log(&mut self, bytes: &[u8]);
    fn close_log(&mut self);
    fn reload(&mut self);
    /// Input text/bare/output roles 0/1/2; no borrowed path crosses an effect.
    fn file_error(&mut self, role: usize, error: io::Error) -> i32;
}
pub struct Paths<'a> {
    pub text: &'a Path,
    pub bare: &'a Path,
    pub output: &'a Path,
}
/// Fixed tune/name arenas plus retained source bytes; no per-token/tune heap work.
pub struct Workspace {
    pub sources: Sources,
    names: [[u8; 12]; MAX_TUNES],
    records: [[u8; RECORD_BYTES]; MAX_TUNES],
    count: usize,
    token: [u8; 256],
    line: i32,
    errors: i32,
}
impl Workspace {
    pub fn new(classes: Classes) -> Self {
        Self {
            sources: Sources::new(classes),
            names: [[0; 12]; MAX_TUNES],
            records: [[0; RECORD_BYTES]; MAX_TUNES],
            count: 0,
            token: [0; 256],
            line: 1,
            errors: 0,
        }
    }
    fn error(&mut self, host: &mut impl Host, prefix: &[u8], text: &[u8], suffix: &[u8]) {
        let mut out = Scratch::new();
        let _ = write!(out, "({}): ", self.line - 1);
        out.add(prefix);
        out.add(string(text));
        out.add(suffix);
        out.add(b"\n");
        host.log(out.view());
        self.errors += 1;
    }
    fn next(&mut self, host: &mut impl Host, kind: i32) -> i32 {
        let Some(t) = self.sources.next(0, kind, self.line) else {
            return -2;
        };
        self.line = t.line;
        if t.length >= 0 {
            self.token[..t.length as usize + 1].copy_from_slice(&t.bytes[..t.length as usize + 1]);
        }
        if t.status == -3 {
            self.error(host, b"Item exceeds 255 bytes", b"", b"");
            return -1;
        }
        if t.status != 0 {
            return t.status;
        }
        match t.diagnostic {
            1 => self.error(host, b"Expected an unsigned number", b"", b""),
            2 => self.error(host, b"Expected a number", b"", b""),
            3 => self.error(host, b"Number exceeds signed integer range", b"", b""),
            _ => {}
        }
        if kind == 3 || kind == 4 {
            t.value
        } else if kind == 10 {
            KEYWORDS
                .iter()
                .position(|s| *s == string(&self.token))
                .map_or(-1, |i| i as i32 + 1)
        } else {
            0
        }
    }
    fn envelope(&mut self, host: &mut impl Host) -> Option<u8> {
        self.next(host, 2);
        let result = ENVELOPES
            .iter()
            .position(|s| *s == string(&self.token))
            .map(|i| i as u8 * 2);
        if result.is_none() {
            let text = self.token;
            self.error(host, b"Bad envelope name: '", &text, b"'");
        }
        result
    }
    /// Parse already admitted source slot zero. Outputs stay in fixed arenas.
    pub fn parse(&mut self, host: &mut impl Host, width: usize) -> io::Result<()> {
        self.discover(width)?;
        self.parse_pass(host);
        Ok(())
    }
    fn discover(&mut self, width: usize) -> io::Result<()> {
        if !(2..=4096).contains(&width) || self.sources.source(0).is_none() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.names.fill([0; 12]);
        self.records.fill([0; RECORD_BYTES]);
        self.count = PRESETS.len();
        self.token.fill(0);
        self.line = 1;
        self.errors = 0;
        for (i, n) in PRESETS.iter().enumerate() {
            self.names[i] = name(n);
        }
        self.sources.source(0).unwrap().rewind();
        let mut chunk = [0; 4096];
        while let Some(n) = self.sources.source(0).unwrap().chunk(&mut chunk[..width]) {
            if n >= 5
                && &chunk[..4] == b"tune"
                && self.sources.classes.spaces[chunk[4] as usize] != 0
            {
                let mut start = 5;
                while start < n && self.sources.classes.spaces[chunk[start] as usize] != 0 {
                    start += 1;
                }
                let mut end = start;
                while end < n
                    && end - start < 11
                    && chunk[end] != 0
                    && self.sources.classes.spaces[chunk[end] as usize] == 0
                {
                    end += 1;
                }
                let found = name(&chunk[start..end]);
                if !self.names[..self.count]
                    .iter()
                    .any(|n| string(n) == string(&found))
                {
                    self.names[self.count] = found;
                    self.count += 1;
                    if self.count == MAX_TUNES {
                        break;
                    }
                }
            }
        }
        self.sources.source(0).unwrap().rewind();
        Ok(())
    }
    fn parse_pass(&mut self, host: &mut impl Host) {
        let mut tune = default_record();
        let mut found = 0;
        let mut number = 0;
        let mut split = false;
        let mut onset = false;
        let mut last = false;
        while !self.sources.source(0).unwrap().ended() {
            match self.next(host, 10) {
                1 => {
                    split = false;
                    tune = default_record();
                    self.next(host, 2);
                    tune[..12].copy_from_slice(&name(&self.token));
                    found = 0;
                    number = 0;
                    if let Some(i) = self.names[..self.count]
                        .iter()
                        .position(|n| string(n) == string(&tune[..12]))
                    {
                        number = i;
                        found = if self.records[i][0] != 0 { 2 } else { 1 };
                    }
                    if found == 2 {
                        self.error(host, b"Duplicate tune name: '", &tune[..12], b"'");
                    }
                    if found == 0 {
                        self.error(host, b"Bad tune name: '", &tune[..12], b";");
                    }
                }
                2 => {
                    if found == 0 {
                        continue;
                    }
                    // Legacy onset/last flags persist across tune declarations.
                    if !onset {
                        tune[36] = tune[37];
                        tune[39] = tune[40];
                    }
                    if !last {
                        tune[38] = tune[37];
                        tune[41] = tune[40];
                    }
                    self.records[number] = tune;
                }
                3 => {
                    tune[24] = self.next(host, 3) as u8;
                    tune[25] = self.next(host, 3) as u8;
                }
                4 => {
                    tune[30] = self.next(host, 3) as u8;
                    tune[36] = self.next(host, 4) as u8;
                    tune[39] = self.next(host, 4) as u8;
                    onset = true;
                }
                5 => {
                    for i in [34, 31, 32] {
                        tune[i] = self.next(host, 3) as u8;
                    }
                    tune[37] = self.next(host, 4) as u8;
                    tune[40] = self.next(host, 4) as u8;
                }
                6 => {
                    if let Some(env) = self.envelope(host) {
                        tune[26] = env;
                    }
                    tune[27] = self.next(host, 3) as u8;
                }
                7 => {
                    let mut n = 0;
                    for i in 0..8 {
                        let (c, line) = self.sources.source(0).unwrap().check_next(self.line);
                        self.line = line;
                        if c != 45 && (c < 0 || self.sources.classes.digits[c as usize] == 0) {
                            break;
                        }
                        let value = self.next(host, 4);
                        if let Some(scaled) = value.checked_mul(64) {
                            tune[16 + i] = (scaled / 100) as u8;
                        } else {
                            self.error(
                                host,
                                b"Head extension exceeds signed integer range",
                                b"",
                                b"",
                            );
                            tune[16 + i] = 0;
                        }
                        n = i + 1;
                    }
                    tune[35] = n as u8;
                }
                8 => {
                    tune[33] = self.next(host, 3) as u8;
                    tune[38] = self.next(host, 4) as u8;
                    tune[41] = self.next(host, 4) as u8;
                    last = true;
                }
                9 => {
                    if let Some(env) = self.envelope(host) {
                        tune[42] = env;
                        tune[43] = self.next(host, 3) as u8;
                        tune[44] = self.next(host, 3) as u8;
                    }
                }
                10 => {
                    if let Some(env) = self.envelope(host) {
                        tune[45] = env;
                        for byte in &mut tune[46..50] {
                            *byte = self.next(host, 3) as u8;
                        }
                        if !split {
                            let value: [u8; 5] = tune[45..50].try_into().unwrap();
                            tune[50..55].copy_from_slice(&value);
                        }
                    }
                }
                11 => {
                    if let Some(env) = self.envelope(host) {
                        split = true;
                        tune[50] = env;
                        for byte in &mut tune[51..55] {
                            *byte = self.next(host, 3) as u8;
                        }
                        self.next(host, 2);
                        self.token[12] = 0;
                        if let Some(i) = self.names[..self.count]
                            .iter()
                            .position(|n| string(n) == string(&self.token))
                        {
                            tune[55] = i as u8;
                        } else {
                            let text = self.token;
                            self.error(host, b"Tune '", &text, b"' not found");
                        }
                    }
                }
                _ => {
                    let text = self.token;
                    self.error(host, b"Unexpected: '", &text, b"'");
                }
            }
        }
        for (i, n) in PRESETS.iter().enumerate() {
            if self.records[i][0] == 0 {
                self.error(host, b"Tune '", n, b"' not defined");
            }
        }
    }
    pub fn records(&self) -> &[[u8; RECORD_BYTES]] {
        &self.records[..self.count]
    }
    pub fn errors(&self) -> i32 {
        self.errors
    }
    /// Own source selection, output completion, summary and reload ordering.
    pub fn compile(
        &mut self,
        host: &mut impl Host,
        paths: &Paths<'_>,
        width: usize,
        mode: TextMode,
    ) -> i32 {
        if !(2..=4096).contains(&width) {
            return FAILED;
        }
        let result = self
            .sources
            .open(0, paths.text, mode)
            .or_else(|_| self.sources.open(0, paths.bare, mode));
        if let Err(error) = result {
            self.sources.clear();
            host.close_log();
            return host.file_error(1, error);
        }
        if self.discover(width).is_err() {
            self.sources.clear();
            host.close_log();
            return FAILED;
        }
        // Name discovery precedes truncating the output, just as in C. Parsing
        // has no native effects apart from log delivery; fixed arenas replace calloc.
        // Open the output before diagnostics from the semantic pass.
        let mut writer = match Writer::create(paths.output) {
            Ok(writer) => writer,
            Err(error) => {
                self.sources.clear();
                host.close_log();
                return host.file_error(2, error);
            }
        };
        self.parse_pass(host);
        let mut result = Ok(());
        for record in self.records() {
            if let Err(error) = writer.write(record) {
                result = Err(error);
                break;
            }
        }
        if result.is_ok() {
            result = writer.finish();
        } else {
            drop(writer);
        }
        self.sources.clear();
        if let Err(error) = result {
            host.close_log();
            return host.file_error(2, error);
        }
        let mut summary = Scratch::new();
        let _ = writeln!(
            summary,
            "Compiled {} intonation tunes: {} errors.",
            self.count, self.errors
        );
        host.log(summary.view());
        host.reload();
        if self.errors > 0 {
            FAILED
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        log: Vec<u8>,
        calls: Vec<u8>,
        role: usize,
    }
    impl Host for Effects {
        fn log(&mut self, bytes: &[u8]) {
            self.log.extend_from_slice(bytes);
            self.calls.push(1);
        }
        fn close_log(&mut self) {
            self.calls.push(2);
        }
        fn reload(&mut self) {
            self.calls.push(3);
        }
        fn file_error(&mut self, role: usize, _: io::Error) -> i32 {
            self.role = role;
            self.calls.push(4);
            5
        }
    }
    fn workspace() -> Workspace {
        Workspace::new(Classes {
            spaces: std::array::from_fn(|i| u8::from((i as u8).is_ascii_whitespace() || i == 11)),
            digits: std::array::from_fn(|i| u8::from((i as u8).is_ascii_digit())),
        })
    }
    const BASIC:&[u8]=b"tune s1\nhead 4 80 50 -8 -5\nendtune\ntune c1\nendtune\ntune q1\nendtune\ntune e1\nendtune\n";
    #[test]
    fn defaults_cross_tune_flags_split_and_all_record_bytes_are_owned() {
        let mut w = workspace();
        let mut h = Effects::default();
        w.sources.bind(0, BASIC, TextMode::Binary).unwrap();
        w.parse(&mut h, 1024).unwrap();
        assert_eq!(w.errors(), 0);
        assert!(h.log.is_empty());
        assert_eq!(w.records().len(), 4);
        let mut expected = default_record();
        expected[..12].copy_from_slice(&name(b"s1"));
        expected[34] = 4;
        expected[31] = 80;
        expected[32] = 50;
        expected[36..39].fill((-8i8) as u8);
        expected[39..42].fill((-5i8) as u8);
        assert_eq!(w.records()[0], expected);
        let bytes=b"tune s1\nonset 99 7 9\nheadlast 22 11 13\nheadextend -100 50\nsplit rise 1 2 3 4 q1\nnucleus fall 30 40 50 60\nendtune\ntune c1\nhead 4 70 50 -8 -5\nendtune\ntune q1\nendtune\ntune e1\nendtune\n";
        w.sources.bind(0, bytes, TextMode::Binary).unwrap();
        w.parse(&mut h, 1024).unwrap();
        assert_eq!(w.errors(), 0);
        let r = &w.records()[0];
        assert_eq!(&r[16..18], &[192, 32]);
        assert_eq!(r[35], 2);
        assert_eq!(&r[50..56], &[2, 1, 2, 3, 4, 2]);
        assert_eq!(
            (r[30], r[33], r[36], r[38], r[39], r[41]),
            (99, 22, 7, 11, 9, 13)
        );
        // Original flags are not reset by the next tune declaration.
        let r = &w.records()[1];
        assert_eq!((r[36], r[38], r[39], r[41]), (249, 249, 249, 249));
        assert!(w.records().iter().all(|r| r[56..].iter().all(|b| *b == 0)));
    }
    #[test]
    fn duplicate_missing_names_bad_envelopes_and_extra_names_keep_diagnostics_and_output() {
        let mut w = workspace();
        let mut h = Effects::default();
        let mut bytes = BASIC.to_vec();
        bytes.extend_from_slice(
            b"tune s1\nheadenv broken 9\nprehead -4 x\nendtune\n  tune hidden\nendtune\n",
        );
        w.sources.bind(0, &bytes, TextMode::Binary).unwrap();
        w.parse(&mut h, 1024).unwrap();
        assert_eq!(w.records().len(), 4);
        assert_eq!(w.records()[0][27], 9);
        assert_eq!(w.errors(), 5);
        let log = String::from_utf8(h.log).unwrap();
        assert!(log.contains("Duplicate tune name: 's1'"));
        assert!(log.contains("Bad envelope name: 'broken'"));
        assert!(log.contains("Expected an unsigned number"));
        assert!(log.contains("Expected a number"));
        assert!(log.contains("Bad tune name: 'hidden;"));
    }
    #[test]
    fn tune_bound_long_tokens_numeric_and_scaling_overflow_reject_without_uninitialized_reads() {
        let mut w = workspace();
        let mut h = Effects::default();
        let mut bytes = BASIC.to_vec();
        for i in 0..110 {
            bytes.extend_from_slice(format!("tune n{i}\nendtune\n").as_bytes());
        }
        w.sources.bind(0, &bytes, TextMode::Binary).unwrap();
        w.parse(&mut h, 1024).unwrap();
        assert_eq!(w.records().len(), 100);
        assert_eq!(w.errors(), 14);
        let mut bytes = BASIC.to_vec();
        bytes.extend_from_slice(b"tune s1\nheadextend 2147483647\nprehead 2147483648 9\n");
        bytes.extend_from_slice(&[b'x'; 10000]);
        bytes.extend_from_slice(b"\nendtune\n");
        w.sources.bind(0, &bytes, TextMode::Binary).unwrap();
        h.log.clear();
        w.parse(&mut h, 1024).unwrap();
        let log = String::from_utf8(h.log).unwrap();
        assert!(log.contains("Head extension exceeds signed integer range"));
        assert!(log.contains("Number exceeds signed integer range"));
        assert!(log.contains("Item exceeds 255 bytes"));
        assert_eq!(w.records()[0][16], 0);
        assert_eq!(w.records()[0][24], 0);
    }
    #[test]
    fn windows_text_and_repeated_bindings_match_binary_tune_records() {
        let mut w = workspace();
        let mut h = Effects::default();
        w.sources.bind(0, BASIC, TextMode::Binary).unwrap();
        w.parse(&mut h, 1024).unwrap();
        let expected = w.records().to_vec();
        let mut bytes = Vec::new();
        for b in BASIC {
            if *b == 10 {
                bytes.push(13);
            }
            bytes.push(*b);
        }
        bytes.extend_from_slice(b"\x1atune hidden\nendtune\n");
        for _ in 0..16 {
            w.sources.bind(0, &bytes, TextMode::Windows).unwrap();
            w.parse(&mut h, 1024).unwrap();
            assert_eq!(w.records(), expected);
            assert_eq!(w.errors(), 0);
        }
        assert!(w.parse(&mut h, 1).is_err());
        w.sources.clear();
        assert!(w.parse(&mut h, 1024).is_err());
    }
    #[test]
    fn real_driver_source_fallback_output_errors_completion_and_reload_order() {
        let _lock = crate::engine_io::writer_test_lock();
        let root = std::env::temp_dir().join(format!("espeak-intonation-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let text = root.join("intonation.txt");
        let bare = root.join("intonation");
        let output = root.join("intonations");
        let paths = Paths {
            text: &text,
            bare: &bare,
            output: &output,
        };
        let mut w = workspace();
        let mut h = Effects::default();
        assert_eq!(w.compile(&mut h, &paths, 1024, TextMode::Binary), 5);
        assert_eq!(h.calls, [2, 4]);
        assert_eq!(h.role, 1);
        assert!(!output.exists());
        std::fs::write(&bare, BASIC).unwrap();
        h.calls.clear();
        assert_eq!(w.compile(&mut h, &paths, 1024, TextMode::Binary), 0);
        assert_eq!(h.calls, [1, 3]);
        let expected = w.records().concat();
        assert_eq!(std::fs::read(&output).unwrap(), expected);
        let mut changed = BASIC.to_vec();
        changed.extend_from_slice(b"tune added\nendtune\n");
        std::fs::write(&text, changed).unwrap();
        h.calls.clear();
        assert_eq!(w.compile(&mut h, &paths, 1024, TextMode::Binary), 0);
        assert_eq!(std::fs::read(&output).unwrap().len(), 5 * 68);
        assert_eq!(h.calls, [1, 3]);
        assert!(w.sources.source(0).is_none());
        let bad = Paths {
            text: &text,
            bare: &bare,
            output: &root,
        };
        h.calls.clear();
        assert_eq!(w.compile(&mut h, &bad, 1024, TextMode::Binary), 5);
        assert_eq!(h.calls, [2, 4]);
        assert_eq!(h.role, 2);
        assert!(w.sources.source(0).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
