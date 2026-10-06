//! Reusable byte storage for marker, URI and compatibility wide names.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust migration (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Limit,
    Format,
    Capacity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub offset: usize,
    pub length: usize,
    pub width: usize,
}
/// Opaque wide bytes retain original byte offsets, even after narrow entries.
/// Consumers must not cast possibly unaligned entries to wide references.
/// Raw views expire on growing append/reset/destruction; serialize consumers.
pub struct Names {
    words: Vec<u64>,
    used: usize,
    limit: usize,
}
impl Names {
    /// The reservation budget rounds down to whole eight-byte storage words.
    pub fn new(limit: usize) -> Result<Self, Error> {
        if !(8..=MAX_BYTES).contains(&limit) {
            return Err(Error::Limit);
        }
        Ok(Self {
            words: Vec::new(),
            used: 0,
            limit: limit / 8 * 8,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: initialized u64 storage, used bounded by initialized words.
        unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast(), self.used) }
    }
    pub fn reserved_bytes(&self) -> usize {
        self.words.capacity() * 8
    }
    /// Clear logical entries, retaining warmed storage for the next utterance.
    /// Existing records/views expire; only destruction releases the allocation.
    pub fn reset(&mut self) {
        self.used = 0;
    }
    pub fn get(&self, record: Record) -> Option<&[u8]> {
        self.bytes()
            .get(record.offset..record.offset.checked_add(record.length)?)
    }
    /// Input includes exactly one final zero unit, width1/2/4, without preceding
    /// zero units. Byte order and all nonzero code-unit bit patterns are opaque.
    /// Admission/allocation failure preserves old entries and logical offsets.
    pub fn append(&mut self, bytes: &[u8], width: usize) -> Result<Record, Error> {
        if !matches!(width, 1 | 2 | 4)
            || bytes.is_empty()
            || bytes.len() % width != 0
            || bytes[bytes.len() - width..].iter().any(|b| *b != 0)
            || bytes[..bytes.len() - width]
                .chunks_exact(width)
                .any(|unit| unit.iter().all(|b| *b == 0))
        {
            return Err(Error::Format);
        }
        let end = self
            .used
            .checked_add(bytes.len())
            .filter(|end| *end <= self.limit)
            .ok_or(Error::Capacity)?;
        let required = end.div_ceil(8);
        if required > self.words.capacity() {
            let capacity = (self.words.capacity().saturating_mul(2))
                .max(required)
                .max(128)
                .min(self.limit / 8);
            self.words
                .try_reserve_exact(capacity.saturating_sub(self.words.len()))
                .map_err(|_| Error::Capacity)?;
        }
        if required > self.words.len() {
            self.words.resize(required, 0);
        }
        // SAFETY: initialized exclusive word storage; all bit patterns valid,
        // source cannot alias through safe caller borrows; end admitted above.
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.words.as_mut_ptr().cast::<u8>().add(self.used),
                bytes.len(),
            );
        }
        let record = Record {
            offset: self.used,
            length: bytes.len(),
            width,
        };
        self.used = end;
        Ok(record)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_width_names_preserve_bytes_offsets_and_warm_storage() {
        let mut names = Names::new(1024).unwrap();
        let first = names.append(b"ab\0", 1).unwrap();
        let second = names.append(&[b'x', 0, 0, 0], 2).unwrap();
        assert_eq!(second.offset, 3);
        assert_eq!(names.get(first), Some(b"ab\0".as_slice()));
        assert_eq!(names.get(second), Some([b'x', 0, 0, 0].as_slice()));
        assert_eq!(names.bytes().as_ptr() as usize % 8, 0);
        let address = names.bytes().as_ptr();
        let capacity = names.reserved_bytes();
        for _ in 0..100 {
            names.reset();
            assert!(names.bytes().is_empty());
            names.append(b"repeat\0", 1).unwrap();
            assert_eq!(address, names.bytes().as_ptr());
            assert_eq!(capacity, names.reserved_bytes());
        }
    }
    #[test]
    fn admission_preserves_entries_for_malformed_or_exhausted_names() {
        let mut names = Names::new(32).unwrap();
        names.append(b"marker\0", 1).unwrap();
        let bytes = names.bytes().to_vec();
        let address = names.bytes().as_ptr();
        for (value, width) in [
            (b"bad".as_slice(), 1),
            (b"x\0y\0", 1),
            (b"x\0", 3),
            (&[0, 0, 1, 0, 0, 0], 2),
            (&[b'a'; 40], 1),
        ] {
            assert!(names.append(value, width).is_err());
            assert_eq!(names.bytes(), bytes);
            assert_eq!(names.bytes().as_ptr(), address);
        }
        let mut large = [b'a'; 32];
        large[31] = 0;
        assert_eq!(names.append(&large, 1), Err(Error::Capacity));
        assert_eq!(names.bytes(), bytes);
        assert!(Names::new(7).is_err());
        assert!(Names::new(MAX_BYTES + 1).is_err());
    }
}
