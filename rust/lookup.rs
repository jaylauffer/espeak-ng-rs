//! Contextual dictionary matching, with explicit immutable grammatical state.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::dictionary::{Entry, InvalidDictionary};

pub const FOUND_ATTRIBUTES: u32 = 0x40000000;
pub const FOUND: u32 = 0x80000000;
const SUFFIX: u32 = 4;
const SUFFIX_S: u32 = 8;
const PREFIX: u32 = 0x400;
const VERB_SUFFIX: u32 = 0x800;

/// Values mirror the fork's serialized flags, without borrowing a Translator.
#[derive(Default, Clone, Copy, Debug)]
#[repr(C)]
pub struct Context {
    pub conditions: u32,
    pub end_flags: u32,
    pub word_flags: u32,
    pub lookup_symbol: u32,
    pub language: u32,
    pub previous_flags: u32,
    pub expect_verb: i32,
    pub expect_verb_s: i32,
    pub expect_past: i32,
    pub expect_noun: i32,
    pub native_translator: u32,
    pub sentence: u32,
    pub single_symbol: u32,
    /// Bytes from the next-word pointer to clause end; zero when already past it.
    pub clause_remaining: usize,
}
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct WordInfo {
    pub flags: u32,
    pub length: u32,
}

/// Keep the last attempted phoneme copy and skip count even on rejection: the
/// serialized C API exposes these historical side effects. The selected flags
/// are separate; an attributes-only match has no word_end but does have flags.
#[derive(Default, Debug)]
pub struct Outcome<'a> {
    pub phonemes: Option<&'a [u8]>,
    pub flags: Option<[u32; 2]>,
    pub(crate) trace_flags: Option<[u32; 2]>,
    pub word_end: Option<usize>,
    pub skipwords: Option<u32>,
}

fn prefix_matches(text: &[u8], expected: &[u8]) -> bool {
    for (index, &byte) in expected.iter().enumerate() {
        let actual = text.get(index).copied().unwrap_or(0);
        if byte != actual {
            return false;
        }
        if byte == 0 {
            return true;
        }
    }
    true
}
fn accepts(flags: [u32; 2], context: &Context, end: usize) -> bool {
    let [first, second] = flags;
    let suffix = context.end_flags & SUFFIX != 0;
    if !suffix && second & 0x10000 != 0 {
        return false;
    } // stem needs suffix
    if context.end_flags & PREFIX != 0 && second & 0xc000 != 0 {
        return false;
    }
    if suffix
        && (second & 0x4000 != 0 || (second & 0x8000 != 0 && context.end_flags & SUFFIX_S == 0))
    {
        return false;
    }
    if second & 0x200 != 0 && context.word_flags & 2 == 0 {
        return false;
    }
    if second & 0x400 != 0 && context.word_flags & 1 == 0 {
        return false;
    }
    if first & 0x02000000 != 0 && context.word_flags & 0x10000 == 0 {
        return false;
    }
    if second & 0x20000 != 0 && end < context.clause_remaining && context.lookup_symbol == 0 {
        return false;
    }
    if second & 0x40000 != 0 && context.word_flags & 0x200 == 0 {
        return false;
    }
    if second & 0x2000 != 0 && context.sentence == 0 {
        return false;
    }
    if second & 0x10 != 0 {
        if context.expect_verb == 0
            && !(context.expect_verb_s != 0 && context.end_flags & SUFFIX_S != 0)
        {
            return false;
        }
        if context.language == 0x656e
            && context.previous_flags & 0x200000 != 0
            && context.end_flags & SUFFIX_S != 0
        {
            return false;
        }
    }
    if second & 0x40 != 0 && context.expect_past == 0 {
        return false;
    }
    if second & 0x20 != 0 && (context.expect_noun == 0 || context.end_flags & VERB_SUFFIX != 0) {
        return false;
    }
    if second & 0x80000 != 0 && context.native_translator == 0 {
        return false;
    }
    if first & 0x10000 != 0 && context.language == 0x6875 && context.previous_flags & 0x8000 == 0 {
        return false;
    }
    true
}

/// One already indexed bucket. Performs no allocation or I/O. `descriptor`
/// includes the compression bit; `key` may include a legacy unchanged hash tail.
pub fn lookup_bucket<'a>(
    bucket: &'a [u8],
    key: &[u8],
    descriptor: usize,
    next_words: &[u8],
    context: &Context,
    words: Option<&[WordInfo]>,
) -> Result<Outcome<'a>, InvalidDictionary> {
    let mut outcome = Outcome::default();
    let mut cursor = 0;
    loop {
        let length = usize::from(
            *bucket
                .get(cursor)
                .ok_or(InvalidDictionary("missing bucket terminator"))?,
        );
        if length == 0 {
            return Ok(outcome);
        }
        let record = bucket
            .get(cursor..cursor + length)
            .ok_or(InvalidDictionary("truncated lookup record"))?;
        cursor += length;
        let entry = Entry::parse(record)?;
        let recorded_descriptor = entry.word.len() | if entry.compressed { 0x40 } else { 0 };
        if descriptor != recorded_descriptor || key.get(..descriptor & 0x3f) != Some(entry.word) {
            continue;
        }
        let phonemes = entry.phonemes.unwrap_or(&[]);
        if phonemes.len() >= 160 {
            return Err(InvalidDictionary(
                "pronunciation exceeds legacy phoneme buffer",
            ));
        }
        outcome.phonemes = Some(phonemes);
        let mut flags = [0_u32; 2];
        let mut failed = false;
        let mut end = 0;
        for (position, &flag) in entry.flags.iter().enumerate() {
            if flag >= 100 {
                let (bit, inverted) = if flag >= 132 {
                    (flag - 132, true)
                } else {
                    (flag - 100, false)
                };
                let mask = 1_u32
                    .checked_shl(u32::from(bit))
                    .ok_or(InvalidDictionary("condition exceeds 31"))?;
                if (context.conditions & mask != 0) == inverted {
                    failed = true;
                }
            } else if flag > 80 {
                let count = u32::from(flag - 80);
                if let Some(words) = words.filter(|words| words.len() > count as usize) {
                    for word in words[..=count as usize]
                        .iter()
                        .take_while(|word| word.length != 0)
                    {
                        if word.flags & 0xc00 != 0 {
                            failed = true;
                        }
                    }
                }
                let suffix = &entry.flags[position + 1..];
                if suffix.len() > next_words.len() || !prefix_matches(next_words, suffix) {
                    failed = true;
                }
                if !failed {
                    flags[0] |= 0x80;
                    outcome.skipwords = Some(count);
                    end = suffix.len();
                }
                break;
            } else if flag > 64 {
                flags[0] = (flags[0] & !15) | u32::from(flag & 15);
                if flag & 12 == 12 {
                    flags[0] |= 0x200;
                }
            } else if flag >= 32 {
                // For reserved flag byte 64, the 64-bit C shift is
                // discarded when stored in 32-bit flags. Preserve the no-op
                // explicitly, without shifting a Rust u32 by 32.
                flags[1] |= 1_u32.checked_shl(u32::from(flag - 32)).unwrap_or(0);
            } else {
                flags[0] |= 1_u32 << flag;
            }
        }
        if failed || !accepts(flags, context, end) {
            continue;
        }
        flags[0] |= FOUND_ATTRIBUTES;
        if !phonemes.is_empty() {
            flags[0] |= FOUND;
            outcome.trace_flags = Some(flags);
            if context.single_symbol != 0 {
                flags[0] |= 0x08000000;
            }
            outcome.word_end = Some(end);
        }
        if outcome.trace_flags.is_none() {
            outcome.trace_flags = Some(flags);
        }
        outcome.flags = Some(flags);
        return Ok(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(phonemes: &[u8], flags: &[u8]) -> Vec<u8> {
        let mut out = vec![
            (6 + phonemes.len() + flags.len()) as u8,
            3,
            b'c',
            b'a',
            b't',
        ];
        out.extend_from_slice(phonemes);
        out.push(0);
        out.extend_from_slice(flags);
        out
    }
    #[test]
    fn conditions_precedence_and_attributes_only_preserve_side_effects() {
        let mut bucket = entry(&[42], &[100]);
        bucket.extend(entry(&[], &[15]));
        bucket.push(0);
        let result = lookup_bucket(&bucket, b"cat", 3, b"", &Context::default(), None).unwrap();
        assert_eq!(result.phonemes, Some(&[][..]));
        assert_eq!(result.flags, Some([FOUND_ATTRIBUTES | 0x8000, 0]));
        assert_eq!(result.word_end, None);
        let result = lookup_bucket(
            &bucket,
            b"cat",
            3,
            b"",
            &Context {
                conditions: 1,
                ..Context::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(result.phonemes, Some(&[42][..]));
        assert_eq!(result.flags, Some([FOUND_ATTRIBUTES | FOUND, 0]));
        assert!(lookup_bucket(&bucket[..3], b"cat", 3, b"", &Context::default(), None).is_err());
    }
    #[test]
    fn multiword_emphasis_suffix_and_verb_context() {
        let mut bucket = entry(&[42], &[36, 81, b'd', b'o', b'g', b' ']);
        bucket.push(0);
        let context = Context {
            expect_verb: 1,
            ..Context::default()
        };
        let words = [
            WordInfo {
                flags: 0,
                length: 3,
            },
            WordInfo {
                flags: 0,
                length: 3,
            },
        ];
        let result = lookup_bucket(&bucket, b"cat", 3, b"dog ", &context, Some(&words)).unwrap();
        assert_eq!(result.word_end, Some(4));
        assert_eq!(result.skipwords, Some(1));
        let emphasized = [
            words[0],
            WordInfo {
                flags: 0x800,
                ..words[1]
            },
        ];
        assert!(
            lookup_bucket(&bucket, b"cat", 3, b"dog ", &context, Some(&emphasized))
                .unwrap()
                .flags
                .is_none()
        );
        assert!(
            lookup_bucket(&bucket, b"cat", 3, b"dog ", &Context::default(), None)
                .unwrap()
                .flags
                .is_none()
        );
    }
    #[test]
    fn packed_key_comparison_includes_embedded_nul_and_preserves_flags_only_match() {
        let bytes = [9, 0x43, 4, 0xe1, 0, 42, 0, 76, 42, 0];
        let result = lookup_bucket(
            &bytes,
            &[4, 0xe1, 0],
            0x43,
            b"",
            &Context {
                word_flags: 1,
                ..Context::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(
            result.flags,
            Some([FOUND | FOUND_ATTRIBUTES | 0x20c, 0x400])
        );
        assert!(
            lookup_bucket(&bytes, &[4, 0xe1], 0x43, b"", &Context::default(), None)
                .unwrap()
                .flags
                .is_none()
        );
        let bytes = [6, 0x83, b'c', b'a', b't', 76, 0];
        let result = lookup_bucket(&bytes, b"cat", 3, b"", &Context::default(), None).unwrap();
        assert_eq!(result.flags, Some([FOUND_ATTRIBUTES | 0x20c, 0]));
        assert_eq!(result.phonemes, Some(&[][..]));
        assert_eq!(result.word_end, None);
    }

    #[test]
    fn rejects_invalid_conditions_and_truncated_multiword_windows() {
        let mut bucket = entry(&[42], &[164]);
        bucket.push(0);
        assert!(lookup_bucket(&bucket, b"cat", 3, b"", &Context::default(), None).is_err());
        let mut bucket = entry(&[42], &[81, b'd', b'o', b'g', b' ']);
        bucket.push(0);
        for next in [b"".as_slice(), b"dog", b"dog\0"] {
            let outcome =
                lookup_bucket(&bucket, b"cat", 3, next, &Context::default(), None).unwrap();
            assert_eq!(outcome.word_end, None);
            assert_eq!(outcome.skipwords, None);
        }
    }
}
