//! Dictionary-list policy over serialized dictionary and translator primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_lookup::PHONEME_BYTES;

pub const WORD_BYTES: usize = 160;
pub const SKIP_WORDS: u32 = 0x80;
pub const MAX_THREE: u32 = 0x0800_0000;
pub const TEXT_MODE: u32 = 0x2000_0000;
pub const ACCENT: u32 = 0x800;
pub const ALLOW_TEXT: u32 = 2;
const ADDED_E: u32 = 0x10;
const DOUBLED: u32 = 0x1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Source,
    Phonemes,
    Capacity,
    State,
}

/// Each callback borrows only owned, initialized scratch through its return.
/// No source/translator loan survives a callback that can invoke translation.
pub trait Host {
    fn byte(&self, position: usize) -> Option<u8>;
    fn lookup(
        &mut self,
        key: &[u8; WORD_BYTES],
        next: usize,
        flags: &mut [u32; 2],
        phonemes: &mut [u8; PHONEME_BYTES],
    ) -> Result<Option<usize>, Error>;
    fn repeat(&self, output: &mut [u8; 20]) -> i32;
    fn set_repeat(&mut self, output: &[u8; 20], count: i32);
    fn text_mode(&self) -> bool;
    fn skip_words(&mut self, count: i32);
    fn accent(&mut self, code: u32, capacity: usize, phonemes: &mut [u8; PHONEME_BYTES]);
    /// Publish the padded, terminated replacement to the serialized owner.
    fn replacement(&mut self, text: &[u8; WORD_BYTES]);
    /// Trace the original matched source prefix and the just-published text.
    fn trace_replacement(&mut self, matched: usize);
}
fn length(bytes: &[u8]) -> Result<usize, Error> {
    bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Phonemes)
}
fn publish(phonemes: &[u8; PHONEME_BYTES], output: &mut [u8]) -> Result<(), Error> {
    let size = length(phonemes)?;
    if size >= output.len() {
        return Err(Error::Capacity);
    }
    output[..=size].copy_from_slice(&phonemes[..=size]);
    Ok(())
}
fn lookup(
    host: &mut impl Host,
    key: &[u8; WORD_BYTES],
    next: usize,
    flags: &mut [u32; 2],
    phonemes: &mut [u8; PHONEME_BYTES],
) -> Result<Option<usize>, Error> {
    let found = host.lookup(key, next, flags, phonemes)?;
    length(phonemes)?;
    Ok(found)
}

/// Fold dotted abbreviations, suppress repeated symbols, retry stems and admit
/// text replacements. On failure output/flags remain untouched; primitive
/// translator effects already executed remain live and must never be replayed.
pub fn dictionary_list(
    host: &mut impl Host,
    end_flags: u32,
    flags: &mut [u32; 2],
    output: &mut [u8],
) -> Result<bool, Error> {
    if output.is_empty() || output.len() > PHONEME_BYTES {
        return Err(Error::Capacity);
    }
    let mut selected = *flags;
    let mut phonemes = [0; PHONEME_BYTES];
    let mut word = [0; WORD_BYTES];
    let mut size = 0;
    let mut cursor = 0;
    loop {
        let first = host.byte(cursor).ok_or(Error::Source)?;
        let width = match first {
            0..=0x7f => 1,
            0x80..=0xdf => 2,
            0xe0..=0xef => 3,
            _ => 4,
        };
        if host.byte(cursor + width) != Some(b' ') || host.byte(cursor + width + 1) != Some(b'.') {
            break;
        }
        if width + 1 > WORD_BYTES - size {
            size = 0;
            break;
        }
        for byte in &mut word[size..size + width] {
            *byte = host.byte(cursor).ok_or(Error::Source)?;
            cursor += 1;
        }
        size += width;
        word[size] = b'.';
        size += 1;
        cursor += 3;
    }
    if size > 0 {
        let mut tail = 0;
        while !matches!(host.byte(cursor + tail).ok_or(Error::Source)?, 0 | b' ') {
            tail += 1;
            if tail > WORD_BYTES {
                break;
            }
        }
        if size + tail < WORD_BYTES {
            for (index, byte) in word[size..size + tail].iter_mut().enumerate() {
                *byte = host.byte(cursor + index).ok_or(Error::Source)?;
            }
            word[size + tail] = 0;
            if lookup(host, &word, cursor, &mut selected, &mut phonemes)?.is_some() {
                selected[0] |= SKIP_WORDS;
                host.skip_words(size as i32);
                publish(&phonemes, output)?;
                *flags = selected;
                return Ok(true);
            }
        }
    }
    size = 0;
    cursor = 0;
    while size < WORD_BYTES - 1 {
        let byte = host.byte(cursor).ok_or(Error::Source)?;
        cursor += 1;
        if byte == 0
            || byte == b' '
            || (byte == b'.' && size > 0 && crate::common_text::digit09(u32::from(word[size - 1])))
        {
            break;
        }
        word[size] = byte;
        size += 1;
    }
    word[size] = 0;
    let mut found = lookup(host, &word, cursor, &mut selected, &mut phonemes)?;
    let mut repeated = [0; 20];
    let count = host.repeat(&mut repeated);
    if selected[0] & MAX_THREE != 0 {
        let ph_size = length(&phonemes)?;
        let old_size = length(&repeated)?;
        if phonemes[..ph_size] == repeated[..old_size] {
            let count = count.checked_add(1).ok_or(Error::State)?;
            host.set_repeat(&repeated, count);
            if count > 3 {
                phonemes[0] = 0;
            }
        } else {
            let copied = ph_size.min(repeated.len() - 1);
            repeated[..copied].copy_from_slice(&phonemes[..copied]);
            repeated[copied..].fill(0);
            host.set_repeat(&repeated, 1);
        }
    } else {
        host.set_repeat(&repeated, 0);
    }
    if found.is_none() && selected[1] & ACCENT != 0 {
        let start = usize::from(word[0] == b'_');
        let character = crate::utf8::decode(&word, start, false).map_err(|_| Error::Source)?;
        host.accent(character.code, output.len(), &mut phonemes);
        length(&phonemes)?;
        // The legacy accent branch synthesizes a found pointer into its local
        // key. It has no original-source trace extent; never subtract unrelated
        // pointers when a malformed combination also requests text replacement.
        found = Some(start + character.width);
    }
    if found.is_none() && size >= 2 {
        phonemes[0] = 0;
        if (end_flags & ADDED_E != 0 && word[size - 1] == b'e')
            || (end_flags & DOUBLED != 0 && word[size - 1] == word[size - 2])
        {
            word[size - 1] = 0;
            found = lookup(host, &word, cursor, &mut selected, &mut phonemes)?;
        }
    }
    let result = if let Some(matched) = found {
        if host.text_mode() {
            selected[0] ^= TEXT_MODE;
        }
        if selected[0] & TEXT_MODE != 0 {
            if end_flags & ALLOW_TEXT != 0 {
                let ph_size = length(&phonemes)?;
                if ph_size + 4 > WORD_BYTES {
                    return Err(Error::Capacity);
                }
                let mut replacement = [0; WORD_BYTES];
                replacement[1] = b' ';
                replacement[2..2 + ph_size].copy_from_slice(&phonemes[..ph_size]);
                replacement[2 + ph_size] = b' ';
                host.replacement(&replacement);
                host.trace_replacement(matched);
            }
            phonemes[0] = 0;
            false
        } else {
            true
        }
    } else {
        phonemes[0] = 0;
        false
    };
    publish(&phonemes, output)?;
    *flags = selected;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Reply {
        found: Option<usize>,
        flags: [u32; 2],
        phonemes: Vec<u8>,
    }
    struct Fixture {
        source: Vec<u8>,
        replies: VecDeque<Reply>,
        calls: Vec<(Vec<u8>, usize, [u32; 2])>,
        repeated: [u8; 20],
        count: i32,
        mode: bool,
        toggle_mode: bool,
        skip: i32,
        accent: Vec<u8>,
        accent_calls: Vec<(u32, usize)>,
        replacements: Vec<[u8; WORD_BYTES]>,
        traces: Vec<usize>,
    }
    impl Fixture {
        fn new(source: &[u8], replies: &[(Option<usize>, [u32; 2], &[u8])]) -> Self {
            Self {
                source: source.to_vec(),
                replies: replies
                    .iter()
                    .map(|(found, flags, phonemes)| Reply {
                        found: *found,
                        flags: *flags,
                        phonemes: phonemes.to_vec(),
                    })
                    .collect(),
                calls: vec![],
                repeated: [0; 20],
                count: 0,
                mode: false,
                toggle_mode: false,
                skip: 77,
                accent: vec![],
                accent_calls: vec![],
                replacements: vec![],
                traces: vec![],
            }
        }
    }
    impl Host for Fixture {
        fn byte(&self, position: usize) -> Option<u8> {
            self.source.get(position).copied()
        }
        fn lookup(
            &mut self,
            key: &[u8; WORD_BYTES],
            next: usize,
            flags: &mut [u32; 2],
            phonemes: &mut [u8; PHONEME_BYTES],
        ) -> Result<Option<usize>, Error> {
            self.calls
                .push((key[..length(key).unwrap()].to_vec(), next, *flags));
            let reply = self.replies.pop_front().expect("unexpected lookup");
            *flags = reply.flags;
            phonemes[..reply.phonemes.len()].copy_from_slice(&reply.phonemes);
            if self.toggle_mode {
                self.mode = !self.mode;
            }
            Ok(reply.found)
        }
        fn repeat(&self, output: &mut [u8; 20]) -> i32 {
            *output = self.repeated;
            self.count
        }
        fn set_repeat(&mut self, output: &[u8; 20], count: i32) {
            self.repeated = *output;
            self.count = count;
        }
        fn text_mode(&self) -> bool {
            self.mode
        }
        fn skip_words(&mut self, count: i32) {
            self.skip = count;
        }
        fn accent(&mut self, code: u32, capacity: usize, phonemes: &mut [u8; PHONEME_BYTES]) {
            self.accent_calls.push((code, capacity));
            phonemes[..self.accent.len()].copy_from_slice(&self.accent);
        }
        fn replacement(&mut self, text: &[u8; WORD_BYTES]) {
            self.replacements.push(*text);
        }
        fn trace_replacement(&mut self, matched: usize) {
            self.traces.push(matched);
        }
    }
    #[test]
    fn dotted_abbreviations_keep_byte_skip_count_and_bypass_repeat_policy() {
        let mut host = Fixture::new(
            "é . b . c tail\0".as_bytes(),
            &[(Some(11), [TEXT_MODE, ACCENT], b"ab\0")],
        );
        host.count = 9;
        let mut flags = [0, 5];
        let mut out = [0x97; 8];
        assert_eq!(
            dictionary_list(&mut host, ALLOW_TEXT, &mut flags, &mut out),
            Ok(true)
        );
        assert_eq!(host.calls, [("é.b.c".as_bytes().to_vec(), 9, [0, 5])]);
        assert_eq!(host.skip, 5);
        assert_eq!(host.count, 9);
        assert!(host.replacements.is_empty());
        assert_eq!(flags, [TEXT_MODE | SKIP_WORDS, ACCENT]);
        assert_eq!(&out[..4], &[b'a', b'b', 0, 0x97]);
    }
    #[test]
    fn numeric_dot_and_terminal_nul_keep_the_next_source_offset() {
        for (text, key, next) in [
            (b"2.-ig\0".as_slice(), b"2".as_slice(), 2),
            (b"cat\0", b"cat", 4),
        ] {
            let mut host = Fixture::new(text, &[(None, [0, 0], b"\0")]);
            let mut flags = [7, 8];
            let mut out = [0x97; 2];
            assert_eq!(
                dictionary_list(&mut host, 0, &mut flags, &mut out),
                Ok(false)
            );
            assert_eq!(host.calls, [(key.to_vec(), next, [7, 8])]);
            assert_eq!(out, [0, 0x97]);
        }
    }
    #[test]
    fn repeat_suppression_uses_full_pronunciation_and_bounded_cached_prefix() {
        let mut host = Fixture::new(b"!\0", &[(Some(2), [MAX_THREE, 0], b"ab\0")]);
        host.repeated[..3].copy_from_slice(b"ab\0");
        host.count = 3;
        let mut flags = [0, 0];
        let mut out = [0x97; 4];
        assert_eq!(
            dictionary_list(&mut host, 0, &mut flags, &mut out),
            Ok(true)
        );
        assert_eq!(host.count, 4);
        assert_eq!(out, [0, 0x97, 0x97, 0x97]);
        let mut host = Fixture::new(
            b"!\0",
            &[(Some(2), [MAX_THREE, 0], b"abcdefghijklmnopqrstuv\0")],
        );
        assert_eq!(
            dictionary_list(&mut host, 0, &mut flags, &mut [0; 24]),
            Ok(true)
        );
        assert_eq!(&host.repeated, b"abcdefghijklmnopqrs\0");
        assert_eq!(host.count, 1);
    }
    #[test]
    fn accent_miss_preserves_dictionary_prefix_when_child_is_a_no_op() {
        let mut host = Fixture::new("_é\0".as_bytes(), &[(None, [0, ACCENT], b"\x82\0")]);
        let mut flags = [0, 0];
        let mut out = [0x97; 3];
        assert_eq!(
            dictionary_list(&mut host, 0, &mut flags, &mut out),
            Ok(true)
        );
        assert_eq!(host.accent_calls, [(0xe9, 3)]);
        assert_eq!(out, [0x82, 0, 0x97]);
    }
    #[test]
    fn added_e_and_doubled_stem_retries_keep_original_next_position() {
        for (text, key, end) in [
            (b"cake\0".as_slice(), b"cak".as_slice(), ADDED_E),
            (b"fall\0", b"fal", DOUBLED),
            (b"fall\0", b"fal", ADDED_E | DOUBLED),
        ] {
            let mut host = Fixture::new(
                text,
                &[(None, [0, 0], b"prefix\0"), (Some(5), [1, 2], b"a\0")],
            );
            let mut flags = [0, 0];
            let mut out = [0x97; 3];
            assert_eq!(
                dictionary_list(&mut host, end, &mut flags, &mut out),
                Ok(true)
            );
            assert_eq!(host.calls[1], (key.to_vec(), 5, [0, 0]));
            assert_eq!(flags, [1, 2]);
            assert_eq!(out, [b'a', 0, 0x97]);
        }
    }
    #[test]
    fn replacement_policy_reads_fresh_text_mode_and_requires_original_word() {
        for allow in [0, ALLOW_TEXT] {
            let mut host = Fixture::new(b"cat dog\0", &[(Some(7), [0, 2], b"name\0")]);
            host.toggle_mode = true;
            let mut flags = [0, 0];
            let mut out = [0x97; 2];
            assert_eq!(
                dictionary_list(&mut host, allow, &mut flags, &mut out),
                Ok(false)
            );
            assert_eq!(flags, [TEXT_MODE, 2]);
            assert_eq!(out, [0, 0x97]);
            if allow == 0 {
                assert!(host.replacements.is_empty());
                assert!(host.traces.is_empty());
            } else {
                assert_eq!(&host.replacements[0][..8], b"\0 name \0");
                assert_eq!(host.traces, [7]);
            }
        }
    }
    #[test]
    fn malformed_output_and_short_publication_keep_output_and_flags_atomic() {
        for phonemes in [vec![0x82; PHONEME_BYTES], b"abc\0".to_vec()] {
            let mut host = Fixture::new(b"cat\0", &[(Some(4), [1, 2], &phonemes)]);
            let mut flags = [7, 8];
            let mut out = [0x97; 3];
            assert!(dictionary_list(&mut host, 0, &mut flags, &mut out).is_err());
            assert_eq!(flags, [7, 8]);
            assert_eq!(out, [0x97; 3]);
            assert_eq!(host.calls.len(), 1);
        }
    }
    #[test]
    fn overlong_replacement_and_repeat_overflow_fail_without_replaying_lookup() {
        let mut text = vec![b'a'; 157];
        text.push(0);
        let mut host = Fixture::new(b"cat\0", &[(Some(4), [TEXT_MODE, 0], &text)]);
        let mut flags = [0, 0];
        let mut out = [0x97; 3];
        assert_eq!(
            dictionary_list(&mut host, ALLOW_TEXT, &mut flags, &mut out),
            Err(Error::Capacity)
        );
        assert!(host.replacements.is_empty());
        assert_eq!(out, [0x97; 3]);
        let mut host = Fixture::new(b"!\0", &[(Some(2), [MAX_THREE, 0], b"a\0")]);
        host.count = i32::MAX;
        host.repeated[..2].copy_from_slice(b"a\0");
        assert_eq!(
            dictionary_list(&mut host, 0, &mut flags, &mut out),
            Err(Error::State)
        );
        assert_eq!(host.calls.len(), 1);
    }
}
