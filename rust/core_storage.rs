//! Reusable aligned storage for the four core phoneme assets.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Slot {
    Phontab,
    Phonindex,
    Phondata,
    Intonations,
}
impl TryFrom<u32> for Slot {
    type Error = io::Error;
    fn try_from(value: u32) -> io::Result<Self> {
        match value {
            0 => Ok(Self::Phontab),
            1 => Ok(Self::Phonindex),
            2 => Ok(Self::Phondata),
            3 => Ok(Self::Intonations),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid phoneme asset slot",
            )),
        }
    }
}
#[derive(Default, Debug)]
pub(crate) struct Buffer {
    words: Vec<u64>,
    length: usize,
}
impl Buffer {
    pub(crate) fn capacity(&self) -> usize {
        self.words.capacity() * 8
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        // SAFETY: words are initialized u64 storage; all bit patterns are valid,
        // and the byte span never exceeds its allocation or initialized length.
        unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast(), self.length) }
    }
    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: exclusive initialized word storage, with a bounded byte span.
        // Writes preserve validity of every u64; padding bytes are inaccessible.
        unsafe { std::slice::from_raw_parts_mut(self.words.as_mut_ptr().cast(), self.length) }
    }
    pub(crate) fn read(&mut self, source: &mut impl Read, length: usize) -> io::Result<()> {
        let words = length
            .checked_add(7)
            .map(|size| size / 8)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "asset size overflow"))?;
        self.length = 0;
        self.words
            .try_reserve_exact(words.saturating_sub(self.words.len()))
            .map_err(|_| {
                io::Error::new(io::ErrorKind::OutOfMemory, "cannot reserve speech asset")
            })?;
        self.words.resize(words, 0);
        self.length = length;
        if let Err(error) = source.read_exact(self.bytes_mut()) {
            self.length = 0;
            self.words.clear();
            return Err(error);
        }
        Ok(())
    }
}
/// At most `limit` bytes of reserved storage across all four slots, including
/// retained capacity from earlier loads. Replacing a warmed slot reuses its
/// allocation. Views expire on replacement/destruction; serialize all consumers.
pub struct Storage {
    buffers: [Buffer; 4],
    limit: usize,
}
impl Storage {
    pub fn new(limit: usize) -> io::Result<Self> {
        if limit == 0 || limit > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "core asset limit must be 1..=134217728",
            ));
        }
        Ok(Self {
            buffers: std::array::from_fn(|_| Buffer::default()),
            limit,
        })
    }
    pub fn bytes(&self, slot: Slot) -> &[u8] {
        self.buffers[slot as usize].bytes()
    }
    pub fn reserved_bytes(&self) -> usize {
        self.buffers
            .iter()
            .map(|buffer| buffer.words.capacity() * 8)
            .sum()
    }
    fn admit(&self, slot: Slot, length: usize) -> io::Result<usize> {
        let words = length.checked_add(7).map(|size| size / 8).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "core asset size exceeds address space",
            )
        })?;
        let existing = self.buffers[slot as usize].words.capacity();
        let needed = self.reserved_bytes().checked_add(
            words
                .saturating_sub(existing)
                .checked_mul(8)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "core asset size overflow")
                })?,
        );
        if !needed.is_some_and(|bytes| bytes <= self.limit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "core assets exceed reserved byte limit",
            ));
        }
        Ok(words)
    }
    /// Read on initialization/a caller worker, never during host completion
    /// delivery. Admission happens before mutation. Once reading starts, failed
    /// input clears this slot while retaining its allocation, matching the legacy
    /// loader's invalidation behavior. Other slots remain live.
    pub fn read(&mut self, slot: Slot, source: &mut impl Read, length: usize) -> io::Result<()> {
        self.admit(slot, length)?;
        self.buffers[slot as usize].read(source, length)
    }
    /// Copy already-resident proactor bytes during initialization/worker work.
    /// Native consumers can retain their existing resident owner directly;
    /// compatibility consumers need these aligned views while C remains active.
    pub fn replace(&mut self, slot: Slot, bytes: &[u8]) -> io::Result<()> {
        self.read(slot, &mut io::Cursor::new(bytes), bytes.len())
    }
    pub fn load(&mut self, slot: Slot, path: &Path) -> io::Result<()> {
        let metadata = std::fs::metadata(path)?;
        if metadata.is_dir() {
            return Err(io::Error::from(io::ErrorKind::IsADirectory));
        }
        let length = usize::try_from(metadata.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "core asset exceeds address space",
            )
        })?;
        let mut file = File::open(path)?;
        self.read(slot, &mut file, length)
    }
}
impl Default for Storage {
    fn default() -> Self {
        Self {
            buffers: std::array::from_fn(|_| Buffer::default()),
            limit: MAX_BYTES,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aligned_buffers_reuse_capacity_and_account_for_retained_peak_storage() {
        let mut storage = Storage::new(40).unwrap();
        storage.replace(Slot::Phondata, b"123456789").unwrap();
        let address = storage.bytes(Slot::Phondata).as_ptr();
        assert_eq!(address as usize % std::mem::align_of::<u64>(), 0);
        storage.replace(Slot::Phondata, b"short").unwrap();
        assert_eq!(address, storage.bytes(Slot::Phondata).as_ptr());
        storage.replace(Slot::Phontab, b"123456789abcdefg").unwrap();
        assert_eq!(storage.reserved_bytes(), 32);
        storage.replace(Slot::Intonations, b"12345678").unwrap();
        assert!(storage.replace(Slot::Phonindex, b"x").is_err());
        assert!(storage.bytes(Slot::Phonindex).is_empty());
        storage.replace(Slot::Phondata, b"").unwrap();
        assert_eq!(storage.reserved_bytes(), 40);
        storage.replace(Slot::Phondata, b"123456789").unwrap();
        assert_eq!(address, storage.bytes(Slot::Phondata).as_ptr());
        assert_eq!(storage.bytes(Slot::Phondata), b"123456789");
    }
    #[test]
    fn failed_admission_preserves_views_but_short_reads_clear_only_the_read_slot() {
        let mut storage = Storage::new(16).unwrap();
        storage.replace(Slot::Phontab, b"table").unwrap();
        storage.replace(Slot::Phondata, b"old").unwrap();
        assert!(storage
            .read(Slot::Phondata, &mut io::Cursor::new(b"new"), usize::MAX)
            .is_err());
        assert_eq!(storage.bytes(Slot::Phondata), b"old");
        assert_eq!(
            storage
                .read(Slot::Phondata, &mut io::Cursor::new(b"short"), 8)
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert!(storage.bytes(Slot::Phondata).is_empty());
        assert_eq!(storage.bytes(Slot::Phontab), b"table");
        assert_eq!(storage.reserved_bytes(), 16);
        assert!(Storage::new(0).is_err());
        assert!(Storage::new(MAX_BYTES + 1).is_err());
        assert!(Slot::try_from(4).is_err());
    }
}
