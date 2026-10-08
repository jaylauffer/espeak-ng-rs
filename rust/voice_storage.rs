//! Owned voice catalogue metadata, initialized from files or host-loaded bytes.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme_data::InvalidPhonemeData as Error,
    voice::Directives,
    voice_catalog::{self, Roster},
    voice_selection::{Metadata, Voice},
};
use std::{borrow::Cow, fs, io, path::Path};

pub const COMPAT_CAPACITY: usize = 498;
const READ_CHUNK: usize = 8192;
const MAX_SCAN_ENTRIES: usize = 8192;
const MAX_DEPTH: usize = 64;
const MAX_SCAN_BYTES: usize = 16 * 1024 * 1024;

pub struct Record {
    pub(crate) metadata: Metadata,
    identifier: Box<[u8]>,
    score: i32,
}
impl Record {
    pub fn view(&self) -> Voice<'_> {
        self.metadata
            .view(self.identifier())
            .expect("admitted metadata")
    }
    pub fn identifier(&self) -> &[u8] {
        &self.identifier[..self.identifier.len() - 1]
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn terminated_identifier(&self) -> &[u8] {
        &self.identifier
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Diagnostic {
    GenderOnLanguage,
    InvalidFile,
    Full,
}

/// Incremental fgets-compatible metadata for reusable host I/O buffers. Each
/// input chunk is borrowed only during `feed`; storage stays fixed at 119 bytes.
/// Discard this builder on a parse error; no record has yet been published.
pub struct MetadataChunks {
    metadata: Metadata,
    line: [u8; 119],
    used: usize,
    language_file: bool,
    #[cfg(windows)]
    pending_cr: bool,
    #[cfg(windows)]
    ended: bool,
}
impl MetadataChunks {
    pub fn new(language_file: bool) -> Self {
        Self {
            metadata: Metadata::default(),
            line: [0; 119],
            used: 0,
            language_file,
            #[cfg(windows)]
            pending_cr: false,
            #[cfg(windows)]
            ended: false,
        }
    }
    fn flush(&mut self, diagnostic: &mut impl FnMut(Diagnostic)) -> Result<(), Error> {
        let used = self.used;
        self.used = 0;
        for (key, value) in Directives::new(&self.line[..used], 120)? {
            if self.metadata.apply(key, value)? && self.language_file {
                diagnostic(Diagnostic::GenderOnLanguage);
            }
        }
        Ok(())
    }
    pub fn feed(
        &mut self,
        bytes: &[u8],
        mut diagnostic: impl FnMut(Diagnostic),
    ) -> Result<(), Error> {
        for &byte in bytes {
            #[cfg(windows)]
            {
                if self.ended {
                    break;
                }
                if self.pending_cr {
                    self.pending_cr = false;
                    if byte != b'\n' {
                        self.push(b'\r', &mut diagnostic)?;
                    }
                }
                if byte == 0x1a {
                    self.ended = true;
                    break;
                }
                if byte == b'\r' {
                    self.pending_cr = true;
                    continue;
                }
            }
            self.push(byte, &mut diagnostic)?;
        }
        Ok(())
    }
    fn push(&mut self, byte: u8, diagnostic: &mut impl FnMut(Diagnostic)) -> Result<(), Error> {
        self.line[self.used] = byte;
        self.used += 1;
        if self.used == 119 || byte == b'\n' {
            self.flush(diagnostic)?;
        }
        Ok(())
    }
    pub fn finish(mut self, mut diagnostic: impl FnMut(Diagnostic)) -> Result<Metadata, Error> {
        #[cfg(windows)]
        if self.pending_cr {
            self.push(b'\r', &mut diagnostic)?;
        }
        if self.used > 0 {
            self.flush(&mut diagnostic)?;
        }
        Ok(self.metadata)
    }
}

/// Metadata records remain in stable boxes for compatibility pointer lifetimes.
/// Construction and file scanning are initialization/worker work. `insert`
/// also accepts resident bytes supplied by the caller's host proactor.
pub struct Catalog {
    #[allow(clippy::vec_box)] // stable metadata addresses across insertion/sorting
    records: Vec<Box<Record>>,
    capacity: usize,
}
impl Catalog {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if capacity == 0 || capacity > voice_catalog::MAX_VOICES {
            return Err(Error("voice catalogue capacity must be 1..=499"));
        }
        Ok(Self {
            records: Vec::with_capacity(capacity),
            capacity,
        })
    }
    pub fn records(&self) -> impl ExactSizeIterator<Item = &Record> {
        self.records.iter().map(Box::as_ref)
    }
    pub fn insert(
        &mut self,
        identifier: &[u8],
        bytes: &[u8],
        language_file: bool,
        mut diagnostic: impl FnMut(Diagnostic, &[u8]),
    ) -> Result<bool, Error> {
        if self.records.len() == self.capacity {
            diagnostic(Diagnostic::Full, identifier);
            return Ok(false);
        }
        if identifier.is_empty() || identifier.len() >= 4096 || identifier.contains(&0) {
            return Err(Error("voice identifier/file exceeds admission bound"));
        }
        let mut builder = MetadataChunks::new(language_file);
        builder.feed(bytes, |kind| diagnostic(kind, identifier))?;
        let metadata = builder.finish(|kind| diagnostic(kind, identifier))?;
        self.admit(identifier, metadata)
    }
    fn admit(&mut self, identifier: &[u8], metadata: Metadata) -> Result<bool, Error> {
        if identifier.is_empty() || identifier.len() >= 4096 || identifier.contains(&0) {
            return Err(Error("voice identifier exceeds admission bound"));
        }
        if metadata.language_count == 0 {
            return Ok(false);
        }
        // Name/language fields are initialized fixed storage; only the admitted
        // identifier needs variable storage. Both allocations live with the record.
        let mut terminated = Vec::with_capacity(identifier.len() + 1);
        terminated.extend_from_slice(identifier);
        terminated.push(0);
        self.records.push(Box::new(Record {
            metadata,
            identifier: terminated.into_boxed_slice(),
            score: 0,
        }));
        Ok(true)
    }
    pub fn set_score(&mut self, index: usize, score: i32) -> Result<(), Error> {
        self.records
            .get_mut(index)
            .ok_or(Error("voice index outside catalogue"))?
            .score = score;
        Ok(())
    }
    pub fn sort(&mut self) {
        self.records
            .sort_by(|a, b| voice_catalog::name_order(a.view(), b.view()));
    }
    /// Serialized compatibility fallback when the caller supplies no host.
    /// One bounded scratch buffer is reused across files; no background scheduler
    /// or worker is created. Missing/unreadable files are skipped as in C.
    pub fn load(root: &Path, mut diagnostic: impl FnMut(Diagnostic, &[u8])) -> io::Result<Self> {
        let mut catalog = Self::new(COMPAT_CAPACITY).map_err(io::Error::other)?;
        let mut scan = Scan {
            entries: 0,
            bytes: 0,
        };
        for (directory, language) in [("voices", false), ("lang", true)] {
            let base = root.join(directory);
            catalog.scan(&base, &base, language, 0, &mut scan, &mut diagnostic)?;
        }
        Ok(catalog)
    }
    fn scan(
        &mut self,
        base: &Path,
        path: &Path,
        language: bool,
        depth: usize,
        scan: &mut Scan,
        diagnostic: &mut impl FnMut(Diagnostic, &[u8]),
    ) -> io::Result<()> {
        if depth > MAX_DEPTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "voice directory depth exceeds 64",
            ));
        }
        let Ok(directory) = fs::read_dir(path) else {
            return Ok(());
        };
        for entry in directory {
            if self.records.len() == self.capacity {
                diagnostic(Diagnostic::Full, b"");
                break;
            }
            scan.entries += 1;
            if scan.entries > MAX_SCAN_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "voice directory entry limit exceeded",
                ));
            }
            let Ok(entry) = entry else { continue };
            if entry.file_name().as_encoded_bytes().starts_with(b".") {
                continue;
            }
            let path = entry.path();
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if metadata.is_dir() {
                self.scan(base, &path, language, depth + 1, scan, diagnostic)?;
                continue;
            }
            if !metadata.is_file() || metadata.len() == 0 {
                continue;
            }
            let Ok(identifier) = path_bytes(path.strip_prefix(base).map_err(io::Error::other)?)
            else {
                continue;
            };
            let identifier = identifier.as_ref();
            // through the engine's reader (the proactor, where built); a file
            // that cannot be opened is skipped, one that fails to read is invalid
            let contents = match crate::engine_io::read_file(&path, MAX_SCAN_BYTES) {
                Ok(contents) => contents,
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    return Err(scan_limit()); // larger than the whole scan's budget
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                    ) =>
                {
                    continue;
                }
                Err(_) => {
                    diagnostic(Diagnostic::InvalidFile, identifier);
                    continue;
                }
            };
            let mut builder = MetadataChunks::new(language);
            let mut failed = false;
            for chunk in contents.chunks(READ_CHUNK) {
                scan.bytes = scan
                    .bytes
                    .checked_add(chunk.len())
                    .filter(|bytes| *bytes <= MAX_SCAN_BYTES)
                    .ok_or_else(scan_limit)?;
                if builder
                    .feed(chunk, |kind| diagnostic(kind, identifier))
                    .is_err()
                {
                    failed = true;
                    break;
                }
                #[cfg(windows)]
                if builder.ended {
                    break;
                }
            }
            if failed {
                diagnostic(Diagnostic::InvalidFile, identifier);
                continue;
            }
            let result = builder
                .finish(|kind| diagnostic(kind, identifier))
                .and_then(|metadata| self.admit(identifier, metadata));
            if result.is_err() {
                diagnostic(Diagnostic::InvalidFile, identifier);
            }
        }
        Ok(())
    }
}
fn scan_limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "voice catalogue scan exceeds byte limit",
    )
}
struct Scan {
    entries: usize,
    bytes: usize,
}
#[cfg(not(windows))]
fn path_bytes(path: &Path) -> io::Result<Cow<'_, [u8]>> {
    Ok(Cow::Borrowed(path.as_os_str().as_encoded_bytes()))
}
#[cfg(windows)]
fn path_bytes(path: &Path) -> io::Result<Cow<'_, [u8]>> {
    windows::encode(path).map(Cow::Owned)
}
#[cfg(feature = "c-abi")]
pub(crate) fn compat_path(bytes: &[u8]) -> io::Result<std::path::PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(Path::new(std::ffi::OsStr::from_bytes(bytes)).to_path_buf())
    }
    #[cfg(windows)]
    {
        windows::decode(bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(Path::new(std::str::from_utf8(bytes).map_err(io::Error::other)?).to_path_buf())
    }
}
#[cfg(windows)]
mod windows {
    use super::*;
    #[cfg(feature = "c-abi")]
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
    use std::{os::windows::ffi::OsStrExt, ptr};
    // Match the legacy FindFirstFileA/fopen path representation using the active
    // Windows code page. UTF-8-only conversion would reject valid legacy paths.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetACP() -> u32;
        #[cfg(feature = "c-abi")]
        fn MultiByteToWideChar(
            page: u32,
            flags: u32,
            input: *const u8,
            length: i32,
            output: *mut u16,
            capacity: i32,
        ) -> i32;
        fn WideCharToMultiByte(
            page: u32,
            flags: u32,
            input: *const u16,
            length: i32,
            output: *mut u8,
            capacity: i32,
            default: *const u8,
            used: *mut i32,
        ) -> i32;
    }
    #[cfg(feature = "c-abi")]
    pub fn decode(bytes: &[u8]) -> io::Result<PathBuf> {
        if bytes.is_empty() {
            return Ok(PathBuf::new());
        }
        let length = i32::try_from(bytes.len()).map_err(io::Error::other)?;
        // SAFETY: input is retained and explicit length is checked; query writes
        // nothing, then the initialized UTF-16 output has exactly the queried size.
        let size = unsafe { MultiByteToWideChar(0, 0, bytes.as_ptr(), length, ptr::null_mut(), 0) };
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut output = vec![0; size as usize];
        // SAFETY: retained input and initialized disjoint output spans match sizes.
        if unsafe { MultiByteToWideChar(0, 0, bytes.as_ptr(), length, output.as_mut_ptr(), size) }
            != size
        {
            return Err(io::Error::last_os_error());
        }
        Ok(PathBuf::from(OsString::from_wide(&output)))
    }
    pub fn encode(path: &Path) -> io::Result<Vec<u8>> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.is_empty() {
            return Ok(Vec::new());
        }
        let length = i32::try_from(wide.len()).map_err(io::Error::other)?;
        // SAFETY: GetACP has no pointer arguments or ownership effects.
        let utf8 = unsafe { GetACP() } == 65001;
        let flags = if utf8 { 0 } else { 0x400 }; // WC_NO_BEST_FIT_CHARS
        let mut substituted = 0;
        let used = if utf8 {
            ptr::null_mut()
        } else {
            &mut substituted
        };
        // SAFETY: initialized UTF-16 input is retained, query output is NULL;
        // used is NULL for UTF-8 or exclusive initialized BOOL storage otherwise.
        let size = unsafe {
            WideCharToMultiByte(
                0,
                flags,
                wide.as_ptr(),
                length,
                ptr::null_mut(),
                0,
                ptr::null(),
                used,
            )
        };
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut bytes = vec![0; size as usize];
        // SAFETY: input/output spans are initialized and disjoint with checked
        // sizes; the default/used pointers obey the code-page restrictions.
        if unsafe {
            WideCharToMultiByte(
                0,
                flags,
                wide.as_ptr(),
                length,
                bytes.as_mut_ptr(),
                size,
                ptr::null(),
                used,
            )
        } != size
        {
            return Err(io::Error::last_os_error());
        }
        if substituted != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "voice identifier is not representable in the Windows code page",
            ));
        }
        Ok(bytes)
    }
}
impl Roster for Catalog {
    fn len(&self) -> usize {
        self.records.len()
    }
    fn voice(&self, index: usize) -> Option<Voice<'_>> {
        self.records.get(index).map(|record| record.view())
    }
    fn previous_score(&self, index: usize) -> i32 {
        self.records.get(index).map_or(0, |record| record.score)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_chunks_match_whole_file_across_fgets_and_nul_boundaries() {
        let mut bytes = b"language en\nname Native\0discarded\nvariants 12\n#".to_vec();
        bytes.extend_from_slice(&[b'a'; 500]);
        bytes.extend_from_slice(b"\nlanguage de 3\ngender female 30");
        let expected = Metadata::parse(&bytes).unwrap();
        for width in 1..=129 {
            let mut builder = MetadataChunks::new(true);
            let mut genders = 0;
            let address = builder.line.as_ptr();
            for chunk in bytes.chunks(width) {
                builder
                    .feed(chunk, |kind| {
                        assert_eq!(kind, Diagnostic::GenderOnLanguage);
                        genders += 1;
                    })
                    .unwrap();
                assert_eq!(builder.line.as_ptr(), address);
            }
            let actual = builder.finish(|_| genders += 1).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(genders, 1);
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_file_chunks_normalize_crlf_and_stop_at_text_eof() {
        let mut builder = MetadataChunks::new(false);
        for chunk in b"language en\r\nname Native\r\n\x1alanguage de\r\n".chunks(1) {
            builder.feed(chunk, |_| {}).unwrap();
        }
        assert_eq!(
            builder.finish(|_| {}).unwrap(),
            Metadata::parse(b"language en\nname Native\n").unwrap()
        );
    }
    #[test]
    fn file_discovery_skips_hidden_empty_and_unusable_metadata() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "espeak-owned-catalog-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("voices/sub")).unwrap();
        fs::create_dir(root.join("lang")).unwrap();
        fs::write(root.join("voices/.hidden"), b"language hidden").unwrap();
        fs::write(root.join("voices/empty"), b"").unwrap();
        fs::write(root.join("voices/unusable"), b"name missing language").unwrap();
        fs::write(root.join("voices/sub/custom"), b"language en\nname Owned").unwrap();
        let mut long = b"language en\nname Owned\n".to_vec();
        for _ in 0..1000 {
            long.extend_from_slice(b"# a deliberately long configuration file that must not require a large temporary buffer\n");
        }
        fs::write(root.join("voices/sub/custom"), long).unwrap();
        fs::write(root.join("lang/en"), b"language en\ngender female 30").unwrap();
        let mut messages = Vec::new();
        let mut catalog = Catalog::load(&root, |kind, _| messages.push(kind)).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(catalog.len(), 2);
        assert_eq!(messages, [Diagnostic::GenderOnLanguage]);
        catalog.sort();
        assert_eq!(catalog.voice(0).unwrap().name, b"Owned");
        let expected = Path::new("sub").join("custom");
        assert_eq!(
            catalog.voice(0).unwrap().identifier,
            expected.as_os_str().as_encoded_bytes()
        );
        assert_eq!(Catalog::load(&root, |_, _| {}).unwrap().len(), 0);
    }
    #[test]
    fn admitted_metadata_and_identifiers_have_stable_owned_storage() {
        let mut catalog = Catalog::new(3).unwrap();
        assert!(!catalog
            .insert(b"ignored", b"name no language", false, |_, _| {})
            .unwrap());
        assert!(catalog
            .insert(b"en", b"language en\nname English", false, |_, _| {})
            .unwrap());
        let address = catalog.voice(0).unwrap().languages.as_ptr();
        assert!(catalog
            .insert(b"other", b"language de\n", false, |_, _| {})
            .unwrap());
        assert_eq!(catalog.voice(0).unwrap().languages.as_ptr(), address);
        assert_eq!(catalog.voice(1).unwrap().name, b"other");
        catalog.set_score(1, 123).unwrap();
        assert_eq!(catalog.previous_score(1), 123);
        assert!(catalog
            .insert(b"bad\0identifier", b"language en", false, |_, _| {})
            .is_err());
        assert!(catalog.insert(b"bad",b"language overlong-overlong-overlong-overlong-overlong-overlong-overlong-overlong-overlong",false,|_,_|{}).is_err());
        assert_eq!(catalog.len(), 2);
        catalog.sort();
        assert_eq!(catalog.voice(0).unwrap().identifier, b"other");
    }
    #[test]
    fn capacity_and_language_diagnostics_do_not_overwrite_records() {
        let mut catalog = Catalog::new(1).unwrap();
        let mut messages = Vec::new();
        assert!(catalog
            .insert(b"en", b"gender female 30\nlanguage en", true, |kind, _| {
                messages.push(kind)
            })
            .unwrap());
        assert_eq!(messages, [Diagnostic::GenderOnLanguage]);
        assert!(!catalog
            .insert(b"de", b"language de", false, |kind, _| messages.push(kind))
            .unwrap());
        assert_eq!(messages, [Diagnostic::GenderOnLanguage, Diagnostic::Full]);
        assert_eq!(catalog.voice(0).unwrap().identifier, b"en");
    }
}
