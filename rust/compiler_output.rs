//! Owned phoneme compiler file set, bounded staging and completed proactor output.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{engine_io::Writer, spect_compile::errno, voice_storage::compat_path};
pub const MANIFEST: usize = 0;
pub const DATA: usize = 1;
pub const INDEX: usize = 2;
pub const TABLE: usize = 3;
pub const LOG: usize = 4;
pub const FILES: usize = 5;
pub const MAX_WRITE: usize = 16 * 1024 * 1024;
pub const HEADER: &[u8] = b"# This file lists the type of data that has been compiled into the\n# phondata file\n#\n# The first character of a line indicates the type of data:\n#   S - A SPECT_SEQ structure\n#   W - A wavefile segment\n#   E - An envelope\n#\n# Address is the displacement within phondata of this item\n#\n#  Address  Data file\n#  -------  ---------\n";
const NAMES: [&[u8]; FILES] = [
    b"phondata-manifest",
    b"phondata",
    b"phonindex",
    b"phontab",
    b"compile_prog_log",
];
pub trait Host {
    fn stderr(&mut self, bytes: &[u8]) -> Result<(), i32>;
}
pub struct Request<'a> {
    pub source: &'a [u8],
    pub destination: &'a [u8],
    pub width: usize,
    pub version: u32,
    pub rate: i32,
    pub windows_text: bool,
}
pub struct Workspace {
    files: [Option<Writer>; FILES],
    filenames: [[u8; 4096]; FILES],
    lengths: [usize; FILES],
    active: bool,
    fallback: bool,
    windows_text: bool,
    error: i32,
    error_file: usize,
    /// Compatibility inputs are copied here before any I/O, then reused.
    pub(crate) input: Vec<u8>,
    text: Vec<u8>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            files: std::array::from_fn(|_| None),
            filenames: [[0; 4096]; FILES],
            lengths: [0; FILES],
            active: false,
            fallback: false,
            windows_text: false,
            error: 0,
            error_file: 0,
            input: Vec::new(),
            text: Vec::new(),
        }
    }
}
impl Workspace {
    pub fn error(&self) -> i32 {
        self.error
    }
    pub fn error_name(&self) -> &[u8] {
        &self.filenames[self.error_file][..self.lengths[self.error_file]]
    }
    pub fn mask(&self) -> u32 {
        self.files
            .iter()
            .enumerate()
            .fold(0, |mask, (i, f)| mask | (u32::from(f.is_some()) << i))
    }
    pub fn remember(&mut self, code: i32, file: usize) -> i32 {
        if self.error == 0 && code != 0 {
            self.error = code;
            self.error_file = file.min(FILES - 1);
        }
        code
    }
    pub fn begin(&mut self, host: &mut impl Host, r: &Request<'_>) -> Result<(), i32> {
        if self.active
            || !(2..=4096).contains(&r.width)
            || r.source.len() > 4095
            || r.destination.len() > 4095
        {
            return Err(22);
        }
        self.error = 0;
        self.error_file = 0;
        self.fallback = false;
        self.windows_text = r.windows_text;
        self.input.clear();
        self.text.clear();
        self.active = true;
        for (i, name) in NAMES.iter().enumerate() {
            let base = if i == LOG { r.source } else { r.destination };
            let mut n = 0;
            for part in [base, b"/".as_slice(), *name] {
                let count = part.len().min(r.width - 1 - n);
                self.filenames[i][n..n + count].copy_from_slice(&part[..count]);
                n += count;
            }
            self.filenames[i][n] = 0;
            self.lengths[i] = n;
        }
        // Preserve creation/partial-failure order: manifest/header, data,
        // index, table, optional log, then initial binary words.
        for i in 0..FILES {
            let opened = compat_path(&self.filenames[i][..self.lengths[i]])
                .and_then(|path| Writer::create_compiler_output(&path));
            match opened {
                Ok(writer) => self.files[i] = Some(writer),
                Err(_) if i == MANIFEST => self.fallback = true,
                Err(_) if i == LOG => {}
                Err(error) => {
                    let code = errno(&error);
                    self.remember(code, i);
                    self.finish();
                    return Err(code);
                }
            }
            if i == MANIFEST {
                if let Err(code) = self.write(host, MANIFEST, HEADER) {
                    self.finish();
                    return Err(code);
                }
            }
        }
        let mut words = [0; 8];
        words[..4].copy_from_slice(&r.version.to_le_bytes());
        words[4..].copy_from_slice(&r.rate.to_le_bytes());
        if let Err(code) = self
            .write(host, DATA, &words)
            .and_then(|_| self.write(host, INDEX, &words[..4]))
        {
            self.finish();
            return Err(code);
        }
        Ok(())
    }
    pub fn write(&mut self, host: &mut impl Host, file: usize, bytes: &[u8]) -> Result<(), i32> {
        if !self.active || file >= FILES || bytes.len() > MAX_WRITE {
            return Err(22);
        }
        if self.error != 0 {
            return Err(self.error);
        }
        if file == MANIFEST && self.fallback {
            // Borrowed C stderr already has the platform's text semantics.
            return host.stderr(bytes).inspect_err(|code| {
                self.remember(*code, file);
            });
        }
        if file == LOG && self.files[file].is_none() {
            return Ok(());
        }
        let Some(writer) = self.files[file].as_mut() else {
            return Err(9);
        };
        let result = if file == MANIFEST && self.windows_text {
            let length = bytes
                .len()
                .checked_add(bytes.iter().filter(|b| **b == b'\n').count())
                .filter(|v| *v <= MAX_WRITE * 2)
                .ok_or(22)?;
            self.text.clear();
            if self.text.try_reserve(length).is_err() {
                return Err(self.remember(12, file));
            }
            for b in bytes {
                if *b == b'\n' {
                    self.text.push(b'\r');
                }
                self.text.push(*b);
            }
            writer.write(&self.text)
        } else {
            writer.write(bytes)
        };
        result.map_err(|error| {
            let code = errno(&error);
            self.remember(code, file);
            code
        })
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn write_snapshot(&mut self, host: &mut impl Host, file: usize) -> Result<(), i32> {
        // Move the retained input owner temporarily; no foreign input loan can
        // cross the proactor wait or stderr callback.
        let mut input = std::mem::take(&mut self.input);
        let result = self.write(host, file, &input);
        input.clear();
        self.input = input;
        result
    }
    pub fn position(&self, file: usize) -> Result<u64, i32> {
        if !self.active || file >= FILES {
            return Err(22);
        }
        if self.error != 0 {
            return Err(self.error);
        }
        self.files[file].as_ref().map(Writer::position).ok_or(9)
    }
    pub fn flush(&mut self, file: usize) -> Result<(), i32> {
        if !self.active || file >= FILES {
            return Err(22);
        }
        if self.error != 0 {
            return Err(self.error);
        }
        if file == MANIFEST && self.fallback {
            return Ok(());
        }
        if let Some(writer) = self.files[file].as_mut() {
            if let Err(error) = writer.flush() {
                let code = errno(&error);
                self.remember(code, file);
                return Err(code);
            }
        }
        Ok(())
    }
    /// Complete every admitted file even after a failure, then release all
    /// registrations and OS handles. Drop never starts I/O.
    pub fn finish(&mut self) -> i32 {
        for i in 0..FILES {
            if let Some(writer) = self.files[i].take() {
                if let Err(error) = writer.finish() {
                    self.remember(errno(&error), i);
                }
            }
        }
        self.active = false;
        self.fallback = false;
        self.input.clear();
        self.text.clear();
        self.error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_io::{read_file, writer_test_lock, CHUNK_BYTES};
    use std::path::PathBuf;

    #[derive(Default)]
    struct Capture {
        bytes: Vec<u8>,
        fail: i32,
    }
    impl Host for Capture {
        fn stderr(&mut self, bytes: &[u8]) -> Result<(), i32> {
            self.bytes.extend_from_slice(bytes);
            if self.fail == 0 {
                Ok(())
            } else {
                Err(self.fail)
            }
        }
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("espeak-output-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn bytes(&self) -> Vec<u8> {
            self.0.to_string_lossy().as_bytes().to_vec()
        }
        fn block(&self, name: &str) {
            std::fs::create_dir(self.0.join(name)).unwrap();
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn request(path: &[u8]) -> Request<'_> {
        Request {
            source: path,
            destination: path,
            width: 4096,
            version: 0x12345678,
            rate: 22050,
            windows_text: false,
        }
    }
    #[test]
    fn five_outputs_complete_chunks_with_source_reads_and_retained_storage() {
        let _serial = writer_test_lock();
        let directory = Directory::new("five");
        let path = directory.bytes();
        let mut host = Capture::default();
        let mut workspace = Workspace::default();
        let data: Vec<u8> = (0..CHUNK_BYTES * 3 + 41).map(|i| (i * 19) as u8).collect();
        std::fs::write(directory.0.join("source"), &data).unwrap();
        let mut old = None;
        for cycle in 0..64 {
            workspace.begin(&mut host, &request(&path)).unwrap();
            assert_eq!(workspace.mask(), 31);
            let buffers = std::array::from_fn::<_, FILES, _>(|i| {
                workspace.files[i].as_ref().unwrap().test_buffer()
            });
            assert!(buffers.iter().all(|(_, capacity)| *capacity == CHUNK_BYTES));
            if let Some(previous) = old {
                assert_eq!(buffers, previous);
            } else {
                old = Some(buffers);
            }
            assert_eq!(
                Writer::create_compiler_output(&directory.0.join("sixth"))
                    .err()
                    .unwrap()
                    .kind(),
                std::io::ErrorKind::WouldBlock
            );
            assert!(!directory.0.join("sixth").exists());
            assert_eq!(
                read_file(&directory.0.join("source"), data.len()).unwrap(),
                data
            );
            let bytes = if cycle == 0 {
                data.as_slice()
            } else {
                &data[..41]
            };
            for i in 0..FILES {
                workspace.write(&mut host, i, bytes).unwrap();
            }
            assert_eq!(workspace.position(DATA).unwrap(), 8 + bytes.len() as u64);
            workspace.flush(INDEX).unwrap();
            assert_eq!(workspace.finish(), 0);
            for (i, name) in NAMES.iter().enumerate() {
                let mut expected = match i {
                    MANIFEST => HEADER.to_vec(),
                    DATA => [
                        request(&path).version.to_le_bytes(),
                        request(&path).rate.to_le_bytes(),
                    ]
                    .concat(),
                    INDEX => request(&path).version.to_le_bytes().to_vec(),
                    _ => Vec::new(),
                };
                expected.extend_from_slice(bytes);
                assert_eq!(
                    read_file(
                        &directory.0.join(std::str::from_utf8(name).unwrap()),
                        expected.len()
                    )
                    .unwrap(),
                    expected
                );
            }
            assert_eq!(workspace.mask(), 0);
            assert_eq!(workspace.position(DATA), Err(22));
        }
        assert!(host.bytes.is_empty());
    }
    #[test]
    fn partial_creation_failures_close_all_admitted_files_before_reuse() {
        let _serial = writer_test_lock();
        for name in ["phondata", "phonindex", "phontab"] {
            let directory = Directory::new(name);
            let path = directory.bytes();
            directory.block(name);
            let mut host = Capture::default();
            let mut workspace = Workspace::default();
            let code = workspace.begin(&mut host, &request(&path)).unwrap_err();
            assert_ne!(code, 0);
            assert!(workspace.error_name().ends_with(name.as_bytes()));
            assert_eq!(workspace.mask(), 0);
            assert_eq!(workspace.finish(), code);
            assert_eq!(
                std::fs::read(directory.0.join("phondata-manifest")).unwrap(),
                HEADER
            );
            for earlier in ["phondata", "phonindex"]
                .into_iter()
                .take_while(|earlier| *earlier != name)
            {
                assert_eq!(
                    std::fs::metadata(directory.0.join(earlier)).unwrap().len(),
                    0
                );
            }
            std::fs::remove_dir(directory.0.join(name)).unwrap();
            workspace.begin(&mut host, &request(&path)).unwrap();
            assert_eq!(workspace.mask(), 31);
            assert_eq!(workspace.finish(), 0);
        }
    }
    #[test]
    fn optional_streams_and_windows_manifest_text_preserve_exact_bytes() {
        let _serial = writer_test_lock();
        let directory = Directory::new("optional");
        let path = directory.bytes();
        directory.block("phondata-manifest");
        directory.block("compile_prog_log");
        let mut host = Capture::default();
        let mut workspace = Workspace::default();
        let mut r = request(&path);
        r.windows_text = true;
        workspace.begin(&mut host, &r).unwrap();
        assert_eq!(workspace.mask(), 14);
        workspace.write(&mut host, MANIFEST, b"S line\r\n").unwrap();
        workspace.write(&mut host, LOG, b"ignored").unwrap();
        assert_eq!(workspace.finish(), 0);
        assert_eq!(host.bytes, [HEADER, b"S line\r\n"].concat());
        std::fs::remove_dir(directory.0.join("phondata-manifest")).unwrap();
        workspace.begin(&mut host, &r).unwrap();
        workspace
            .write(&mut host, MANIFEST, b"line\r\nline\n")
            .unwrap();
        assert_eq!(workspace.finish(), 0);
        let expected = [HEADER, b"line\r\nline\n"]
            .concat()
            .into_iter()
            .flat_map(|b| {
                if b == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![b]
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            std::fs::read(directory.0.join("phondata-manifest")).unwrap(),
            expected
        );
    }
    #[test]
    fn fallback_errors_are_sticky_and_pending_outputs_still_complete() {
        let _serial = writer_test_lock();
        let directory = Directory::new("errors");
        let path = directory.bytes();
        directory.block("phondata-manifest");
        let mut host = Capture {
            fail: 28,
            ..Capture::default()
        };
        let mut workspace = Workspace::default();
        assert_eq!(workspace.begin(&mut host, &request(&path)), Err(28));
        assert_eq!(workspace.mask(), 0);
        host.fail = 0;
        workspace.begin(&mut host, &request(&path)).unwrap();
        workspace.write(&mut host, DATA, b"pending").unwrap();
        host.fail = 5;
        assert_eq!(workspace.write(&mut host, MANIFEST, b"failure"), Err(5));
        assert_eq!(workspace.write(&mut host, INDEX, b"refused"), Err(5));
        assert_eq!(workspace.finish(), 5);
        assert_eq!(workspace.mask(), 0);
        let expected = [
            request(&path).version.to_le_bytes().as_slice(),
            request(&path).rate.to_le_bytes().as_slice(),
            b"pending",
        ]
        .concat();
        assert_eq!(
            std::fs::read(directory.0.join("phondata")).unwrap(),
            expected
        );
        host.fail = 0;
        workspace.begin(&mut host, &request(&path)).unwrap();
        assert_eq!(workspace.finish(), 0);
        let mut invalid = request(&path);
        invalid.width = 1;
        assert_eq!(workspace.begin(&mut host, &invalid), Err(22));
        assert_eq!(workspace.mask(), 0);
    }
}
