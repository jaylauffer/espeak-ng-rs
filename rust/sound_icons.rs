//! Owned configuration names and bounded, reusable sound-icon WAV storage.
// Copyright (C) 2005-2015 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    core_storage::{Buffer, MAX_BYTES},
    voice_reader::Reader,
};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
pub const MAX_ICONS: usize = 80;
#[derive(Default)]
struct Entry {
    name: i32,
    filename: Vec<u8>,
    data: Buffer,
    samples: i32,
}
pub struct Icon<'a> {
    pub name: i32,
    pub filename: &'a [u8],
    pub bytes: &'a [u8],
    pub samples: i32,
}
pub struct Catalog {
    entries: [Entry; MAX_ICONS],
    count: usize,
    limit: usize,
}
impl Catalog {
    pub fn new(limit: usize) -> io::Result<Self> {
        if limit == 0 || limit > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sound-icon byte limit must be 1..=134217728",
            ));
        }
        Ok(Self {
            entries: std::array::from_fn(|_| Entry::default()),
            count: 0,
            limit,
        })
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn reserved_bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.data.capacity()).sum()
    }
    pub fn icon(&self, index: usize) -> Option<Icon<'_>> {
        (index < self.count).then(|| {
            let entry = &self.entries[index];
            Icon {
                name: entry.name,
                filename: &entry.filename[..entry.filename.len() - 1],
                bytes: entry.data.bytes(),
                samples: entry.samples,
            }
        })
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn filename_c(&self, index: usize) -> &[u8] {
        &self.entries[index].filename
    }
    pub fn find_name(&self, name: i32) -> Option<usize> {
        (0..self.count).find(|index| self.entries[*index].name == name)
    }
    pub fn find_file(&self, filename: &[u8]) -> Option<usize> {
        (0..self.count).find(|index| self.icon(*index).unwrap().filename == filename)
    }
    fn filename(entry: &mut Entry, name: &[u8]) -> io::Result<()> {
        if name.is_empty() || name.len() > 4095 || name.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid sound-icon filename",
            ));
        }
        entry
            .filename
            .try_reserve_exact((name.len() + 1).saturating_sub(entry.filename.len()))
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        entry.filename.clear();
        entry.filename.extend_from_slice(name);
        entry.filename.push(0);
        Ok(())
    }
    pub fn define(&mut self, name: i32, filename: &[u8]) -> io::Result<usize> {
        if self.count == MAX_ICONS {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "sound-icon table is full",
            ));
        }
        let index = self.count;
        let entry = &mut self.entries[index];
        Self::filename(entry, filename)?;
        entry.name = name;
        entry.samples = 0;
        self.count += 1;
        Ok(index)
    }
    /// Match the legacy fixed WAV header: mono signed PCM16, source sample rate,
    /// and data starting at byte 44. The old disabled conversion path does not
    /// support other formats. Length validation prevents borrowed PCM overruns.
    fn samples(bytes: &[u8], rate: i32) -> io::Result<i32> {
        let header = bytes.get(..44).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "short sound-icon WAV header")
        })?;
        let word =
            |offset: usize| u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap());
        let rate = u32::try_from(rate)
            .ok()
            .filter(|rate| *rate > 0)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid sound-icon sample rate",
                )
            })?;
        if word(20) != 0x10001 || word(24) != rate || Some(word(28)) != rate.checked_mul(2) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "sound icon requires mono PCM16 at the active sample rate",
            ));
        }
        let length = word(40) as usize;
        if length > bytes.len() - 44 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sound-icon PCM length exceeds file",
            ));
        }
        i32::try_from(length / 2).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "sound-icon sample count exceeds bound",
            )
        })
    }
    fn admit(&self, index: usize, length: usize) -> io::Result<()> {
        let required = length
            .checked_add(7)
            .map(|size| size / 8 * 8)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        if index >= MAX_ICONS
            || self
                .reserved_bytes()
                .checked_add(required.saturating_sub(self.entries[index].data.capacity()))
                .is_none_or(|total| total > self.limit)
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "sound icons exceed reserved byte limit",
            ));
        }
        Ok(())
    }
    /// Read on initialization/the serialized synthesis worker. Warm nonempty
    /// icons stay cached just as in C, and retain their PCM address until drop.
    /// Zero-length icons may reread, reusing storage. No per-lookup allocation.
    pub fn load_file(&mut self, filename: &[u8], path: &Path, rate: i32) -> io::Result<usize> {
        if let Some(index) = self.find_file(filename) {
            return self.load_icon(index, path, rate);
        }
        let metadata = std::fs::metadata(path)?;
        if metadata.is_dir() {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        let length = usize::try_from(metadata.len())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        let mut file = File::open(path)?;
        self.read_file(filename, &mut file, length, rate)
    }
    /// Character lookup preserves the selected entry even when another
    /// character names the same file. Dynamic filename lookup selects the first.
    pub fn load_icon(&mut self, index: usize, path: &Path, rate: i32) -> io::Result<usize> {
        if index >= self.count {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if self.entries[index].samples != 0 {
            return Ok(index);
        }
        let metadata = std::fs::metadata(path)?;
        if metadata.is_dir() {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        let length = usize::try_from(metadata.len())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        let mut file = File::open(path)?;
        self.read_icon(index, &mut file, length, rate)?;
        Ok(index)
    }
    fn read_icon(
        &mut self,
        index: usize,
        source: &mut impl Read,
        length: usize,
        rate: i32,
    ) -> io::Result<()> {
        self.admit(index, length)?;
        let entry = &mut self.entries[index];
        entry.samples = 0;
        entry.data.read(source, length)?;
        entry.samples = Self::samples(entry.data.bytes(), rate)?;
        Ok(())
    }
    /// Install bytes already read by a caller-owned host proactor, after its
    /// completion on the owner/worker. Performs no filesystem operation.
    pub fn resident(&mut self, filename: &[u8], bytes: &[u8], rate: i32) -> io::Result<usize> {
        self.read_file(filename, &mut io::Cursor::new(bytes), bytes.len(), rate)
    }
    fn read_file(
        &mut self,
        filename: &[u8],
        source: &mut impl Read,
        length: usize,
        rate: i32,
    ) -> io::Result<usize> {
        let existing = self.find_file(filename);
        let index = existing.unwrap_or(self.count);
        if existing.is_some() && self.entries[index].samples != 0 {
            return Ok(index);
        }
        self.read_icon(index, source, length, rate)?;
        let entry = &mut self.entries[index];
        if existing.is_none() {
            Self::filename(entry, filename)?;
            entry.name = 0;
            self.count += 1;
        }
        Ok(index)
    }
}
/// Exact compatibility relative-path prefix and snprintf-width truncation.
/// Unix absolute names begin with '/'; other prefixes remain relative, as in C.
#[cfg(feature = "c-abi")]
pub(crate) fn path(
    root: &[u8],
    filename: &[u8],
    separator: u8,
    width: usize,
) -> io::Result<std::path::PathBuf> {
    if !(2..=4096).contains(&width)
        || root.contains(&0)
        || filename.contains(&0)
        || filename.is_empty()
        || root.len() > 4095
        || filename.len() > 4095
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut buffer = [0; 4096];
    let mut used = 0;
    if filename[0] == b'/' {
        return crate::voice_storage::compat_path(filename);
    }
    for byte in root
        .iter()
        .copied()
        .chain([separator])
        .chain(b"soundicons".iter().copied())
        .chain([separator])
        .chain(filename.iter().copied())
        .take(width - 1)
    {
        buffer[used] = byte;
        used += 1;
    }
    crate::voice_storage::compat_path(&buffer[..used])
}
/// Configuration retains raw first-column prefix behavior, including toneXYZ
/// and soundiconXYZ prefixes. Valid completed chunks commit in file order.
/// Bounds replace C's overflowing filename/table writes; malformed directives
/// are ignored and tone arithmetic rejection preserves earlier points.
pub fn configure<R: Read>(
    reader: &mut Reader<R>,
    points: &mut [i32; 12],
    icons: &mut Catalog,
    signed_character: bool,
) -> io::Result<()> {
    while let Some(chunk) = reader.next_chunk()? {
        let end = chunk
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(chunk.len());
        let line = &chunk[..end];
        if line.first() == Some(&b'/') {
            continue;
        }
        if line.starts_with(b"tone") {
            if let Ok(next) = crate::voice::tone_points(line.get(5..).unwrap_or_default()) {
                *points = next;
            }
        } else if line.starts_with(b"soundicon") {
            let Some(value) = line.get(10..) else {
                continue;
            };
            if value.first() != Some(&b'_') || value.len() < 2 {
                continue;
            }
            let byte = value[1];
            let mut name = &value[2..];
            while name.first().is_some_and(u8::is_ascii_whitespace) {
                name = &name[1..];
            }
            let end = name
                .iter()
                .position(u8::is_ascii_whitespace)
                .unwrap_or(name.len());
            if end == 0 || end > 199 {
                continue;
            }
            let character = if signed_character {
                i32::from(byte as i8)
            } else {
                i32::from(byte)
            };
            icons.define(character, &name[..end])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave(length: usize) -> Vec<u8> {
        let mut bytes = vec![0; 44 + length];
        bytes[20..24].copy_from_slice(&0x10001_u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&22050_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&44100_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&(length as u32).to_le_bytes());
        bytes
    }
    #[test]
    fn named_and_dynamic_icons_share_stable_pcm_and_bound_storage() {
        let mut catalog = Catalog::new(112).unwrap();
        let index = catalog.define(33, b"one").unwrap();
        assert_eq!(catalog.resident(b"one", &wave(10), 22050).unwrap(), index);
        let address = catalog.icon(index).unwrap().bytes.as_ptr();
        for _ in 0..20 {
            assert_eq!(catalog.resident(b"one", b"unread", 22050).unwrap(), index);
        }
        assert_eq!(catalog.icon(index).unwrap().bytes.as_ptr(), address);
        assert_eq!(catalog.icon(index).unwrap().samples, 5);
        assert_eq!((address as usize + 44) % 2, 0);
        assert_eq!(catalog.resident(b"two", &wave(10), 22050).unwrap(), 1);
        assert_eq!(catalog.reserved_bytes(), 112);
        assert_eq!(
            catalog
                .resident(b"three", &wave(10), 22050)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(catalog.len(), 2);
    }
    #[test]
    fn empty_pcm_reuses_buffer_and_malformed_or_unsupported_waves_fail() {
        let mut catalog = Catalog::new(128).unwrap();
        let empty = wave(0);
        let index = catalog.resident(b"zero", &empty, 22050).unwrap();
        let address = catalog.icon(index).unwrap().bytes.as_ptr();
        assert_eq!(catalog.resident(b"zero", &empty, 22050).unwrap(), index);
        assert_eq!(catalog.icon(index).unwrap().bytes.as_ptr(), address);
        assert_eq!(
            catalog
                .resident(b"bad", b"short", 22050)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        let mut bad = wave(4);
        bad[40..44].copy_from_slice(&999_u32.to_le_bytes());
        assert_eq!(
            catalog.resident(b"bad", &bad, 22050).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            catalog
                .resident(b"bad", &wave(0), 16000)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(catalog.len(), 1);
    }
    #[test]
    fn configuration_uses_legacy_prefixes_and_fgets_bounds_and_caps_entries() {
        let text=b"/tone 9\ntoneX100 120 200 130\n soundicon _? ignored\nsoundicon _! first\nsoundiconX_?second extra\nsoundicon _\xff high\n";
        let mut reader = Reader::new(
            io::Cursor::new(text),
            260,
            crate::voice_reader::TextMode::Windows,
        )
        .unwrap();
        let mut points = [0; 12];
        let mut catalog = Catalog::new(MAX_BYTES).unwrap();
        configure(&mut reader, &mut points, &mut catalog, true).unwrap();
        assert_eq!(points[..4], [100, 120, 200, 130]);
        assert_eq!(points[4..], [-1; 8]);
        assert_eq!(catalog.len(), 3);
        assert_eq!(catalog.icon(0).unwrap().filename, b"first");
        assert_eq!(catalog.icon(1).unwrap().filename, b"second");
        assert_eq!(catalog.icon(2).unwrap().name, -1);
        for index in 3..MAX_ICONS {
            catalog.define(index as i32, b"name").unwrap();
        }
        assert_eq!(
            catalog.define(90, b"over").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(catalog.len(), MAX_ICONS);
    }
}
