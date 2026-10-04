//! Bounds-checked indexing of the compiled dictionary rule stream.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::dictionary::InvalidDictionary;
pub const MISSING: usize = usize::MAX;

/// Offsets are relative to the complete dictionary, never raw pointers.
/// Fixed bounds match the legacy Translator arrays, including its 255 sentinel.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct RuleIndex {
    pub singles: [usize; 256],
    pub offsets: [usize; 128],
    pub pairs: [usize; 120],
    pub pair_names: [u32; 120],
    pub pair_count: usize,
    pub pair_starts: [u8; 256],
    pub pair_counts: [u8; 256],
    pub letters: [usize; 95],
    pub replacements: usize,
}

impl Default for RuleIndex {
    fn default() -> Self {
        Self {
            singles: [MISSING; 256],
            offsets: [MISSING; 128],
            pairs: [MISSING; 120],
            pair_names: [0; 120],
            pair_count: 0,
            pair_starts: [255; 256],
            pair_counts: [0; 256],
            letters: [MISSING; 95],
            replacements: MISSING,
        }
    }
}

fn nul_end(bytes: &[u8], start: usize) -> Result<usize, InvalidDictionary> {
    bytes
        .get(start..)
        .and_then(|tail| tail.iter().position(|b| *b == 0))
        .map(|length| start + length)
        .ok_or(InvalidDictionary("unterminated rule record"))
}

impl RuleIndex {
    pub fn parse(bytes: &[u8], start: usize) -> Result<Self, InvalidDictionary> {
        let mut result = Self::default();
        let mut cursor = start;
        if bytes.get(cursor) == Some(&7) {
            return Ok(result);
        } // dictionary without rules
        loop {
            match bytes.get(cursor) {
                Some(0) => return Ok(result),
                Some(6) => cursor += 1,
                _ => {
                    return Err(InvalidDictionary(
                        "missing rule-group start or stream terminator",
                    ))
                }
            }
            match bytes.get(cursor) {
                Some(20) => {
                    cursor = cursor
                        .checked_add(4)
                        .ok_or(InvalidDictionary("replacement alignment overflow"))?
                        & !3;
                    result.replacements = cursor;
                    while bytes.get(cursor..cursor + 4) != Some(&[0, 0, 0, 0]) {
                        if cursor >= bytes.len() {
                            return Err(InvalidDictionary("unterminated character replacements"));
                        }
                        cursor += 1;
                    }
                    while bytes.get(cursor) != Some(&7) {
                        if cursor >= bytes.len() {
                            return Err(InvalidDictionary("missing replacement-group end"));
                        }
                        cursor += 1;
                    }
                    cursor += 1;
                    continue;
                }
                Some(18) => {
                    let group = bytes
                        .get(cursor + 1)
                        .ok_or(InvalidDictionary("truncated letter-group name"))?
                        .wrapping_sub(b'A') as usize;
                    if group >= result.letters.len() {
                        return Err(InvalidDictionary("letter-group index exceeds 94"));
                    }
                    cursor += 2;
                    result.letters[group] = cursor;
                }
                Some(_) => {
                    let end = nul_end(bytes, cursor)?;
                    let name = &bytes[cursor..end];
                    cursor = end + 1;
                    match name {
                        [] => result.singles[0] = cursor,
                        [first] => result.singles[usize::from(*first)] = cursor,
                        [1, offset, ..] => {
                            let index = usize::from(*offset)
                                .checked_sub(1)
                                .filter(|i| *i < result.offsets.len())
                                .ok_or(InvalidDictionary("offset group exceeds 127"))?;
                            result.offsets[index] = cursor;
                        }
                        [first, second, ..] => {
                            let index = result.pair_count;
                            if index >= result.pairs.len() {
                                return Err(InvalidDictionary("too many two-letter rule groups"));
                            }
                            let first = usize::from(*first);
                            if result.pair_starts[first] == 255 {
                                result.pair_starts[first] = index as u8;
                            }
                            result.pair_counts[first] += 1;
                            result.pairs[index] = cursor;
                            result.pair_names[index] = first as u32 | (u32::from(*second) << 8);
                            result.pair_count += 1;
                        }
                    }
                }
                None => return Err(InvalidDictionary("truncated rule-group header")),
            }
            loop {
                if bytes.get(cursor) == Some(&7) {
                    cursor += 1;
                    break;
                }
                cursor = nul_end(bytes, cursor)? + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexes_single_pair_offset_letter_and_aligned_replacement_groups() {
        let mut bytes = vec![0; 9];
        bytes.extend_from_slice(&[
            6, b'a', 0, 3, 42, 0, 7, 6, b'a', b'b', 0, 7, 6, 1, 128, 0, 7,
        ]);
        bytes.extend_from_slice(&[6, 18, b'A', b'x', 0, 7, 6, 20]);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        let replacements = bytes.len();
        bytes.extend_from_slice(&[65, 0, 0, 0, 66, 0, 0, 0, 0, 0, 0, 0, 7, 0]);
        let index = RuleIndex::parse(&bytes, 9).unwrap();
        assert_eq!(index.singles[b'a' as usize], 12);
        assert_eq!(index.pair_names[0], 0x6261);
        assert_ne!(index.offsets[127], MISSING);
        assert_ne!(index.letters[0], MISSING);
        assert_eq!(index.replacements, replacements);
        assert_eq!(index.pair_starts[b'a' as usize], 0);
        assert_eq!(index.pair_counts[b'a' as usize], 1);
    }
    #[test]
    fn rejects_malformed_streams_and_accepts_no_rules() {
        for bytes in [
            &[][..],
            &[6][..],
            &[6, b'a'][..],
            &[6, 18, 0][..],
            &[6, 20][..],
            &[6, 1, 129, 0, 7, 0][..],
        ] {
            assert!(RuleIndex::parse(bytes, 0).is_err());
        }
        assert!(RuleIndex::parse(&[7, 0], 0).is_ok());
    }
}
