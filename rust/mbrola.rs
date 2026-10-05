//! Owned, bounded MBROLA mappings and contextual phoneme-name selection.
// Copyright (C) 2005-2013 Jonathan Duddington, 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme::Phoneme;
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Mapping {
    pub name: i32,
    pub next: u32,
    pub first: i32,
    pub second: i32,
    pub percent: i32,
    pub control: i32,
}
impl Mapping {
    fn decode(bytes: &[u8; 24]) -> Self {
        let words: [u32; 6] = std::array::from_fn(|i| {
            u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes"))
        });
        Self {
            name: words[0] as i32,
            next: words[1],
            first: words[2] as i32,
            second: words[3] as i32,
            percent: words[4] as i32,
            control: words[5] as i32,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Context {
    pub word_start: u32,
    pub next_word_start: u32,
    pub synth_flags: u32,
    pub stress: u32,
    pub word_stress: u32,
    pub prefix: i32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Selection {
    pub name: i32,
    pub second: i32,
    pub percent: i32,
    pub control: i32,
    pub prefix: i32,
}
/// First matching mapping wins. Prefix state is an explicit ordered effect;
/// absent previous/next records cannot match contextual rows. No callbacks,
/// allocations, I/O or process operations occur during name selection.
pub fn select(
    mappings: &[Mapping],
    current: &Phoneme,
    previous: Option<&Phoneme>,
    next: Option<&Phoneme>,
    pause: Option<&Phoneme>,
    context: &Context,
) -> Selection {
    let mut result = Selection {
        name: current.mnemonic as i32,
        ..Selection::default()
    };
    for mapping in mappings.iter().take_while(|mapping| mapping.name != 0) {
        if current.mnemonic != mapping.name as u32 {
            continue;
        }
        let mut found =
            mapping.next == 0 || (mapping.next == b':' as u32 && context.synth_flags & 8 != 0);
        if !found {
            let other = if mapping.control & 2 != 0 {
                previous
            } else if mapping.control & 8 != 0 && context.next_word_start != 0 {
                pause
            } else {
                next
            };
            if let Some(other) = other {
                found = mapping.next == other.mnemonic
                    || (mapping.next == 2 && other.kind == 2)
                    || (mapping.next == b'_' as u32 && other.kind == 0);
            }
        }
        if mapping.control & 4 != 0 && context.word_start == 0 {
            found = false;
        }
        if mapping.control & 0x40 != 0 && context.next_word_start == 0 {
            found = false;
        }
        if mapping.control & 0x20 != 0 && context.stress < context.word_stress {
            found = false;
        }
        if found {
            result.second = mapping.second;
            result.percent = mapping.percent;
            result.control = mapping.control;
            if mapping.control & 0x10 != 0 {
                result.name = 0;
                result.prefix = mapping.first;
                return result;
            }
            result.name = mapping.first;
            break;
        }
    }
    if context.prefix != 0 {
        result.name =
            ((result.name as u32).wrapping_shl(8) | (context.prefix as u32 & 0xff)) as i32;
    }
    result
}

/// Two reusable mapping buffers allow transactional file/host-resident loads.
/// At most `limit` reserved mapping bytes across both buffers. Published borrows
/// expire on successful replacement; failed reads preserve the active table.
pub struct Table {
    active: Vec<Mapping>,
    scratch: Vec<Mapping>,
    control: u32,
    limit: usize,
}
impl Table {
    pub fn new(limit: usize) -> io::Result<Self> {
        if limit == 0 || limit > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MBROLA mapping limit must be 1..=134217728",
            ));
        }
        Ok(Self {
            active: Vec::new(),
            scratch: Vec::new(),
            control: 0,
            limit,
        })
    }
    pub fn mappings(&self) -> &[Mapping] {
        &self.active
    }
    pub fn control(&self) -> u32 {
        self.control
    }
    pub fn reserved_bytes(&self) -> usize {
        (self.active.capacity() + self.scratch.capacity()) * 24
    }
    pub fn read(&mut self, reader: &mut impl Read, length: usize) -> io::Result<()> {
        if length < 28 || (length - 4) % 24 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid MBROLA mapping length",
            ));
        }
        let rows = (length - 4) / 24;
        let peak = self
            .active
            .capacity()
            .checked_add(self.scratch.capacity().max(rows))
            .and_then(|rows| rows.checked_mul(24));
        if peak.is_none_or(|bytes| bytes > self.limit) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "MBROLA mapping capacity exhausted",
            ));
        }
        self.scratch.clear();
        self.scratch.try_reserve_exact(rows).map_err(|_| {
            io::Error::new(io::ErrorKind::OutOfMemory, "cannot reserve MBROLA mappings")
        })?;
        let mut header = [0; 4];
        reader.read_exact(&mut header)?;
        let mut terminated = false;
        let mut chunk = [0; 24 * 128];
        let mut remaining = rows;
        while remaining > 0 {
            let count = remaining.min(128);
            reader.read_exact(&mut chunk[..count * 24])?;
            for record in chunk[..count * 24].chunks_exact(24) {
                let mapping = Mapping::decode(record.try_into().expect("complete record"));
                terminated |= mapping.name == 0;
                self.scratch.push(mapping);
            }
            remaining -= count;
        }
        if !terminated {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unterminated MBROLA mappings",
            ));
        }
        std::mem::swap(&mut self.active, &mut self.scratch);
        self.control = u32::from_le_bytes(header);
        Ok(())
    }
    /// Install already-resident proactor bytes on the owner, without filesystem I/O.
    pub fn replace(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.read(&mut io::Cursor::new(bytes), bytes.len())
    }
    /// Synchronous initialization/worker work; never invoke from a completion.
    pub fn load(&mut self, path: &Path) -> io::Result<()> {
        let mut file = File::open(path)?;
        let length = usize::try_from(file.metadata()?.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "MBROLA table too large"))?;
        self.read(&mut file, length)
    }
}
impl Default for Table {
    fn default() -> Self {
        Self::new(MAX_BYTES).expect("valid fixed limit")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(mappings: &[Mapping], control: u32) -> Vec<u8> {
        let mut bytes = control.to_le_bytes().to_vec();
        for row in mappings {
            for word in [
                row.name as u32,
                row.next,
                row.first as u32,
                row.second as u32,
                row.percent as u32,
                row.control as u32,
            ] {
                bytes.extend(word.to_le_bytes());
            }
        }
        bytes
    }
    #[test]
    fn resident_tables_reuse_bounded_buffers_and_preserve_active_on_failure() {
        let row = Mapping {
            name: b'a' as i32,
            first: b'b' as i32,
            percent: 60,
            ..Default::default()
        };
        let bytes = bytes(&[row, Mapping::default()], 0xa5);
        let mut table = Table::new(96).unwrap();
        table.replace(&bytes).unwrap();
        table.replace(&bytes).unwrap();
        let reserved = table.reserved_bytes();
        let addresses = [table.active.as_ptr(), table.scratch.as_ptr()];
        for _ in 0..100 {
            table.replace(&bytes).unwrap();
            assert!(addresses.contains(&table.active.as_ptr()));
            assert_eq!(table.reserved_bytes(), reserved);
        }
        assert_eq!(table.control(), 0xa5);
        assert_eq!(table.mappings(), &[row, Mapping::default()]);
        let address = table.active.as_ptr();
        assert_eq!(
            table.replace(&bytes[..bytes.len() - 1]).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(table
            .read(&mut io::Cursor::new(&bytes[..20]), bytes.len())
            .is_err());
        assert_eq!(table.active.as_ptr(), address);
        let bad = bytes[..28].to_vec();
        assert!(table.replace(&bad).is_err());
        assert_eq!(table.active.as_ptr(), address);
        let bigger = super::tests::bytes(&[row, row, Mapping::default()], 0);
        assert_eq!(
            table.replace(&bigger).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn contextual_rows_and_prefix_effects_are_ordered() {
        let current = Phoneme {
            mnemonic: b'a' as u32,
            ..Default::default()
        };
        let next = Phoneme {
            mnemonic: b'e' as u32,
            kind: 2,
            ..Default::default()
        };
        let pause = Phoneme::default();
        let rows = [
            Mapping {
                name: b'a' as i32,
                next: 2,
                first: b'A' as i32,
                control: 4 | 0x20,
                ..Default::default()
            },
            Mapping {
                name: b'a' as i32,
                first: b'?' as i32,
                control: 0x10,
                ..Default::default()
            },
            Mapping::default(),
        ];
        let context = Context {
            word_start: 1,
            stress: 4,
            word_stress: 4,
            ..Default::default()
        };
        assert_eq!(
            select(&rows, &current, None, Some(&next), Some(&pause), &context).name,
            b'A' as i32
        );
        let result = select(&rows, &current, None, None, None, &Context::default());
        assert_eq!(result.prefix, b'?' as i32);
        assert_eq!(result.name, 0);
        let context = Context {
            prefix: result.prefix,
            ..Default::default()
        };
        assert_eq!(
            select(&[], &current, None, None, None, &context).name,
            ((b'a' as i32) << 8) | b'?' as i32
        );
        let context = Context {
            prefix: i32::MIN,
            ..Default::default()
        };
        assert_eq!(
            select(
                &[],
                &Phoneme {
                    mnemonic: u32::MAX,
                    ..Default::default()
                },
                None,
                None,
                None,
                &context
            )
            .name,
            -256
        );
    }
}
