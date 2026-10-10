//! Complete individual-letter spelling and plural-suffix control.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::translate_word::{Error, Source};
pub trait Host {
    fn byte(&self, source: Source) -> Option<u8>;
    /// Operate on fresh live pronunciation; retain no foreign source loan.
    fn letter(&mut self, source: Source, control: u32) -> Result<usize, Error>;
    fn leading(&self) -> u8;
    fn copy_switch(&mut self) -> Result<(), Error>;
    fn stress(&mut self, mode: i32, count: i32) -> Result<(), Error>;
    /// Owned initialized source, scoped to one rule call on fresh output.
    fn plural(&mut self, source: &mut [u8; 5]) -> Result<(), Error>;
}
pub fn spell(host: &mut impl Host, mode: i32) -> Result<Option<Source>, Error> {
    let mut cursor = Source::default();
    let mut count = 0i32;
    let capitals = (u32::from(mode > 2) * 2) | (u32::from(mode > 1) * 4);
    loop {
        let byte = host.byte(cursor).ok_or(Error::Source)?;
        if matches!(byte, 0 | b' ') {
            break;
        }
        let consumed = host.letter(cursor, capitals | u32::from(count > 0))?;
        cursor = cursor.advance(isize::try_from(consumed).map_err(|_| Error::Source)?)?;
        count = count.checked_add(1).ok_or(Error::State)?;
        if host.leading() == 21 {
            host.copy_switch()?;
            return Ok(None);
        }
        // A language switch legitimately consumes zero and terminates control.
        // Refuse zero progress only when spelling would otherwise continue.
        if consumed == 0 {
            return Err(Error::Source);
        }
    }
    host.stress(mode, count)?;
    Ok(Some(cursor))
}
pub fn plural(host: &mut impl Host, flags: u32, last: u8) -> Result<(), Error> {
    if flags & 4 == 0 {
        return Ok(());
    }
    let letters = match last {
        b'f' => *b"ss",
        b'h' | b's' | b'x' => *b"iz",
        _ => *b"zz",
    };
    host.plural(&mut [0, b' ', letters[0], letters[1], 0])
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        source: Vec<u8>,
        calls: Vec<(isize, u32)>,
        output: Vec<u8>,
        copied: Vec<u8>,
        stress: Option<(i32, i32)>,
        plural: Option<[u8; 5]>,
        switch: bool,
        consumed: Option<usize>,
        fail: bool,
    }
    impl Fixture {
        fn new(text: &[u8]) -> Self {
            Self {
                source: text.into(),
                calls: vec![],
                output: b"owner\0".into(),
                copied: vec![],
                stress: None,
                plural: None,
                switch: false,
                consumed: None,
                fail: false,
            }
        }
    }
    impl Host for Fixture {
        fn byte(&self, source: Source) -> Option<u8> {
            self.source
                .get(usize::try_from(source.offset).ok()?)
                .copied()
        }
        fn letter(&mut self, source: Source, control: u32) -> Result<usize, Error> {
            self.calls.push((source.offset, control));
            self.output = if self.switch {
                b"\x15fr\0".into()
            } else {
                b"nested\0".into()
            };
            if self.fail {
                return Err(Error::State);
            }
            if let Some(n) = self.consumed {
                return Ok(n);
            }
            let text = std::str::from_utf8(&self.source[source.offset as usize..]).unwrap();
            Ok(text.chars().next().unwrap().len_utf8())
        }
        fn leading(&self) -> u8 {
            self.output[0]
        }
        fn copy_switch(&mut self) -> Result<(), Error> {
            self.copied.clone_from(&self.output);
            Ok(())
        }
        fn stress(&mut self, mode: i32, count: i32) -> Result<(), Error> {
            self.stress = Some((mode, count));
            assert!(count == 0 || self.output == b"nested\0");
            Ok(())
        }
        fn plural(&mut self, source: &mut [u8; 5]) -> Result<(), Error> {
            self.plural = Some(*source);
            self.output = b"rule-effect\0".into();
            source[1] = b'?';
            if self.fail {
                Err(Error::State)
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn spelling_modes_count_characters_and_use_fresh_output() {
        for mode in [-3, 0, 1, 2, 3, i32::MAX] {
            let mut f = Fixture::new("aé𐐀 tail".as_bytes());
            assert_eq!(spell(&mut f, mode), Ok(Some(Source { slot: 0, offset: 7 })));
            let bits = if mode > 2 {
                6
            } else if mode > 1 {
                4
            } else {
                0
            };
            assert_eq!(f.calls, [(0, bits), (1, bits | 1), (3, bits | 1)]);
            assert_eq!(f.stress, Some((mode, 3)));
        }
    }
    #[test]
    fn empty_spelling_still_applies_zero_character_stress() {
        for source in [b"\0".as_slice(), b" tail".as_slice()] {
            let mut f = Fixture::new(source);
            assert_eq!(spell(&mut f, 2), Ok(Some(Source::default())));
            assert!(f.calls.is_empty());
            assert_eq!(f.stress, Some((2, 0)));
        }
    }
    #[test]
    fn language_switch_copies_nested_output_and_skips_stress() {
        for consumed in [0, 1] {
            let mut f = Fixture::new(b"abc ");
            f.switch = true;
            f.consumed = Some(consumed);
            assert_eq!(spell(&mut f, 3), Ok(None));
            assert_eq!(f.copied, b"\x15fr\0");
            assert_eq!(f.calls, [(0, 6)]);
            assert_eq!(f.stress, None);
        }
    }
    #[test]
    fn failed_or_nonprogressing_letter_retains_effect_without_retry() {
        for n in [0, usize::MAX] {
            let mut f = Fixture::new(b"abc ");
            f.consumed = Some(n);
            assert_eq!(spell(&mut f, 0), Err(Error::Source));
            assert_eq!(f.calls.len(), 1);
            assert_eq!(f.output, b"nested\0");
            assert_eq!(f.stress, None);
        }
        let mut f = Fixture::new(b"abc ");
        f.fail = true;
        assert_eq!(spell(&mut f, 0), Err(Error::State));
        assert_eq!(f.calls.len(), 1);
    }
    #[test]
    fn missing_source_terminator_refuses_after_retaining_letter_effect() {
        let mut f = Fixture::new(b"a");
        assert_eq!(spell(&mut f, 0), Err(Error::Source));
        assert_eq!(f.calls, [(0, 0)]);
        assert_eq!(f.stress, None);
    }
    #[test]
    fn plural_selection_preserves_all_byte_values_and_flag_admission() {
        for last in 0..=u8::MAX {
            let mut f = Fixture::new(b"");
            plural(&mut f, 0xffff_fffb, last).unwrap();
            assert_eq!(f.plural, None);
            plural(&mut f, 4, last).unwrap();
            let letters = match last {
                b'f' => *b"ss",
                b'h' | b's' | b'x' => *b"iz",
                _ => *b"zz",
            };
            assert_eq!(f.plural, Some([0, b' ', letters[0], letters[1], 0]));
            assert_eq!(f.output, b"rule-effect\0");
        }
    }
    #[test]
    fn failed_plural_keeps_primitive_effects() {
        let mut f = Fixture::new(b"");
        f.fail = true;
        assert_eq!(plural(&mut f, 4, b'f'), Err(Error::State));
        assert_eq!(f.output, b"rule-effect\0");
        assert_eq!(f.plural, Some(*b"\0 ss\0"));
    }
}
