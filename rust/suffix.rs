//! Bounded native suffix removal and language-specific spelling repairs.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::letters::LetterSet;
pub const WORD_BYTES: usize = 160;
const SUFFIX_E: u32 = 0x100;
const SUFFIX_I: u32 = 0x200;
const SUFFIX_VERB: u32 = 0x800;
const FLAG_SUFFIX: u32 = 4;
const FLAG_S: u32 = 8;
const FLAG_ADDED: u32 = 16;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Context {
    pub language: u32,
    pub added_character: i32,
    pub expect_verb: i32,
    pub signed_bytes: u32,
    /// Three preceding initialized bytes, oldest first. Standalone word owners
    /// normally provide spaces; clause owners retain their actual context.
    pub preceding: [u8; 4],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Effects {
    pub flags: u32,
    pub expect_verb: i32,
    pub added: u32,
    /// Zero means unchanged; otherwise write value-1 to the preceding byte.
    pub preceding: u32,
}
pub struct Outcome {
    pub effects: Effects,
    pub original: [u8; WORD_BYTES],
    pub original_length: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Context,
    Terminator,
    SuffixBound,
    Capacity,
}

/// Compatibility encoding permits surrogate code units, as the C helper does;
/// codes outside 0..0x110000 become one space. No terminator is written.
pub fn encode(code: u32) -> ([u8; 4], usize) {
    let mut bytes = [0; 4];
    if code < 0x80 {
        bytes[0] = code as u8;
        return (bytes, 1);
    }
    if code >= 0x110000 {
        bytes[0] = b' ';
        return (bytes, 1);
    }
    let following = if code < 0x800 {
        1
    } else if code < 0x10000 {
        2
    } else {
        3
    };
    bytes[0] = [0, 0xc0, 0xe0, 0xf0][following] | (code >> (6 * following)) as u8;
    for (index, byte) in bytes.iter_mut().enumerate().take(following + 1).skip(1) {
        *byte = 0x80 + ((code >> (6 * (following - index))) & 0x3f) as u8;
    }
    (bytes, following + 1)
}
fn previous(bytes: &[u8], position: usize, distance: usize, context: &Context) -> u8 {
    position.checked_sub(distance).map_or_else(
        || context.preceding[3 + position - distance],
        |index| restored(bytes[index]),
    )
}
fn restored(byte: u8) -> u8 {
    if byte == b'E' {
        b'e'
    } else {
        byte
    }
}
fn ends_with(word: &[u8], start: usize, suffix: &[u8], last: Option<u8>) -> bool {
    let Some(position) = start.checked_sub(suffix.len()) else {
        return false;
    };
    suffix.iter().enumerate().all(|(offset, expected)| {
        let index = position + offset;
        let byte = if index + 1 == start {
            last.unwrap_or_else(|| restored(word[index]))
        } else {
            restored(word[index])
        };
        byte == *expected
    })
}
fn letter(letters: &LetterSet<'_>, byte: u8, group: u32, signed: bool) -> bool {
    let code = if signed {
        i32::from(byte as i8)
    } else {
        i32::from(byte)
    };
    letters.mask(code, group) != 0
}
/// Mutate only after validating suffix bounds and spelling-repair capacity.
/// `word` is a writable initialized span containing its first space delimiter;
/// additional initialized trailing bytes permit multibyte spelling repair.
/// Plans bounded edits and a 160-byte original-word copy without copying the
/// complete clause. Long words are supported; only the original copy is clipped.
/// Runtime work is linear in the caller-admitted word span, on its owner/worker.
/// Letters/context are immutable and disjoint.
pub fn remove(
    word: &mut [u8],
    ending: u32,
    context: &Context,
    letters: &LetterSet<'_>,
) -> Result<Outcome, Error> {
    if context.signed_bytes > 1 {
        return Err(Error::Context);
    }
    let length = word
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or(Error::Terminator)?;
    let mut original = [0; WORD_BYTES];
    let original_length = length.min(WORD_BYTES - 1);
    for (index, byte) in original.iter_mut().enumerate().take(original_length) {
        *byte = restored(word[index]);
    }
    let mut start = length;
    for _ in 0..ending & 0x3f {
        start = start.checked_sub(1).ok_or(Error::SuffixBound)?;
        while word[start] & 0xc0 == 0x80 {
            start = start.checked_sub(1).ok_or(Error::SuffixBound)?;
        }
    }
    let removed = (length - start).min(49);
    let mut suffix = [0; 49];
    for (index, byte) in suffix.iter_mut().enumerate().take(removed) {
        *byte = restored(word[start + index]);
    }
    let mut flags = (ending & 0xfff0) | FLAG_SUFFIX;
    let signed = context.signed_bytes != 0;
    let mut last_repair = if ending & SUFFIX_I != 0 && previous(word, start, 1, context) == b'i' {
        Some(b'y')
    } else {
        None
    };
    let mut repair = [0; 4];
    let mut repair_length = 0;
    let mut added = false;
    if ending & SUFFIX_E != 0 {
        let last = last_repair.unwrap_or_else(|| previous(word, start, 1, context));
        let before = previous(word, start, 2, context);
        if context.language == u32::from_be_bytes([0, 0, b'n', b'l']) {
            if last & 0x80 == 0
                && before & 0x80 == 0
                && letter(letters, before, 7, signed)
                && letter(letters, last, 2, signed)
                && !letter(letters, previous(word, start, 3, context), 7, signed)
            {
                if start + 1 >= word.len() {
                    return Err(Error::Capacity);
                }
                repair[..2].copy_from_slice(&[last, b' ']);
                repair_length = 2;
                last_repair = Some(before);
            }
        } else if context.language == u32::from_be_bytes([0, 0, b'e', b'n']) {
            if letter(letters, before, 7, signed) && letter(letters, last, 1, signed) {
                if !ends_with(word, start, b"ion", last_repair) {
                    flags |= FLAG_ADDED;
                }
            } else if [
                b"c".as_slice(),
                b"rs",
                b"ir",
                b"ur",
                b"ath",
                b"ns",
                b"u",
                b"spong",
                b"rang",
                b"larg",
            ]
            .iter()
            .any(|suffix| ends_with(word, start, suffix, last_repair))
            {
                flags |= FLAG_ADDED;
            }
        } else if context.added_character != 0 {
            flags |= FLAG_ADDED;
        }
        if flags & FLAG_ADDED != 0 {
            let (bytes, count) = encode(context.added_character as u32);
            if start + count > word.len() {
                return Err(Error::Capacity);
            }
            repair = bytes;
            repair_length = count;
            added = true;
        }
    }
    let expect_verb = if ending & SUFFIX_VERB != 0 && context.expect_verb == 0 {
        1
    } else {
        context.expect_verb
    };
    if &suffix[..removed] == b"s" || &suffix[..removed] == b"es" {
        flags |= FLAG_S;
    }
    if suffix.first() == Some(&b'\'') {
        flags &= !FLAG_SUFFIX;
    }
    for byte in &mut word[..length] {
        *byte = restored(*byte);
    }
    word[start..start + removed].fill(b' ');
    let preceding = if let Some(byte) = last_repair {
        if start > 0 {
            word[start - 1] = byte;
            0
        } else {
            u32::from(byte) + 1
        }
    } else {
        0
    };
    word[start..start + repair_length].copy_from_slice(&repair[..repair_length]);
    Ok(Outcome {
        effects: Effects {
            flags,
            expect_verb,
            added: u32::from(added),
            preceding,
        },
        original,
        original_length,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn context(language: &[u8; 2]) -> Context {
        Context {
            language: u32::from_be_bytes([0, 0, language[0], language[1]]),
            added_character: 101,
            expect_verb: 0,
            signed_bytes: 1,
            preceding: [b' '; 4],
        }
    }
    fn bits() -> [u8; 256] {
        let mut bits = [0; 256];
        for byte in b"aeiouy" {
            bits[*byte as usize] = 128;
        }
        for byte in b"bcdfgjklmnpqrstvwxz" {
            bits[*byte as usize] = 6;
        }
        bits
    }
    #[test]
    fn language_repairs_keep_original_and_suffix_flags() {
        let bits = bits();
        let letters = LetterSet {
            bits: &bits,
            offset: 0,
            groups: [None; 8],
        };
        let mut word = *b"making   ";
        let outcome = remove(
            &mut word,
            3 | SUFFIX_E | SUFFIX_VERB,
            &context(b"en"),
            &letters,
        )
        .unwrap();
        assert_eq!(&word[..5], b"make ");
        assert_eq!(&outcome.original[..7], b"making\0");
        assert_eq!(outcome.effects.flags & FLAG_ADDED, FLAG_ADDED);
        assert_eq!(outcome.effects.expect_verb, 1);
        let mut word = *b"mannen   ";
        remove(&mut word, 2 | SUFFIX_E, &context(b"nl"), &letters).unwrap();
        assert_eq!(&word[..5], b"mann ");
        let mut word = *b"paden   ";
        remove(&mut word, 2 | SUFFIX_E, &context(b"nl"), &letters).unwrap();
        assert_eq!(&word[..5], b"paad ");
        let mut word = *b"tries   ";
        remove(&mut word, 2 | SUFFIX_I, &context(b"en"), &letters).unwrap();
        assert_eq!(&word[..4], b"try ");
    }
    #[test]
    fn utf8_suffixes_and_copy_restore_discarded_e_and_bounds_are_transactional() {
        let bits = bits();
        let letters = LetterSet {
            bits: &bits,
            offset: 0,
            groups: [None; 8],
        };
        let mut word = "cafEé世    ".as_bytes().to_vec();
        let outcome = remove(&mut word, 2, &context(b"fr"), &letters).unwrap();
        assert_eq!(&word[..9], b"cafe     ");
        assert_eq!(&outcome.original[..9], "cafeé世".as_bytes());
        let previous = word.clone();
        assert!(remove(&mut word, 63, &context(b"fr"), &letters).is_err());
        assert_eq!(word, previous);
        let mut small = *b"a ";
        let previous = small;
        let mut context = context(b"fr");
        context.added_character = 0x1f642;
        assert_eq!(
            remove(&mut small, 1 | SUFFIX_E, &context, &letters).err(),
            Some(Error::Capacity)
        );
        assert_eq!(small, previous);
    }
    #[test]
    fn encoding_preserves_code_units_and_replaces_out_of_range_codes() {
        assert_eq!(encode(0x1f642), ([0xf0, 0x9f, 0x99, 0x82], 4));
        assert_eq!(encode(0xd800), ([0xed, 0xa0, 0x80, 0], 3));
        assert_eq!(encode(u32::MAX), ([b' ', 0, 0, 0], 1));
        assert_eq!(encode(0), ([0; 4], 1));
    }
    #[test]
    fn long_suffix_chains_shorten_full_words_while_original_copies_are_clipped() {
        let bits = bits();
        let letters = LetterSet {
            bits: &bits,
            offset: 0,
            groups: [None; 8],
        };
        let mut word = b"s'".repeat(220);
        word.extend_from_slice(b"   ");
        for remaining in (1..=220).rev() {
            let outcome = remove(&mut word, 2, &context(b"en"), &letters).unwrap();
            assert_eq!(outcome.original_length, (remaining * 2).min(159));
            assert_eq!(outcome.original[outcome.original_length], 0);
            assert_eq!(
                word.iter().position(|byte| *byte == b' '),
                Some((remaining - 1) * 2)
            );
        }
    }
}
