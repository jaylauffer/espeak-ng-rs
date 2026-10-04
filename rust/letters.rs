//! Language-specific letter predicates used by pronunciation and stress rules.
//!
//! Borrow configuration prepared during language setup. Matching allocates
//! nothing and preserves the fork's mask-valued results and accent mapping.
// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "letter_data.rs"]
mod data;

/// A C wide-character list, excluding its terminating NUL. These are code
/// units, not decoded UTF-16: the original `wcschr` compares `wchar_t` units.
#[derive(Clone, Copy, Debug)]
pub enum WideLetters<'a> {
    U16(&'a [u16]),
    U32(&'a [u32]),
}
impl WideLetters<'_> {
    fn contains(self, letter: i32) -> bool {
        match self {
            Self::U16(units) => letter as u16 == 0 || units.contains(&(letter as u16)),
            Self::U32(units) => letter == 0 || units.contains(&(letter as u32)),
        }
    }
}

/// Immutable view of a language's eight scalar letter groups. A wide list
/// overrides its bitfield group; a positive offset selects a 255-unit alphabet.
#[derive(Clone, Copy, Debug)]
pub struct LetterSet<'a> {
    pub bits: &'a [u8; 256],
    pub offset: i32,
    pub groups: [Option<WideLetters<'a>>; 8],
}
impl LetterSet<'_> {
    /// Return the original mask (1..128), or 1 for a wide-list match.
    /// Invalid groups and characters outside the configured alphabet return 0.
    pub fn mask(&self, letter: i32, group: u32) -> u8 {
        let Some(wide) = self.groups.get(group as usize) else {
            return 0;
        };
        if let Some(wide) = wide {
            return u8::from(wide.contains(letter));
        }
        let index = if self.offset > 0 {
            let shifted = i64::from(letter) - i64::from(self.offset);
            if !(1..256).contains(&shifted) {
                return 0;
            }
            shifted as usize
        } else if (0xc0..0x25e).contains(&letter) {
            usize::from(data::REMOVE_ACCENT[(letter - 0xc0) as usize])
        } else if (0..256).contains(&letter) {
            letter as usize
        } else {
            return 0;
        };
        self.bits[index] & (1 << group)
    }

    pub fn is_letter(&self, letter: u32, group: u8) -> bool {
        self.mask(letter as i32, u32::from(group)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_accents_and_offset_boundaries_match_legacy_alphabets() {
        let mut bits = [0; 256];
        bits[b'a' as usize] = 0x81;
        bits[b'y' as usize] = 0x40;
        bits[0] = 0x02;
        bits[255] = 0x04;
        let mut letters = LetterSet {
            bits: &bits,
            offset: 0,
            groups: [None; 8],
        };
        assert_eq!(letters.mask('a' as i32, 7), 128);
        assert_eq!(letters.mask('ä' as i32, 7), 128);
        assert_eq!(letters.mask('ÿ' as i32, 6), 64);
        assert_eq!(letters.mask(0xd7, 1), 2); // accent table maps to byte zero
        assert_eq!(letters.mask(0x25e, 1), 0);
        assert_eq!(letters.mask(-1, 7), 0);
        assert_eq!(letters.mask(0, 8), 0);
        assert_eq!(letters.mask(0, u32::MAX), 0);
        letters.offset = 0x400;
        assert_eq!(letters.mask(0x400, 1), 0);
        assert_eq!(letters.mask(0x461, 7), 128);
        assert_eq!(letters.mask(0x4ff, 2), 4);
        assert_eq!(letters.mask(0x500, 2), 0);
        assert_eq!(letters.mask(i32::MIN, 1), 0);
        letters.offset = i32::MAX;
        assert_eq!(letters.mask(i32::MIN, 1), 0);
    }

    #[test]
    fn wide_groups_override_bits_and_compare_platform_code_units() {
        let bits = [255; 256];
        let mut letters = LetterSet {
            bits: &bits,
            offset: 0x400,
            groups: [None; 8],
        };
        letters.groups[7] = Some(WideLetters::U32(&[0x1f600, 0x451]));
        assert_eq!(letters.mask(0x1f600, 7), 1);
        assert_eq!(letters.mask(0x461, 7), 0);
        assert_eq!(letters.mask(0, 7), 1);
        letters.groups[7] = Some(WideLetters::U16(&[0xf600, 0x451]));
        assert_eq!(letters.mask(0x1f600, 7), 1); // wchar_t truncation, not UTF-16 decoding
        assert_eq!(letters.mask(0x10000, 7), 1);
        letters.groups[7] = Some(WideLetters::U32(&[]));
        assert_eq!(letters.mask(0, 7), 1);
        assert_eq!(letters.mask('a' as i32, 7), 0);
    }
}
