//! Complete ideograph segmentation and recursive dictionary-text expansion.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{language, suffix, translate_word::Error, utf8};
pub const TEXT: usize = 161;
pub const PHRASE: usize = 323;
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    Ideographs,
    Alternate,
    RowFlags,
    Skip,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Class {
    Alpha,
    Space,
    Upper,
    ByteSpace,
    Punctuation,
}
pub trait Host {
    fn value(&self, field: Field) -> i32;
    fn store(&mut self, field: Field, value: i32) -> Result<(), Error>;
    fn classify(&self, code: u32, class: Class) -> bool;
    fn lower(&self, code: u32) -> u32;
    /// Original C source when none, otherwise initialized owned phrase and
    /// numeric offset. The primitive scopes/restores its source context and
    /// keeps no pointer into owned source/replacement scratch after return.
    fn main(
        &mut self,
        source: Option<&mut [u8; PHRASE]>,
        initialized: usize,
        offset: usize,
        replacement: Option<&mut [u8; TEXT]>,
    ) -> Result<u32, Error>;
    fn snapshot(&self, output: &mut [u8; 200]) -> Result<(), Error>;
    fn publish(&mut self, output: &[u8; 200]) -> Result<(), Error>;
}
pub fn split(host: &impl Host, code: u32, previous: u32) -> bool {
    if host.value(Field::Ideographs) != 0 && (code > 0x3040 || previous > 0x3040) {
        return true;
    }
    let alternate = host.value(Field::Alternate);
    if alternate != 0 {
        // Both lookups occur before testing either result, as in the C policy.
        let current = language::alphabet_from_char(code as i32);
        let previous = language::alphabet_from_char(previous as i32);
        return [current, previous]
            .into_iter()
            .flatten()
            .any(|a| a.offset == alternate && a.flags & 0x20 != 0);
    }
    false
}
pub fn segment(host: &impl Host, text: &[u8], output: &mut [u8]) -> Result<usize, Error> {
    if output.is_empty() {
        return Err(Error::Capacity);
    }
    let mut input = 0;
    let mut used = 0;
    let mut code = 0;
    while *text.get(input).ok_or(Error::Source)? != 0 {
        let previous = code;
        let ch = utf8::decode(text, input, false).map_err(|_| Error::Source)?;
        code = ch.code;
        let mut space = false;
        if host.classify(previous, Class::Alpha) {
            if host.classify(code, Class::Alpha) {
                space = split(host, code, previous);
            } else if code == b'-' as u32 {
                let next =
                    utf8::decode(text, input + ch.width, false).map_err(|_| Error::Source)?;
                if host.classify(next.code, Class::Alpha) {
                    if used + 2 > output.len() {
                        break;
                    }
                    output[used] = b' ';
                    used += 1;
                    input += ch.width;
                    code = b' ' as u32;
                    continue;
                }
            } else if !host.classify(code, Class::Space)
                && code >= 0x80
                && !host.classify(code, Class::Punctuation)
            {
                space = true;
            }
        } else if previous >= 0x80
            && !host.classify(previous, Class::Space)
            && host.classify(code, Class::Alpha)
            && !host.classify(previous, Class::Punctuation)
        {
            space = true;
        }
        // The legacy check reserves two bytes even when no space is inserted.
        if used + ch.width + 2 > output.len() {
            break;
        }
        if space {
            output[used] = b' ';
            used += 1;
        }
        let bytes = text.get(input..input + ch.width).ok_or(Error::Source)?;
        output[used..used + ch.width].copy_from_slice(bytes);
        used += ch.width;
        input += ch.width;
    }
    output[used] = 0;
    Ok(used)
}
fn run(
    h: &mut impl Host,
    source: Option<&mut [u8; PHRASE]>,
    initialized: usize,
    offset: usize,
    mut output: Option<&mut [u8; TEXT]>,
    depth: i32,
) -> Result<u32, Error> {
    let flags = h.main(source, initialized, offset, output.as_deref_mut())?;
    let Some(text) = output else {
        return Ok(flags);
    };
    if flags & 0x20000000 == 0 {
        return Ok(flags);
    }
    let mut phrase = [0; PHRASE];
    phrase[1] = b' ';
    phrase[2] = b' ';
    let used = segment(h, text, &mut phrase[3..])?;
    let initialized = used + 4;
    let mut cursor = 3;
    let mut joined = [0; 200];
    let mut position = 0usize;
    let mut available = 200i32;
    let mut first = true;
    while *phrase
        .get(cursor)
        .filter(|_| cursor < initialized)
        .ok_or(Error::Source)?
        != 0
        && available > 1
    {
        let ch = utf8::decode(&phrase[..initialized], cursor, false).map_err(|_| Error::Source)?;
        let mut row = h.value(Field::RowFlags);
        if h.classify(ch.code, Class::Upper) {
            row |= 2;
            h.store(Field::RowFlags, row)?;
            let (lower, width) = suffix::encode(h.lower(ch.code));
            if width == ch.width {
                phrase[cursor..cursor + width].copy_from_slice(&lower[..width]);
            }
        } else {
            row &= !2;
            h.store(Field::RowFlags, row)?;
        }
        let saved_skip = h.value(Field::Skip);
        let result = (|| {
            if depth < 3 {
                let mut replacement = [0; TEXT];
                run(
                    h,
                    Some(&mut phrase),
                    initialized,
                    cursor,
                    Some(&mut replacement),
                    depth + 1,
                )?;
            } else {
                h.main(Some(&mut phrase), initialized, cursor, None)?;
            }
            let mut pronunciation = [0; 200];
            h.snapshot(&mut pronunciation)?;
            let length = pronunciation
                .iter()
                .position(|b| *b == 0)
                .ok_or(Error::Phonemes)?;
            let prefix = usize::from(!first);
            let would = length + prefix;
            let written = would.min(available as usize - 1);
            for i in 0..written {
                joined[position + i] = if prefix != 0 && i == 0 {
                    15
                } else {
                    pronunciation[i - prefix]
                };
            }
            joined[position + written] = 0;
            // Track snprintf's would-have-written scalar without creating an
            // out-of-array pointer when it exceeds the available destination.
            available -= would as i32;
            position += would;
            first = false;
            let skip = h.value(Field::Skip).checked_add(1).ok_or(Error::State)?;
            h.store(Field::Skip, skip)?;
            if skip <= 0 {
                return Err(Error::State);
            }
            while h.value(Field::Skip) > 0 {
                while cursor < initialized
                    && phrase[cursor] != 0
                    && !h.classify(phrase[cursor] as u32, Class::ByteSpace)
                {
                    cursor += 1;
                }
                while cursor < initialized
                    && phrase[cursor] != 0
                    && h.classify(phrase[cursor] as u32, Class::ByteSpace)
                {
                    cursor += 1;
                }
                if cursor >= initialized {
                    return Err(Error::Source);
                }
                h.store(Field::Skip, h.value(Field::Skip) - 1)?;
                // Missing delimiters/excess skips must not walk beyond NUL.
                if phrase[cursor] == 0 {
                    break;
                }
            }
            Ok(())
        })();
        let restored = h.store(Field::Skip, saved_skip);
        result?;
        restored?;
    }
    // An empty first pronunciation does not replace the fresh shared output.
    if position != 0 {
        h.publish(&joined)?;
    }
    Ok(flags)
}
pub fn translate(
    host: &mut impl Host,
    text: Option<&mut [u8; TEXT]>,
    depth: i32,
) -> Result<u32, Error> {
    if depth < 0 {
        return Err(Error::State);
    }
    run(host, None, 0, 0, text, depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        fields: [i32; 4],
        phonemes: [u8; 200],
        text: Vec<u8>,
        child: Vec<u8>,
        calls: usize,
        cycle: bool,
        fail: bool,
        skip: i32,
        mutate: bool,
        keys: Vec<Vec<u8>>,
        case_width: bool,
        published: usize,
    }
    impl Fixture {
        fn new(text: &str) -> Self {
            let mut t = text.as_bytes().to_vec();
            t.push(0);
            Self {
                fields: [0; 4],
                phonemes: [0x55; 200],
                text: t,
                child: b"p\0".to_vec(),
                calls: 0,
                cycle: false,
                fail: false,
                skip: 0,
                mutate: false,
                keys: vec![],
                case_width: false,
                published: 0,
            }
        }
    }
    impl Host for Fixture {
        fn value(&self, f: Field) -> i32 {
            self.fields[f as usize]
        }
        fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
            self.fields[f as usize] = v;
            Ok(())
        }
        fn classify(&self, c: u32, k: Class) -> bool {
            let ch = char::from_u32(c);
            match k {
                Class::Alpha => {
                    crate::common_text::word_alpha(c, |_| ch.is_some_and(char::is_alphabetic))
                }
                Class::Space => ch.is_some_and(char::is_whitespace),
                Class::Upper => ch.is_some_and(char::is_uppercase),
                Class::ByteSpace => (c as u8).is_ascii_whitespace(),
                Class::Punctuation => false,
            }
        }
        fn lower(&self, c: u32) -> u32 {
            if self.case_width && c == b'A' as u32 {
                0x131
            } else {
                char::from_u32(c)
                    .and_then(|c| c.to_lowercase().next())
                    .map_or(c, |c| c as u32)
            }
        }
        fn main(
            &mut self,
            source: Option<&mut [u8; PHRASE]>,
            initialized: usize,
            offset: usize,
            text: Option<&mut [u8; TEXT]>,
        ) -> Result<u32, Error> {
            self.calls += 1;
            if let Some(source) = source {
                assert!(offset < initialized && initialized <= PHRASE);
                let end = source[offset..initialized]
                    .iter()
                    .position(|b| matches!(*b, 0 | b' '))
                    .unwrap()
                    + offset;
                self.keys.push(source[offset..end].to_vec());
                self.fields[Field::Skip as usize] = self.skip;
                self.fields[Field::RowFlags as usize] ^= 0x80;
                if self.mutate && self.calls == 2 {
                    source[end + 1] = b'X';
                }
                if self.fail {
                    self.phonemes[..2].copy_from_slice(b"E\0");
                    return Err(Error::State);
                }
            }
            match (self.calls == 1 || self.cycle, text) {
                (true, Some(text)) => {
                    text[..self.text.len()].copy_from_slice(&self.text);
                    self.phonemes[..2].copy_from_slice(b"i\0");
                    Ok(0x20000000)
                }
                _ => {
                    self.phonemes[..self.child.len()].copy_from_slice(&self.child);
                    Ok(0x80000000)
                }
            }
        }
        fn snapshot(&self, p: &mut [u8; 200]) -> Result<(), Error> {
            *p = self.phonemes;
            Ok(())
        }
        fn publish(&mut self, p: &[u8; 200]) -> Result<(), Error> {
            let n = p.iter().position(|b| *b == 0).ok_or(Error::Phonemes)?;
            self.phonemes[..=n].copy_from_slice(&p[..=n]);
            self.published += 1;
            Ok(())
        }
    }
    fn expand(f: &mut Fixture) -> Result<u32, Error> {
        translate(f, Some(&mut [0; TEXT]), 0)
    }
    #[test]
    fn segmentation_splits_hyphens_and_ideographs_without_changing_ascii_marks() {
        let mut f = Fixture::new("");
        let mut out = [0x55; 100];
        segment(&f, "Abc-Def cai3hong2 汉字\0".as_bytes(), &mut out).unwrap();
        let n = out.iter().position(|b| *b == 0).unwrap();
        assert_eq!(&out[..n], "Abc Def cai3hong2 汉字".as_bytes());
        f.fields[Field::Ideographs as usize] = 1;
        segment(&f, "汉字 A\0".as_bytes(), &mut out).unwrap();
        assert_eq!(&out[..9], "汉 字 A".as_bytes());
        f.fields[Field::Ideographs as usize] = 0;
        f.fields[Field::Alternate as usize] = 0x3100;
        assert!(split(&f, '汉' as u32, 'A' as u32));
    }
    #[test]
    fn segmentation_reserves_two_bytes_and_preserves_untouched_tail() {
        let f = Fixture::new("");
        let mut out = [0x55; 6];
        assert_eq!(segment(&f, b"abcde\0", &mut out[..4]), Ok(2));
        assert_eq!(&out, b"ab\0UUU");
        assert_eq!(segment(&f, b"a-\0", &mut out[..1]), Ok(0));
        assert_eq!(out[0], 0);
        assert_eq!(segment(&f, b"abc", &mut [0; 10]), Err(Error::Source));
    }
    #[test]
    fn expansion_lowers_same_width_and_keeps_nested_flags_and_skip_scope() {
        let mut f = Fixture::new("ABC DEF ");
        f.fields[Field::Skip as usize] = 7;
        assert_eq!(expand(&mut f), Ok(0x20000000));
        assert_eq!(f.keys, vec![b"aBC".to_vec(), b"dEF".to_vec()]);
        assert_eq!(&f.phonemes[..4], b"p\x0fp\0");
        assert_eq!(f.phonemes[4], 0x55);
        assert_eq!(f.fields[Field::Skip as usize], 7);
        assert_ne!(f.fields[Field::RowFlags as usize] & 2, 0);
    }
    #[test]
    fn width_changing_case_mapping_leaves_source_bytes_intact() {
        let mut f = Fixture::new("ABC ");
        f.case_width = true;
        expand(&mut f).unwrap();
        assert_eq!(f.keys, vec![b"ABC".to_vec()]);
    }
    #[test]
    fn cyclic_text_replacements_stop_after_four_expansion_levels() {
        let mut f = Fixture::new("A B ");
        f.cycle = true;
        expand(&mut f).unwrap();
        assert_eq!(f.calls, 31);
        assert_eq!(f.phonemes.iter().position(|b| *b == 0), Some(31));
    }
    #[test]
    fn nested_phrase_mutation_changes_later_translation_input() {
        let mut f = Fixture::new("A B ");
        f.mutate = true;
        expand(&mut f).unwrap();
        assert_eq!(f.keys, vec![b"a".to_vec(), b"x".to_vec()]);
    }
    #[test]
    fn missing_final_space_and_excess_skips_stop_at_nul() {
        let mut f = Fixture::new("word");
        f.skip = 100;
        f.fields[Field::Skip as usize] = 3;
        expand(&mut f).unwrap();
        assert_eq!(f.calls, 2);
        assert_eq!(f.fields[Field::Skip as usize], 3);
        assert_eq!(&f.phonemes[..2], b"p\0");
    }
    #[test]
    fn truncated_join_retains_bounded_bytes_and_still_restores_skip() {
        let mut f = Fixture::new("a b c ");
        f.child = vec![b'p'; 199];
        f.child.push(0);
        f.fields[Field::Skip as usize] = 4;
        expand(&mut f).unwrap();
        assert_eq!(f.calls, 2);
        assert_eq!(f.phonemes[199], 0);
        assert_eq!(f.fields[Field::Skip as usize], 4);
        let mut f = Fixture::new("a b c ");
        f.child = vec![b'p'; 100];
        f.child.push(0);
        expand(&mut f).unwrap();
        assert_eq!(f.calls, 3);
        assert_eq!(f.phonemes[100], 15);
        assert_eq!(f.phonemes[199], 0);
    }
    #[test]
    fn failing_nested_translation_retains_effects_without_replay_and_restores_skip() {
        let mut f = Fixture::new("A B ");
        f.fail = true;
        f.skip = 11;
        f.fields[Field::Skip as usize] = 7;
        assert_eq!(expand(&mut f), Err(Error::State));
        assert_eq!(f.calls, 2);
        assert_eq!(f.published, 0);
        assert_eq!(f.fields[Field::Skip as usize], 7);
        assert_eq!(&f.phonemes[..2], b"E\0");
    }
    #[test]
    fn empty_replacement_and_empty_pronunciation_do_not_publish() {
        let mut f = Fixture::new("");
        expand(&mut f).unwrap();
        assert_eq!(f.calls, 1);
        assert_eq!(f.published, 0);
        assert_eq!(&f.phonemes[..2], b"i\0");
        let mut f = Fixture::new("a ");
        f.child = vec![0];
        expand(&mut f).unwrap();
        assert_eq!(f.published, 0);
    }
    #[test]
    fn invalid_skip_and_negative_depth_refuse_without_spin() {
        let mut f = Fixture::new("A B ");
        f.skip = -1;
        f.fields[Field::Skip as usize] = 3;
        assert_eq!(expand(&mut f), Err(Error::State));
        assert_eq!(f.calls, 2);
        assert_eq!(f.fields[Field::Skip as usize], 3);
        assert_eq!(translate(&mut f, None, -1), Err(Error::State));
        assert_eq!(f.calls, 2);
    }
}
