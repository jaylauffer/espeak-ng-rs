//! Compiled phoneme table validation and bounded inheritance overlays.
// Copyright (C) 2005-2015 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::phoneme::Phoneme;
pub const MAX_TABLES: usize = 150;
pub const PHONDATA_VERSION: u32 = 0x014801;
pub const MISSING: usize = usize::MAX;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InvalidPhonemeData(pub &'static str);
impl std::fmt::Display for InvalidPhonemeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for InvalidPhonemeData {}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TableMeta {
    pub name: [u8; 32],
    pub records_offset: usize,
    pub count: u32,
    pub includes: u32,
}

/// Parse once when resident bytes change. Selection uses this fixed-size index.
#[derive(Debug, Clone)]
pub struct TableIndex {
    tables: [TableMeta; MAX_TABLES],
    length: usize,
}

impl TableIndex {
    pub fn parse(bytes: &[u8]) -> Result<Self, InvalidPhonemeData> {
        let length = usize::from(
            *bytes
                .first()
                .ok_or(InvalidPhonemeData("truncated phontab header"))?,
        );
        if length == 0 || length > MAX_TABLES {
            return Err(InvalidPhonemeData("phontab table count must be 1..=150"));
        }
        let mut result = Self {
            tables: [TableMeta::default(); MAX_TABLES],
            length,
        };
        let mut cursor = 4;
        for table in &mut result.tables[..length] {
            let header = bytes
                .get(cursor..cursor + 36)
                .ok_or(InvalidPhonemeData("truncated phoneme-table header"))?;
            table.count = u32::from(header[0]);
            table.includes = u32::from(header[1]);
            table.name.copy_from_slice(&header[4..]);
            if !table.name.contains(&0) {
                return Err(InvalidPhonemeData(
                    "phoneme-table name lacks NUL terminator",
                ));
            }
            if table.includes as usize > length {
                return Err(InvalidPhonemeData(
                    "phoneme-table inheritance is outside phontab",
                ));
            }
            cursor += 36;
            table.records_offset = cursor;
            cursor += table.count as usize * 16;
            if cursor > bytes.len() {
                return Err(InvalidPhonemeData("truncated phoneme records"));
            }
        }
        if cursor != bytes.len() {
            return Err(InvalidPhonemeData("trailing bytes after phoneme tables"));
        }
        for number in 0..length {
            result.chain(number)?;
        }
        Ok(result)
    }
    pub fn tables(&self) -> &[TableMeta] {
        &self.tables[..self.length]
    }
    pub fn lookup(&self, name: &[u8]) -> Option<usize> {
        self.tables().iter().position(|table| {
            &table.name[..table
                .name
                .iter()
                .position(|b| *b == 0)
                .expect("validated name")]
                == name
        })
    }
    fn chain(&self, number: usize) -> Result<([usize; MAX_TABLES], usize), InvalidPhonemeData> {
        if number >= self.length {
            return Err(InvalidPhonemeData(
                "selected phoneme table is outside phontab",
            ));
        }
        let mut chain = [0; MAX_TABLES];
        let mut seen = [false; MAX_TABLES];
        let mut current = number;
        let mut length = 0;
        loop {
            if seen[current] {
                return Err(InvalidPhonemeData("cyclic phoneme-table inheritance"));
            }
            seen[current] = true;
            chain[length] = current;
            length += 1;
            match self.tables[current].includes {
                0 => return Ok((chain, length)),
                parent => current = parent as usize - 1,
            }
        }
    }
    /// Code-indexed record offsets. Clear all slots first, then overlay ancestors
    /// oldest first. This removes stale records when switching sibling tables.
    pub fn select(&self, bytes: &[u8], number: usize) -> Result<[usize; 256], InvalidPhonemeData> {
        let (chain, length) = self.chain(number)?;
        let mut selected = [MISSING; 256];
        for &number in chain[..length].iter().rev() {
            let table = self.tables[number];
            for record in 0..table.count as usize {
                let offset = table.records_offset + record * 16;
                let code = *bytes.get(offset + 10).ok_or(InvalidPhonemeData(
                    "resident phontab changed or is truncated",
                ))?;
                selected[usize::from(code)] = offset;
            }
        }
        Ok(selected)
    }
}

pub fn record(bytes: &[u8], offset: usize) -> Option<Phoneme> {
    bytes
        .get(offset..offset.checked_add(16)?)
        .map(|bytes| Phoneme::from_record(bytes.try_into().expect("sixteen bytes")))
}
/// Decode the fixed little-endian phondata header without unchecked C shifts.
pub fn header(phondata: &[u8]) -> Result<[u32; 2], InvalidPhonemeData> {
    let header = phondata
        .get(..8)
        .ok_or(InvalidPhonemeData("truncated phondata header"))?;
    let version = u32::from_le_bytes(header[..4].try_into().expect("four bytes"));
    let rate = u32::from_le_bytes(header[4..].try_into().expect("four bytes"));
    Ok([version, rate])
}
pub fn sample_rate(phondata: &[u8]) -> Result<u32, InvalidPhonemeData> {
    let [version, rate] = header(phondata)?;
    if version != PHONDATA_VERSION {
        return Err(InvalidPhonemeData("phondata version mismatch"));
    }
    if rate == 0 || rate > i32::MAX as u32 {
        return Err(InvalidPhonemeData("invalid phondata sample rate"));
    }
    Ok(rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(parents: &[u8]) -> Vec<u8> {
        let mut bytes = vec![parents.len() as u8, 0, 0, 0];
        for (index, &parent) in parents.iter().enumerate() {
            bytes.extend_from_slice(&[1, parent, 0, 0]);
            let mut name = [0; 32];
            name[0] = b'a' + index as u8;
            bytes.extend_from_slice(&name);
            let mut record = [0; 16];
            record[0] = b'a' + index as u8;
            record[10] = 10 + index as u8;
            bytes.extend_from_slice(&record);
        }
        bytes
    }
    #[test]
    fn overlays_parent_records_and_switches_without_stale_siblings() {
        let bytes = fixture(&[0, 1, 1]);
        let index = TableIndex::parse(&bytes).unwrap();
        assert_eq!(index.lookup(b"b"), Some(1));
        let a = index.select(&bytes, 1).unwrap();
        let b = index.select(&bytes, 2).unwrap();
        assert_ne!(a[10], MISSING);
        assert_ne!(a[11], MISSING);
        assert_eq!(a[12], MISSING);
        assert_ne!(b[10], MISSING);
        assert_eq!(b[11], MISSING);
        assert_ne!(b[12], MISSING);
    }
    #[test]
    fn rejects_cycles_bad_parents_and_truncation() {
        assert!(TableIndex::parse(&fixture(&[1])).is_err());
        assert!(TableIndex::parse(&fixture(&[2, 1])).is_err());
        assert!(TableIndex::parse(&fixture(&[3, 0])).is_err());
        let bytes = fixture(&[0, 1]);
        for len in 0..bytes.len() {
            assert!(TableIndex::parse(&bytes[..len]).is_err());
        }
    }
    #[test]
    fn checks_header_length_version_and_sample_rate() {
        let mut bytes = vec![1, 72, 1, 0, 34, 86, 0, 0];
        assert_eq!(sample_rate(&bytes).unwrap(), 22050);
        for length in 0..8 {
            assert!(header(&bytes[..length]).is_err());
        }
        bytes[4..].copy_from_slice(&0_u32.to_le_bytes());
        assert!(sample_rate(&bytes).is_err());
        bytes[4..].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(sample_rate(&bytes).is_err());
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(header(&bytes).unwrap()[0], u32::MAX);
        assert!(sample_rate(&bytes).is_err());
    }
}
