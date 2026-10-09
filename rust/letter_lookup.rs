//! Native letter/symbol lookup, diacritic assembly and spelling pronunciation.
// Copyright (C) 2005-2015 Jonathan Duddington; 2015-2016, 2020 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_digits::Buffer;
use crate::number_lookup::{Error, PHONEME_BYTES};
use std::fmt::Write;
#[path = "letter_lookup_data.rs"]
mod data;

const BEFORE: u32 = 0x1000;
const NO_TRACE: u32 = 0x10000000;
const ACCENTS: [&[u8]; 22] = [
    b"_lig\0", b"_smc\0", b"_tur\0", b"_rev\0", b"_crl\0", b"_acu\0", b"_brv\0", b"_hac\0",
    b"_ced\0", b"_cir\0", b"_dia\0", b"_ac2\0", b"_dot\0", b"_grv\0", b"_mcn\0", b"_ogo\0",
    b"_rng\0", b"_stk\0", b"_tld\0", b"_bar\0", b"_rfx\0", b"_hok\0",
];
pub trait Host {
    /// Lookup key with initialized predecessors; complete owned 10-byte source
    /// passed mutably to support rule callbacks. Lookup retains no source pointer.
    fn lookup(
        &mut self,
        source: &mut [u8; 10],
        start: usize,
        secondary: bool,
        output: &mut [u8; PHONEME_BYTES],
    ) -> i32;
    fn named(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) -> i32;
    /// Local translator language (0) and current accent policy (1).
    fn value(&self, field: u32) -> i32;
    fn space(&self, code: u32) -> bool;
    /// Owned initialized source; scope/restore rule context, retaining nothing.
    fn rules(
        &mut self,
        source: &mut [u8; 10],
        start: usize,
        capacity: usize,
        flags: u32,
        output: &mut [u8; PHONEME_BYTES],
    );
    /// Fresh voice phoneme-table restoration (true), or secondary setup (false).
    fn select(&mut self, restore: bool);
    fn stress(&mut self, output: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2], control: i32);
}
fn source(code: u32) -> [u8; 10] {
    let mut source = [0; 10];
    source[1] = b'_';
    let (bytes, length) = crate::suffix::encode(code);
    source[2..2 + length].copy_from_slice(&bytes[..length]);
    source[length + 2] = b' ';
    source
}
fn lookup(
    host: &mut impl Host,
    source: &mut [u8; 10],
    start: usize,
    secondary: bool,
    buffer: &mut Buffer,
) -> Result<i32, Error> {
    let mut output = [0; PHONEME_BYTES];
    let flags = host.lookup(source, start, secondary, &mut output);
    buffer.assign(&output)?;
    Ok(flags)
}
fn name(host: &mut impl Host, index: usize, buffer: &mut Buffer) -> Result<i32, Error> {
    let mut output = [0; PHONEME_BYTES];
    let flags = host.named(ACCENTS[index], &mut output);
    buffer.assign(&output)?;
    Ok(flags)
}
fn basic(host: &mut impl Host, code: u32, buffer: &mut Buffer) -> Result<(), Error> {
    let mut source = source(code);
    if lookup(host, &mut source, 1, false, buffer)? == 0 {
        source[1] = b' ';
        if lookup(host, &mut source, 2, false, buffer)? == 0 {
            let mut output = [0; PHONEME_BYTES];
            host.rules(&mut source, 2, 20, 0, &mut output);
            buffer.assign(&output)?;
        }
    }
    Ok(())
}
/// None preserves the existing caller pronunciation, including non-decomposable
/// characters and missing accent/base names. All intermediate buffers respect
/// their original limits; only the final terminated prefix is published.
pub fn accented(
    host: &mut impl Host,
    code: u32,
    output: &mut [u8],
) -> Result<Option<usize>, Error> {
    if output.is_empty() || output.len() > PHONEME_BYTES {
        return Err(Error::Capacity);
    }
    let packed = if (0xe0..0x17f).contains(&code) {
        data::LATIN[(code - 0xe0) as usize]
    } else if (0x250..=0x2a8).contains(&code) {
        data::IPA[(code - 0x250) as usize]
    } else {
        0
    };
    if packed == 0 {
        return Ok(None);
    }
    let mut first = u32::from(packed & 0x3f) + 59;
    if first < u32::from(b'a') {
        first = u32::from(data::NON_ASCII[(first - 59) as usize]);
    }
    let ligature = packed & 0x8000 != 0;
    let accent1 = if ligature {
        0
    } else {
        usize::from((packed >> 6) & 0x1f)
    };
    let accent2 = if ligature {
        usize::from((packed >> 12) & 7)
    } else {
        usize::from((packed >> 11) & 0xf)
    };
    if accent1 == 0 && !ligature {
        return Ok(None);
    }
    let mut name1 = Buffer::new(30);
    let flags1 = name(host, accent1, &mut name1)?;
    if flags1 == 0 {
        return Ok(None);
    }
    let mut letter1 = Buffer::new(30);
    basic(host, first, &mut letter1)?;
    if letter1.bytes().is_empty() {
        return Ok(None);
    }
    let mut name2 = Buffer::new(30);
    let mut result = Buffer::new(output.len());
    if accent2 != 0 && name(host, accent2, &mut name2)? as u32 & BEFORE != 0 {
        result.append(name2.bytes())?;
        name2.clear();
    }
    if ligature {
        let mut letter2 = Buffer::new(30);
        basic(host, u32::from((packed >> 6) & 0x3f) + 59, &mut letter2)?;
        result.append(name1.bytes())?;
        result.append(&[23])?;
        result.append(letter1.bytes())?;
        result.append(&[6])?;
        result.append(letter2.bytes())?;
        result.append(name2.bytes())?;
    } else if accent1 == 0 {
        result.append(letter1.bytes())?;
    } else if host.value(1) & 1 != 0 || flags1 as u32 & BEFORE != 0 {
        result.append(name1.bytes())?;
        result.append(&[23, 6])?;
        result.append(letter1.bytes())?;
    } else {
        result.append(&[4])?;
        result.append(letter1.bytes())?;
        result.append(&[23])?;
        result.append(name1.bytes())?;
        result.append(&[23])?;
    }
    result.publish(output)?;
    Ok(Some(result.bytes().len()))
}
/// Normal text, whitespace names, spelling rules, accent fallback and stress.
/// Errors preserve foreign output; dictionary/source/state effects remain and
/// are not replayed. Restore the live voice table after secondary lookup even
/// when its output is malformed. No foreign output loan crosses callbacks.
pub fn letter(
    host: &mut impl Host,
    code: u32,
    next: i32,
    control: i32,
    output: &mut [u8],
) -> Result<usize, Error> {
    if output.is_empty() || output.len() > PHONEME_BYTES {
        return Err(Error::Capacity);
    }
    let mut word = source(code);
    let (_, width) = crate::suffix::encode(code);
    let mut result = Buffer::new(output.len());
    if next == -1 {
        word[1] = 0;
        if lookup(host, &mut word, 2, false, &mut result)? == 0 {
            word[1] = b'_';
            let mut ignored = Buffer::new(160);
            if lookup(host, &mut word, 1, false, &mut ignored)? == 0 && host.value(0) != 0x656e {
                host.select(false);
                let mut scratch = [0; PHONEME_BYTES];
                let found = host.lookup(&mut word, 2, true, &mut scratch);
                host.select(true);
                ignored.assign(&scratch)?;
                if found != 0 {
                    result.clear();
                    result.append(&[21])?;
                }
            }
        }
    } else if code <= 32 || host.space(code) {
        let mut key = Buffer::new(9);
        write!(key, "_#{} ", code as i32).map_err(|_| Error::Key)?;
        word[1..1 + key.terminated().len()].copy_from_slice(key.terminated());
        lookup(host, &mut word, 1, false, &mut result)?;
    } else {
        word[width + 3] = if next == 32 { b' ' } else { 31 };
        let mut phonemes = Buffer::new(160);
        if lookup(host, &mut word, 1, false, &mut phonemes)? == 0 {
            word[1] = b' ';
            if lookup(host, &mut word, 2, false, &mut phonemes)? == 0 {
                let mut scratch = [0; PHONEME_BYTES];
                host.rules(&mut word, 2, 160, NO_TRACE, &mut scratch);
                phonemes.assign(&scratch)?;
            }
        }
        if phonemes.bytes().is_empty() {
            let mut scratch = [0; PHONEME_BYTES];
            if accented(host, code, &mut scratch[..160])?.is_some() {
                phonemes.assign(&scratch)?;
            }
        }
        result.append(phonemes.bytes())?;
        if result.bytes().first().is_some_and(|b| *b != 21) {
            let mut scratch = [0; PHONEME_BYTES];
            result.publish(&mut scratch)?;
            host.stress(&mut scratch, &mut [0; 2], control & 1);
            result.assign(&scratch)?;
        }
    }
    result.publish(output)?;
    Ok(result.bytes().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Fixture {
        replies: VecDeque<(i32, Vec<u8>)>,
        keys: Vec<Vec<u8>>,
        language: i32,
        accents: i32,
        selections: Vec<bool>,
        stress_calls: Vec<i32>,
        append_stress: bool,
    }
    impl Fixture {
        fn new(replies: &[(i32, &[u8])]) -> Self {
            Self {
                replies: replies.iter().map(|(f, b)| (*f, b.to_vec())).collect(),
                language: 0x6672,
                ..Self::default()
            }
        }
        fn respond(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
            self.keys.push(key.to_vec());
            let (flags, text) = self.replies.pop_front().expect("unexpected lookup");
            out[..text.len()].copy_from_slice(&text);
            flags
        }
    }
    impl Host for Fixture {
        fn lookup(
            &mut self,
            source: &mut [u8; 10],
            start: usize,
            _: bool,
            out: &mut [u8; PHONEME_BYTES],
        ) -> i32 {
            let end = source[start..].iter().position(|b| *b == 0).unwrap() + start;
            self.respond(&source[start..end], out)
        }
        fn named(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
            self.respond(&key[..key.len() - 1], out)
        }
        fn value(&self, field: u32) -> i32 {
            if field == 0 {
                self.language
            } else {
                self.accents
            }
        }
        fn space(&self, _: u32) -> bool {
            false
        }
        fn rules(
            &mut self,
            _: &mut [u8; 10],
            _: usize,
            _: usize,
            _: u32,
            _: &mut [u8; PHONEME_BYTES],
        ) {
            panic!("unexpected rules")
        }
        fn select(&mut self, restore: bool) {
            self.selections.push(restore);
        }
        fn stress(&mut self, out: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2], control: i32) {
            assert_eq!(*flags, [0; 2]);
            self.stress_calls.push(control);
            if self.append_stress {
                let length = out.iter().position(|b| *b == 0).unwrap();
                out[length] = 6;
                out[length + 1] = 0;
            }
        }
    }

    #[test]
    fn accent_keeps_unsigned_phonemes_and_respects_exact_capacity() {
        let mut host = Fixture::new(&[(2, &[0x82, 0]), (2, &[0xff, 0])]);
        let mut out = [0x97; 8];
        assert_eq!(accented(&mut host, 0xe0, &mut out[..6]), Ok(Some(5)));
        assert_eq!(out, [4, 0xff, 23, 0x82, 23, 0, 0x97, 0x97]);
        assert_eq!(host.keys, [b"_grv".to_vec(), b"_a ".to_vec()]);
        let mut host = Fixture::new(&[(2, &[0x82, 0]), (2, &[0xff, 0])]);
        let mut out = [0x97; 5];
        assert_eq!(accented(&mut host, 0xe0, &mut out), Err(Error::Capacity));
        assert_eq!(out, [0x97; 5]);
    }

    #[test]
    fn secondary_modifier_prefixes_only_when_marked_before() {
        for before in [false, true] {
            let mut host = Fixture::new(&[
                (2, &[50, 0]),
                (2, &[51, 0]),
                (if before { BEFORE as i32 } else { 2 }, &[52, 0]),
            ]);
            let mut out = [0x97; 12];
            let size = accented(&mut host, 0x25d, &mut out).unwrap().unwrap();
            let expected: &[u8] = if before {
                &[52, 4, 51, 23, 50, 23, 0]
            } else {
                &[4, 51, 23, 50, 23, 0]
            };
            assert_eq!(&out[..=size], expected);
            assert_eq!(
                host.keys,
                [
                    b"_hok".to_vec(),
                    "_ɛ ".as_bytes().to_vec(),
                    b"_rev".to_vec()
                ]
            );
        }
    }

    #[test]
    fn ligature_keeps_second_letter_and_trailing_modifier() {
        let mut host = Fixture::new(&[(2, &[50, 0]), (2, &[51, 0]), (2, &[52, 0]), (2, &[53, 0])]);
        let mut out = [0x97; 20];
        assert_eq!(accented(&mut host, 0x276, &mut out), Ok(Some(6)));
        assert_eq!(&out[..7], &[50, 23, 51, 6, 53, 52, 0]);
        assert_eq!(
            host.keys,
            [
                b"_lig".to_vec(),
                b"_o ".to_vec(),
                b"_smc".to_vec(),
                b"_e ".to_vec()
            ]
        );
    }

    #[test]
    fn normal_underscore_match_preserves_primary_miss_text() {
        let mut host = Fixture::new(&[(0, &[0x82, 0]), (2, &[0x83, 0])]);
        let mut out = [0x97; 8];
        assert_eq!(letter(&mut host, 'é' as u32, -1, 7, &mut out), Ok(1));
        assert_eq!(&out[..2], &[0x82, 0]);
        assert!(host.selections.is_empty());
        assert!(host.stress_calls.is_empty());
    }

    #[test]
    fn malformed_secondary_lookup_restores_table_before_error() {
        let bad = [31; PHONEME_BYTES];
        let mut host = Fixture::new(&[(0, &[0]), (0, &[0]), (2, &bad)]);
        let mut out = [0x97; 8];
        assert_eq!(
            letter(&mut host, 'é' as u32, -1, 0, &mut out),
            Err(Error::Phonemes)
        );
        assert_eq!(host.selections, [false, true]);
        assert_eq!(out, [0x97; 8]);
    }

    #[test]
    fn stress_growth_is_bounded_and_unmapped_accent_preserves_output() {
        let mut host = Fixture::new(&[(2, &[50, 0])]);
        host.append_stress = true;
        let mut out = [0x97; 2];
        assert_eq!(
            letter(&mut host, 'a' as u32, 8, 7, &mut out),
            Err(Error::Capacity)
        );
        assert_eq!(host.stress_calls, [1]);
        assert_eq!(host.keys, [b"_a \x1f".to_vec()]);
        assert_eq!(out, [0x97; 2]);
        assert_eq!(accented(&mut host, 0x17f, &mut out), Ok(None));
        assert_eq!(out, [0x97; 2]);
    }
}
