//! Complete dotted-abbreviation and unpronounceable-word admission policies.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    common_text, language,
    translate_word::{Error, Source, PHONEMES},
    utf8,
};

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    AlphabetOffset,
    Mode,
    MaxInitialConsonants,
}
/// Sources and scalar state remain with serialized live owners. No foreign
/// source/state/output loan crosses a primitive call; probe scratch is owned.
pub trait Host {
    fn byte(&self, source: Source) -> Option<u8>;
    fn write(&mut self, source: Source, value: u8) -> Result<(), Error>;
    fn value(&self, field: Field) -> i32;
    fn locale_alpha(&self, code: u32) -> bool;
    fn vowel(&mut self, code: u32) -> bool;
    fn probe(&mut self, scratch: &mut [u8; PHONEMES]) -> Result<i32, Error>;
    fn skip_words(&mut self, value: i32);
}
fn byte(host: &impl Host, source: Source) -> Result<u8, Error> {
    host.byte(source).ok_or(Error::Source)
}
fn code(host: &impl Host, mut source: Source) -> Result<utf8::Character, Error> {
    while byte(host, source)? & 0xc0 == 0x80 {
        source = source.advance(1)?;
    }
    utf8::head(|i| source.advance(i as isize).ok().and_then(|s| host.byte(s)))
        .map_err(|_| Error::Source)
}

/// Compact valid dotted sequences only after the complete 80-byte plan fits.
/// Zero/one-letter results and capacity refusal preserve source and skip count.
pub fn dotted(host: &mut impl Host) -> Result<i32, Error> {
    let start = Source::default();
    let mut cursor = start;
    let mut scratch = [0u8; 80];
    let mut used = 0usize;
    let mut count = 0i32;
    loop {
        let ch = code(host, cursor)?;
        let mut width = ch.width;
        let mut finish = false;
        let after = cursor.advance(width as isize)?;
        let mut accepted = false;
        if byte(host, after)? == b' ' && common_text::word_alpha(ch.code, |c| host.locale_alpha(c))
        {
            if byte(host, after.advance(1)?)? == b'.' {
                let following = byte(host, after.advance(2)?)?;
                if following == b' ' {
                    accepted = true;
                } else if following == b'\'' && byte(host, after.advance(3)?)? == b's' {
                    width += 2;
                    accepted = true;
                    finish = true;
                }
            } else if count > 0 {
                accepted = true;
                finish = true;
            }
        }
        if !accepted {
            break;
        }
        if width == 0 || width > scratch.len() - used {
            return Ok(0);
        }
        for slot in &mut scratch[used..used + width] {
            *slot = byte(host, cursor)?;
            cursor = cursor.advance(1)?;
        }
        used += width;
        count = count.checked_add(1).ok_or(Error::State)?;
        if finish {
            break;
        }
        cursor = cursor.advance(3)?;
    }
    if count > 1 {
        for (i, value) in scratch[..used].iter().enumerate() {
            host.write(start.advance(i as isize)?, *value)?;
        }
        let end = usize::try_from(cursor.offset).map_err(|_| Error::Source)?;
        for i in used..end {
            host.write(start.advance(i as isize)?, b' ')?;
        }
        host.skip_words((count - 1).checked_mul(2).ok_or(Error::State)?);
    }
    Ok(count)
}

fn probe(host: &mut impl Host) -> Result<bool, Error> {
    let preceding = Source::default().advance(-1)?;
    let saved = byte(host, preceding)?;
    host.write(preceding, b' ')?;
    let mut scratch = [0; PHONEMES];
    let result = host.probe(&mut scratch);
    // Restore this scoped patch even on a primitive failure. Other executed
    // source/output/state effects remain; a failure never replays the C policy.
    let restored = host.write(preceding, saved);
    let flags = result?;
    restored?;
    Ok(flags == 0 || flags & 0x8000 != 0)
}
pub fn unpronounceable(host: &mut impl Host, position: i32) -> Result<bool, Error> {
    let start = Source::default();
    let first = code(host, start)?.code;
    let offset = host.value(Field::AlphabetOffset);
    if offset > 0 && first < 0x241 {
        return Ok(false);
    }
    if language::alphabet_from_char(first as i32).is_some_and(|a| a.offset != offset) {
        return Ok(false);
    }
    if host.value(Field::Mode) == 1 || matches!(byte(host, start)?, 0 | b' ' | b'\'') {
        return Ok(false);
    }
    let mut cursor = start;
    let mut count = 0i32;
    let mut initial = 0;
    let mut vowel_position = 9i32;
    loop {
        let ch = code(host, cursor)?;
        cursor = cursor.advance(ch.width as isize)?;
        if matches!(ch.code, 0 | 32) {
            break;
        }
        if ch.code == 39 && (count > 1 || position > 0) {
            break;
        }
        if count == 0 {
            initial = ch.code;
        }
        if !(ch.code == 39 && host.value(Field::Mode) == 3) {
            count = count.checked_add(1).ok_or(Error::State)?;
        }
        if host.vowel(ch.code) {
            vowel_position = count;
            break;
        }
        if ch.code != 39 && !host.locale_alpha(ch.code) {
            return Ok(false);
        }
    }
    if vowel_position > 2 && host.value(Field::Mode) == 2 {
        return probe(host);
    }
    if initial as i32 == host.value(Field::Mode) {
        vowel_position -= 1;
    }
    let limit = host
        .value(Field::MaxInitialConsonants)
        .checked_add(1)
        .ok_or(Error::State)?;
    Ok(vowel_position > limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        text: Vec<u8>,
        offset: i32,
        mode: i32,
        maximum: i32,
        skip: i32,
        result: Result<i32, Error>,
        probes: usize,
        effect: bool,
        fresh_mode: Option<i32>,
    }
    impl Fixture {
        fn new(text: &[u8]) -> Self {
            let mut source = vec![b'!'];
            source.extend_from_slice(text);
            source.extend_from_slice(b"\0\0");
            Self {
                text: source,
                offset: 0,
                mode: 0,
                maximum: 2,
                skip: 7,
                result: Ok(0),
                probes: 0,
                effect: false,
                fresh_mode: None,
            }
        }
    }
    impl Host for Fixture {
        fn byte(&self, s: Source) -> Option<u8> {
            if s.slot != 0 {
                return None;
            }
            self.text
                .get(usize::try_from(s.offset.checked_add(1)?).ok()?)
                .copied()
        }
        fn write(&mut self, s: Source, b: u8) -> Result<(), Error> {
            let i = usize::try_from(s.offset.checked_add(1).ok_or(Error::Source)?)
                .map_err(|_| Error::Source)?;
            *self.text.get_mut(i).ok_or(Error::Source)? = b;
            Ok(())
        }
        fn value(&self, f: Field) -> i32 {
            match f {
                Field::AlphabetOffset => self.offset,
                Field::Mode => self.mode,
                Field::MaxInitialConsonants => self.maximum,
            }
        }
        fn locale_alpha(&self, c: u32) -> bool {
            char::from_u32(c).is_some_and(|c| c.is_alphabetic())
        }
        fn vowel(&mut self, c: u32) -> bool {
            if let Some(mode) = self.fresh_mode.take() {
                self.mode = mode;
            }
            matches!(c, 97 | 101 | 105 | 111 | 117)
        }
        fn probe(&mut self, _: &mut [u8; PHONEMES]) -> Result<i32, Error> {
            assert_eq!(self.text[0], b' ');
            self.probes += 1;
            self.effect = true;
            self.text[1] = b'X';
            self.result
        }
        fn skip_words(&mut self, count: i32) {
            self.skip = count;
        }
    }
    #[test]
    fn dotted_sequence_compacts_and_pads_without_changing_tail() {
        let mut f = Fixture::new(b"a . b . c next ");
        let tail = f.text[10..].to_vec();
        assert_eq!(dotted(&mut f), Ok(3));
        assert_eq!(&f.text[1..11], b"abc       ");
        assert_eq!(f.skip, 4);
        assert_eq!(f.text[11..], tail[1..]);
    }
    #[test]
    fn single_letter_and_overflow_leave_source_and_skip_unchanged() {
        for text in [b"a . next ".to_vec(), b"a . ".repeat(81)] {
            let mut f = Fixture::new(&text);
            let old = f.text.clone();
            let value = dotted(&mut f).unwrap();
            assert!(value <= 1);
            assert_eq!(f.text, old);
            assert_eq!(f.skip, 7);
        }
    }
    #[test]
    fn multibyte_and_possessive_compaction_preserve_legacy_byte_copy() {
        let mut f = Fixture::new("é . b .'s next ".as_bytes());
        assert_eq!(dotted(&mut f), Ok(2));
        assert_eq!(&f.text[1..8], "éb .  ".as_bytes());
        assert_eq!(f.skip, 2);
    }
    #[test]
    fn alphabet_and_disabled_admission_skip_the_probe() {
        let mut f = Fixture::new(b"bcdf ");
        f.offset = 0x400;
        assert_eq!(unpronounceable(&mut f, 0), Ok(false));
        assert_eq!(f.probes, 0);
        let mut f = Fixture::new(b"bcdf ");
        f.mode = 1;
        assert_eq!(unpronounceable(&mut f, 0), Ok(false));
    }
    #[test]
    fn vowel_position_and_apostrophe_context_determine_spelling() {
        let mut f = Fixture::new(b"bcdfa ");
        assert_eq!(unpronounceable(&mut f, 0), Ok(true));
        let mut f = Fixture::new(b"bca ");
        assert_eq!(unpronounceable(&mut f, 0), Ok(false));
        let mut f = Fixture::new(b"b'ca ");
        assert_eq!(unpronounceable(&mut f, 1), Ok(true));
    }
    #[test]
    fn rule_probe_restores_predecessor_and_retains_nested_effects() {
        let mut f = Fixture::new(b"bcdf ");
        f.mode = 2;
        f.result = Ok(0x8000);
        assert_eq!(unpronounceable(&mut f, 0), Ok(true));
        assert_eq!(f.text[0], b'!');
        assert_eq!(f.text[1], b'X');
        assert!(f.effect);
        assert_eq!(f.probes, 1);
    }
    #[test]
    fn failed_probe_restores_its_patch_without_replay() {
        let mut f = Fixture::new(b"bcdf ");
        f.mode = 2;
        f.result = Err(Error::State);
        assert_eq!(unpronounceable(&mut f, 0), Err(Error::State));
        assert_eq!(f.text[0], b'!');
        assert!(f.effect);
        assert_eq!(f.probes, 1);
    }
    #[test]
    fn language_mode_is_fresh_after_a_vowel_primitive() {
        let mut f = Fixture::new(b"bcdf ");
        f.fresh_mode = Some(2);
        assert_eq!(unpronounceable(&mut f, 0), Ok(true));
        assert_eq!(f.probes, 1);
    }
    #[test]
    fn missing_source_and_overflowed_limit_refuse_boundedly() {
        let mut f = Fixture::new(b"abc ");
        f.text.clear();
        assert_eq!(dotted(&mut f), Err(Error::Source));
        let mut f = Fixture::new(b"bcdf ");
        f.maximum = i32::MAX;
        assert_eq!(unpronounceable(&mut f, 0), Err(Error::State));
    }
}
