//! Bounded phoneme-word appending and pronunciation attribute transforms.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme,
    word_stress::{Error, Table},
};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Counts {
    pub vowels: i32,
    pub stressed: i32,
}
pub struct Append<'a> {
    pub tail: &'a [u8],
    pub offset: usize,
    pub counts: Counts,
}
/// Admit the complete append and checked counter effects before publication.
/// A full destination is an ordinary no-op. The retained immutable tail includes
/// its NUL; the caller may use initialized slices or publish into raw storage.
pub fn plan_append<'a>(
    length: usize,
    addition: &'a [u8],
    capacity: usize,
    table: &Table<'_>,
    table_count: usize,
    mut counts: Counts,
) -> Result<Option<Append<'a>>, Error> {
    if table_count > 256 {
        return Err(Error::Table);
    }
    let end = addition
        .iter()
        .position(|c| *c == 0)
        .ok_or(Error::Terminator)?;
    if length
        .checked_add(end)
        .is_none_or(|total| total >= capacity)
    {
        return Ok(None);
    }
    let mut unstressed = false;
    for &code in &addition[..end] {
        if code as usize >= table_count {
            continue;
        }
        let Some(ph) = table[code as usize] else {
            continue;
        };
        if ph.kind == 1 {
            if ph.standard_length < 4 {
                unstressed = true;
            }
        } else if ph.kind == 2 {
            if ph.flags & (1 << 1) == 0 && !unstressed {
                counts.stressed = counts.stressed.checked_add(1).ok_or(Error::Capacity)?;
            }
            unstressed = false;
            counts.vowels = counts.vowels.checked_add(1).ok_or(Error::Capacity)?;
        }
    }
    Ok(Some(Append {
        tail: &addition[..end + 1],
        offset: length,
        counts,
    }))
}
/// Append into initialized instance-owned storage without allocating.
pub fn append(
    word: &mut [u8],
    addition: &[u8],
    table: &Table<'_>,
    table_count: usize,
    counts: &mut Counts,
) -> Result<bool, Error> {
    let length = word.iter().position(|c| *c == 0).ok_or(Error::Terminator)?;
    let Some(plan) = plan_append(length, addition, word.len(), table, table_count, *counts)? else {
        return Ok(false);
    };
    word[plan.offset..plan.offset + plan.tail.len()].copy_from_slice(plan.tail);
    *counts = plan.counts;
    Ok(true)
}
/// Apply the alternate pronunciation to the byte immediately after the first
/// primary stress only. Byte signedness is explicit for the compatibility host.
/// Missing names retain the C code-zero substitution, including its effect on
/// string termination. No table callback is needed.
pub fn special_attribute(
    word: &mut [u8],
    options: i32,
    flags: u32,
    table: &Table<'_>,
    table_count: usize,
    signed_bytes: bool,
) -> Result<(), Error> {
    if table_count > 256 {
        return Err(Error::Table);
    }
    let end = word.iter().position(|c| *c == 0).ok_or(Error::Terminator)?;
    if options & 2 == 0 {
        return Ok(());
    }
    let Some(primary) = word[..end.saturating_sub(1)].iter().position(|c| *c == 6) else {
        return Ok(());
    };
    let alternative = flags & 0x10000 != 0;
    let pairs = if alternative {
        [(b'E', b'e'), (b'O', b'o')]
    } else {
        [(b'e', b'E'), (b'o', b'O')]
    };
    let codes = pairs.map(|(old, new)| {
        (
            phoneme::code(table[..table_count].iter().copied(), u32::from(old)),
            phoneme::code(table[..table_count].iter().copied(), u32::from(new)),
        )
    });
    let byte = &mut word[primary + 1];
    if (!signed_bytes || *byte < 128) && *byte == codes[0].0 {
        *byte = codes[0].1;
    }
    if (!signed_bytes || *byte < 128) && *byte == codes[1].0 {
        *byte = codes[1].1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phoneme::Phoneme;
    #[test]
    fn append_tracks_vowels_skips_gaps_and_keeps_full_storage_unchanged() {
        let vowel = Phoneme {
            kind: 2,
            ..Default::default()
        };
        let weak = Phoneme {
            kind: 2,
            flags: 2,
            ..Default::default()
        };
        let mark = Phoneme {
            kind: 1,
            standard_length: 1,
            ..Default::default()
        };
        let mut table = [None; 256];
        table[40] = Some(&vowel);
        table[41] = Some(&weak);
        table[2] = Some(&mark);
        let mut word = [0; 12];
        let mut counts = Counts::default();
        assert!(append(&mut word, &[2, 99, 40, 40, 41, 0], &table, 256, &mut counts).unwrap());
        assert_eq!(
            counts,
            Counts {
                vowels: 3,
                stressed: 1
            }
        );
        let saved = word;
        assert!(!append(&mut word, &[40; 13], &table, 256, &mut counts).is_ok());
        assert!(!append(
            &mut word,
            &[40, 40, 40, 40, 40, 40, 40, 0],
            &table,
            256,
            &mut counts
        )
        .unwrap());
        assert_eq!(word, saved);
        counts.vowels = i32::MAX;
        assert_eq!(
            append(&mut word, &[40, 0], &table, 256, &mut counts),
            Err(Error::Capacity)
        );
        assert_eq!(word, saved);
    }
    #[test]
    fn attribute_changes_only_after_first_primary_and_keeps_tail() {
        let records = [
            Phoneme {
                mnemonic: b'e' as u32,
                code: 40,
                ..Default::default()
            },
            Phoneme {
                mnemonic: b'E' as u32,
                code: 41,
                ..Default::default()
            },
            Phoneme {
                mnemonic: b'o' as u32,
                code: 42,
                ..Default::default()
            },
            Phoneme {
                mnemonic: b'O' as u32,
                code: 43,
                ..Default::default()
            },
        ];
        let mut table = [None; 256];
        for record in &records {
            table[record.code as usize] = Some(record);
        }
        let mut word = [40, 6, 40, 42, 6, 40, 0, 99];
        special_attribute(&mut word, 2, 0, &table, 256, true).unwrap();
        assert_eq!(word, [40, 6, 41, 42, 6, 40, 0, 99]);
        special_attribute(&mut word, 2, 0x10000, &table, 256, true).unwrap();
        assert_eq!(word, [40, 6, 40, 42, 6, 40, 0, 99]);
        let high = [
            Phoneme {
                mnemonic: b'e' as u32,
                code: 140,
                ..Default::default()
            },
            Phoneme {
                mnemonic: b'E' as u32,
                code: 141,
                ..Default::default()
            },
        ];
        let mut table = [None; 256];
        table[140] = Some(&high[0]);
        table[141] = Some(&high[1]);
        let mut word = [6, 140, 0];
        special_attribute(&mut word, 2, 0, &table, 256, true).unwrap();
        assert_eq!(word, [6, 140, 0]);
        special_attribute(&mut word, 2, 0, &table, 256, false).unwrap();
        assert_eq!(word, [6, 141, 0]);
    }
}
