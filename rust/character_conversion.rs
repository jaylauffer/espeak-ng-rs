//! Complete replacement-character, language conversion and uppercase policy.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{language, translate_word::Error, unicode, utf8};

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    Language,
    Ignore,
    ReplacementLength,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Class {
    Upper,
    Alpha,
    Space,
    Vowel,
}
pub trait Host {
    fn value(&self, field: Field) -> i32;
    fn ignore(&mut self, count: i32) -> Result<(), Error>;
    /// kind 0 is the 840-byte clause source; kind 1 the validated dictionary
    /// replacement span. No foreign slice is borrowed across a callback.
    fn byte(&self, kind: u32, index: usize) -> Option<u8>;
    fn write(&mut self, index: usize, value: u8) -> Result<(), Error>;
    fn classify(&self, code: u32, class: Class) -> bool;
    fn lower(&self, code: u32) -> u32;
    /// Numeric offsets into the replacement owner; initialized terminated
    /// strings have been checked before the optional retained trace primitive.
    fn trace(&mut self, from: usize, to: usize) -> Result<(), Error>;
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Call {
    pub index: usize,
    pub previous: u32,
    pub code: u32,
    pub next: u32,
    pub insert: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultCode {
    pub code: u32,
    pub insert: u32,
    pub flags: u32,
}
const SOURCE: usize = 840;
const MAX_DICTIONARY: usize = 134_217_728;
const REPLACED: u32 = 0x200000;

fn byte(h: &impl Host, kind: u32, index: usize, limit: usize) -> Result<u8, Error> {
    if index >= limit {
        return Err(Error::Source);
    }
    h.byte(kind, index).ok_or(Error::Source)
}
fn decode(
    h: &impl Host,
    kind: u32,
    mut index: usize,
    limit: usize,
) -> Result<utf8::Character, Error> {
    while byte(h, kind, index, limit)? & 0xc0 == 0x80 {
        index = index.checked_add(1).ok_or(Error::Source)?;
    }
    utf8::head(|n| {
        index
            .checked_add(n)
            .and_then(|i| byte(h, kind, i, limit).ok())
    })
    .map_err(|_| Error::Source)
}
fn end(h: &impl Host, mut index: usize, limit: usize) -> Result<usize, Error> {
    while byte(h, 1, index, limit)? != 0 {
        index = index.checked_add(1).ok_or(Error::Source)?;
    }
    Ok(index)
}
#[derive(Debug, PartialEq, Eq)]
pub struct Match {
    pub from: usize,
    pub to: usize,
    pub ignored: i32,
}
/// Preserve ordered matching, including consumption of the entire candidate
/// after a mismatch and same-width permissive UTF-8 pointer advancement.
pub fn find(h: &impl Host, code: u32, next: usize, limit: usize) -> Result<Option<Match>, Error> {
    if limit > MAX_DICTIONARY {
        return Err(Error::Source);
    }
    let mut from = 0usize;
    loop {
        let mut zero = true;
        for n in 0..4 {
            zero &= byte(h, 1, from.checked_add(n).ok_or(Error::Source)?, limit)? == 0;
        }
        if zero {
            return Ok(None);
        }
        let start = from;
        let first = decode(h, 1, from, limit)?;
        from = from.checked_add(first.width).ok_or(Error::Source)?;
        if first.code == code {
            let mut matched = true;
            let mut ignored = 0i32;
            let mut source = next;
            while byte(h, 1, from, limit)? != 0 {
                let pattern = decode(h, 1, from, limit)?;
                from = from.checked_add(pattern.width).ok_or(Error::Source)?;
                let current = decode(h, 0, source, SOURCE)?;
                source = source.checked_add(current.width).ok_or(Error::Source)?;
                if h.lower(current.code) != pattern.code {
                    matched = false;
                } else {
                    ignored = ignored.checked_add(1).ok_or(Error::Capacity)?;
                }
            }
            if matched {
                return Ok(Some(Match {
                    from: start,
                    to: from + 1,
                    ignored,
                }));
            }
        }
        from = end(h, from, limit)?.checked_add(1).ok_or(Error::Source)?;
        from = end(h, from, limit)?.checked_add(1).ok_or(Error::Source)?;
    }
}
pub fn substitute(h: &mut impl Host, call: Call) -> Result<ResultCode, Error> {
    let mut result = ResultCode {
        code: call.code,
        insert: call.insert,
        flags: call.flags,
    };
    let ignored = h.value(Field::Ignore);
    if ignored < 0 {
        return Err(Error::State);
    }
    if ignored > 0 {
        h.ignore(ignored - 1)?;
        result.code = 8;
        return Ok(result);
    }
    if call.code == 0 {
        return Ok(result);
    }
    let limit = usize::try_from(h.value(Field::ReplacementLength)).map_err(|_| Error::Source)?;
    if limit == 0 {
        return Ok(result);
    }
    let upper = h.classify(call.code, Class::Upper);
    let code = if upper { h.lower(call.code) } else { call.code };
    let Some(matched) = find(h, code, call.index, limit)? else {
        return Ok(result);
    };
    h.ignore(matched.ignored)?;
    end(h, matched.to, limit)?;
    h.trace(matched.from, matched.to)?;
    let first = decode(h, 1, matched.to, limit)?;
    let next = matched.to.checked_add(first.width).ok_or(Error::Source)?;
    if byte(h, 1, next, limit)? != 0 {
        let second = decode(h, 1, next, limit)?.code;
        result.insert = if upper && h.classify(call.next, Class::Upper) {
            unicode::to_upper(second)
        } else {
            second
        };
    }
    result.code = if upper {
        unicode::to_upper(first.code)
    } else {
        first.code
    };
    result.flags |= REPLACED;
    Ok(result)
}
pub fn translate(h: &mut impl Host, call: Call) -> Result<ResultCode, Error> {
    let mut result = ResultCode {
        code: call.code,
        insert: call.insert,
        flags: call.flags,
    };
    if (0xac00..=0xd7af).contains(&call.code) {
        let code = call.code - 0xac00;
        let initial = code / 28 / 21;
        let medial = code / 28 % 21;
        let final_code = code % 28;
        if initial == 11 {
            result.code = medial + 0x1161;
            if final_code > 0 {
                result.insert = final_code + 0x11a7;
            }
        } else {
            result.code = initial + 0x1100;
            result.insert = 11 * 28 * 21 + medial * 28 + final_code + 0xac00;
        }
        return Ok(result);
    }
    if (0x3130..0x3164).contains(&call.code) {
        const JAMO: [u8; 52] = [
            0, 0, 1, 0xaa, 2, 0xac, 0xad, 3, 4, 5, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb4, 0xb6, 6, 7,
            8, 0xb9, 9, 10, 0xbc, 12, 13, 14, 15, 16, 17, 18, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66,
            0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75,
        ];
        result.code = u32::from(JAMO[(call.code - 0x3130) as usize]) + 0x1100;
        return Ok(result);
    }
    let lang = h.value(Field::Language) as u32;
    if matches!(lang, n if n == language::code(b"af") || n == language::code(b"nl"))
        && call.code == 39
        && !h.classify(call.previous, Class::Alpha)
    {
        let next = call.index.checked_add(1).ok_or(Error::Source)?;
        if h.classify(decode(h, 0, next, SOURCE)?.code, Class::Space) {
            if call.next == 110 && h.value(Field::Language) as u32 == language::code(b"af") {
                h.write(call.index, 32)?;
                result.code = 0x259;
                return Ok(result);
            }
            if matches!(call.next, 110 | 116) {
                result.code = 0x259;
                return Ok(result);
            }
        }
    }
    substitute(h, call)
}
pub fn upper_in_word(h: &impl Host, before: &[u8; 3], code: u32) -> bool {
    if h.value(Field::Language) as u32 != language::code(b"ga") {
        return false;
    }
    for pattern in [
        b"bp".as_slice(),
        b"bhf",
        b"dt",
        b"gc",
        b"hA",
        b"mb",
        b"nd",
        b"ng",
        b"ts",
        b"tA",
        b"nA",
    ] {
        let length = pattern.len();
        if before[3 - length] == 32 && before[4 - length..] == pattern[..length - 1] {
            let last = u32::from(pattern[length - 1]);
            if code == last || (last == 65 && h.classify(code, Class::Vowel)) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        lang: u32,
        ignored: i32,
        data: Vec<u8>,
        source: [u8; 840],
        traces: Vec<(usize, usize)>,
        fail_trace: bool,
    }
    impl Fixture {
        fn new(data: &[u8]) -> Self {
            Self {
                lang: language::code(b"en"),
                ignored: 0,
                data: data.into(),
                source: [0; 840],
                traces: Vec::new(),
                fail_trace: false,
            }
        }
    }
    impl Host for Fixture {
        fn value(&self, f: Field) -> i32 {
            match f {
                Field::Language => self.lang as i32,
                Field::Ignore => self.ignored,
                Field::ReplacementLength => self.data.len() as i32,
            }
        }
        fn ignore(&mut self, n: i32) -> Result<(), Error> {
            self.ignored = n;
            Ok(())
        }
        fn byte(&self, k: u32, i: usize) -> Option<u8> {
            match k {
                0 => self.source.get(i).copied(),
                1 => self.data.get(i).copied(),
                _ => None,
            }
        }
        fn write(&mut self, i: usize, v: u8) -> Result<(), Error> {
            *self.source.get_mut(i).ok_or(Error::Source)? = v;
            Ok(())
        }
        fn classify(&self, c: u32, k: Class) -> bool {
            match k {
                Class::Upper => unicode::is_upper(c),
                Class::Alpha => unicode::is_alpha(c),
                Class::Space => matches!(c, 9..=13 | 32),
                Class::Vowel => matches!(c, 65 | 69 | 73 | 79 | 85),
            }
        }
        fn lower(&self, c: u32) -> u32 {
            unicode::to_lower(c)
        }
        fn trace(&mut self, a: usize, b: usize) -> Result<(), Error> {
            self.traces.push((a, b));
            if self.fail_trace {
                Err(Error::State)
            } else {
                Ok(())
            }
        }
    }
    fn call(code: u32) -> Call {
        Call {
            index: 0,
            previous: 32,
            code,
            next: 32,
            insert: 91,
            flags: 3,
        }
    }
    #[test]
    fn ordered_multi_character_replacement_preserves_case_insert_and_consumption() {
        let mut h = Fixture::new(b"abc\0xyz\0ab\0q\0a\0u\0\0\0\0\0");
        h.source[..3].copy_from_slice(b"BC ");
        let mut c = call(65);
        c.next = 66;
        assert_eq!(
            translate(&mut h, c),
            Ok(ResultCode {
                code: 88,
                insert: 89,
                flags: REPLACED | 3
            })
        );
        assert_eq!(h.ignored, 2);
        assert_eq!(h.traces, [(0, 4)]);
        assert_eq!(translate(&mut h, call(66)).unwrap().code, 8);
        assert_eq!(translate(&mut h, call(67)).unwrap().code, 8);
        assert_eq!(h.ignored, 0);
        assert_eq!(translate(&mut h, call(100)).unwrap().code, 100);
    }
    #[test]
    fn mismatch_scans_full_candidate_then_uses_later_rule() {
        let mut h = Fixture::new(b"abc\0x\0ab\0z\0\0\0\0\0");
        h.source[..3].copy_from_slice(b"bd ");
        assert_eq!(translate(&mut h, call(97)).unwrap().code, 122);
        assert_eq!(h.ignored, 1);
        assert_eq!(h.traces, [(6, 9)]);
    }
    #[test]
    fn hangul_paths_bypass_replacement_consumption_and_preserve_existing_insert() {
        let mut h = Fixture::new(b"a\0x\0\0\0\0\0");
        h.ignored = 2;
        for c in 0xac00..=0xd7af {
            let r = translate(&mut h, call(c)).unwrap();
            let n = c - 0xac00;
            if n / 28 / 21 == 11 {
                assert_eq!(r.code, n / 28 % 21 + 0x1161);
                assert_eq!(r.insert, if n % 28 > 0 { n % 28 + 0x11a7 } else { 91 });
            } else {
                assert_eq!(r.code, n / 28 / 21 + 0x1100);
                assert_eq!(r.insert, 0xac00 + 11 * 28 * 21 + n % 588);
            }
            assert_eq!(r.flags, 3);
            assert_eq!(h.ignored, 2);
        }
        assert_eq!(translate(&mut h, call(0x3133)).unwrap().code, 0x11aa);
    }
    #[test]
    fn apostrophe_language_paths_use_locale_alpha_and_mutate_only_af_n() {
        let mut h = Fixture::new(&[]);
        h.lang = language::code(b"af");
        h.source[..3].copy_from_slice(b"n \0");
        let mut c = call(39);
        c.next = 110;
        assert_eq!(translate(&mut h, c).unwrap().code, 0x259);
        assert_eq!(h.source[0], 32);
        h.lang = language::code(b"nl");
        h.source[0] = 116;
        c.next = 116;
        assert_eq!(translate(&mut h, c).unwrap().code, 0x259);
        assert_eq!(h.source[0], 116);
        c.previous = 65;
        assert_eq!(translate(&mut h, c).unwrap().code, 39);
    }
    #[test]
    fn irish_prefixes_use_three_owned_predecessors_and_vowel_classification() {
        let mut h = Fixture::new(&[]);
        h.lang = language::code(b"ga");
        assert!(upper_in_word(&h, b" bh", 102));
        assert!(upper_in_word(&h, b"x h", 69));
        assert!(!upper_in_word(&h, b"abh", 102));
        assert!(!upper_in_word(&h, b"x h", 66));
        h.lang = language::code(b"en");
        assert!(!upper_in_word(&h, b" bh", 102));
    }
    #[test]
    fn failed_trace_retains_executed_consumption_without_replay() {
        let mut h = Fixture::new(b"abc\0x\0\0\0\0\0");
        h.source[..2].copy_from_slice(b"bc");
        h.fail_trace = true;
        assert_eq!(translate(&mut h, call(97)), Err(Error::State));
        assert_eq!(h.ignored, 2);
        assert_eq!(h.traces, [(0, 4)]);
    }
    #[test]
    fn missing_terminators_and_infinite_continuation_bytes_stop_at_owner_bounds() {
        let mut h = Fixture::new(b"a\0x");
        assert_eq!(translate(&mut h, call(97)), Err(Error::Source));
        assert!(h.traces.is_empty());
        h = Fixture::new(b"ab\0x\0\0\0\0\0");
        h.source.fill(0x80);
        assert_eq!(translate(&mut h, call(97)), Err(Error::Source));
        assert_eq!(h.ignored, 0);
        h.ignored = -1;
        assert_eq!(translate(&mut h, call(97)), Err(Error::State));
    }
}
