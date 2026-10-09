//! Complete isolated-character pronunciation over serialized engine primitives.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    common_text, language,
    number_digits::Buffer,
    number_lookup::{Error, PHONEME_BYTES},
    number_primitives, suffix,
};
use std::fmt::Write;
const PAUSE: u8 = 9;
const SHORT: u8 = 23;
const SWITCH: u8 = 21;
const ENGLISH: u32 = 0x656e;
const KOREAN: u32 = 0x6b6f;
const HEX_NAMES: [&[u8]; 6] = [
    b"'e:j\0", b"b'i:\0", b"s'i:\0", b"d'i:\0", b"'i:\0", b"'ef\0",
];
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    PrimaryTable,
    SamePrimary,
    PrimaryOffset,
    PrimaryAlt,
    PrimaryAlphabet,
    LocalLanguage,
    LocalTable,
    Accents,
    Dotless,
    SecondaryPresent,
    PrimaryAltLanguage,
}
pub trait Host {
    fn value(&self, field: Field) -> i32;
    /// CRT upper/alpha/space classification, in that order (0/1/2).
    fn classify(&self, code: u32, kind: u32) -> bool;
    /// Translator: local (0), original voice (1), secondary (2).
    /// Owned terminated key and initialized pronunciation; actual child extent.
    fn named(
        &mut self,
        which: u32,
        key: &[u8],
        capacity: usize,
        output: &mut [u8; PHONEME_BYTES],
    ) -> Result<i32, Error>;
    fn letter(
        &mut self,
        which: u32,
        code: u32,
        next: i32,
        control: u32,
        capacity: usize,
        output: &mut [u8; PHONEME_BYTES],
    ) -> Result<(), Error>;
    fn secondary(&mut self, name: &[u8]) -> i32;
    fn restore_table(&mut self);
    /// Owned padded Hangul text, scoped to this call; pronunciation capacity 77.
    fn hangul(
        &mut self,
        source: &mut [u8; 12],
        output: &mut [u8; PHONEME_BYTES],
    ) -> Result<(), Error>;
    /// Fixed native fallback literals, encoded by the remaining engine primitive.
    fn encode(&mut self, text: &[u8], output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error>;
    /// Fresh owner output: replace for early language switch, otherwise append
    /// only if the complete prefix fits, preserving the original overflow skip.
    fn publish(&mut self, replace: bool, output: &[u8; PHONEME_BYTES]) -> Result<(), Error>;
}
fn length(output: &[u8; PHONEME_BYTES], capacity: usize) -> Result<usize, Error> {
    if capacity == 0 || capacity > PHONEME_BYTES {
        return Err(Error::Capacity);
    }
    output[..capacity]
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Phonemes)
}
fn named(
    host: &mut impl Host,
    which: u32,
    key: &[u8],
    capacity: usize,
    out: &mut [u8; PHONEME_BYTES],
) -> Result<i32, Error> {
    let flags = host.named(which, key, capacity, out)?;
    length(out, capacity)?;
    Ok(flags)
}
fn letter(
    host: &mut impl Host,
    which: u32,
    code: u32,
    next: i32,
    control: u32,
    capacity: usize,
    out: &mut [u8; PHONEME_BYTES],
) -> Result<(), Error> {
    host.letter(which, code, next, control & 1, capacity, out)?;
    length(out, capacity)?;
    Ok(())
}
fn child(
    host: &mut impl Host,
    which: u32,
    code: u32,
    next: i32,
    control: u32,
    out: &mut [u8; PHONEME_BYTES],
) -> Result<(), Error> {
    let mut scratch = [0; PHONEME_BYTES];
    letter(host, which, code, next, control, 77, &mut scratch)?;
    let size = length(&scratch, 77)?;
    out[3..=size + 3].copy_from_slice(&scratch[..=size]);
    Ok(())
}
fn finish_switch(out: &mut [u8; PHONEME_BYTES], table: i32, capacity: usize) -> Result<(), Error> {
    out[0] = PAUSE;
    out[1] = SWITCH;
    let end = 3 + out[3..capacity]
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Phonemes)?;
    if end + 2 >= capacity {
        return Err(Error::Capacity);
    }
    out[end] = SWITCH;
    out[end + 1] = table as u8;
    out[end + 2] = 0;
    Ok(())
}
fn hangul(code: u32) -> [u8; 12] {
    let code = code - 0xac00;
    let initial = (code / 28) / 21;
    let mut out = [0; 12];
    out[0] = b' ';
    let mut cursor = 1;
    if initial != 11 {
        let (bytes, size) = suffix::encode(initial + 0x1100);
        out[cursor..cursor + size].copy_from_slice(&bytes[..size]);
        cursor += size;
    }
    for part in [((code / 28) % 21) + 0x1161, (code % 28) + 0x11a7] {
        let (bytes, size) = suffix::encode(part);
        out[cursor..cursor + size].copy_from_slice(&bytes[..size]);
        cursor += size;
    }
    out[cursor] = b' ';
    out
}
fn digit(code: u32) -> Option<u32> {
    [
        0x660, 0x6f0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66, 0xe50, 0xed0,
        0xf20, 0x1040, 0x1090,
    ]
    .iter()
    .find_map(|base| {
        code.checked_sub(*base)
            .filter(|n| *n < 10)
            .map(|n| n + u32::from(b'0'))
    })
}
/// Input decoding is the existing native UTF-8 primitive. `current_first` is the
/// selected native alphabet's range start, or None; never a foreign pointer.
/// Returns true for an early language switch (legacy consumed-byte result zero).
/// Scratch overflow rejects boundedly; already executed effects are retained.
pub fn translate(
    host: &mut impl Host,
    mut code: u32,
    next: i32,
    control: u32,
    current_first: Option<u32>,
) -> Result<bool, Error> {
    let mut capital = [0; PHONEME_BYTES];
    let mut out = [0; PHONEME_BYTES];
    let mut alphabet_name = [0; PHONEME_BYTES];
    let mut back_table = host.value(Field::PrimaryTable);
    if code & 0xfff00 == 0xe000 {
        code &= 0xff;
    }
    if control & 2 != 0 && host.classify(code, 0) {
        named(host, 0, b"_cap\0", 30, &mut capital)?;
    }
    code = common_text::lower(code, host.value(Field::Dotless) != 0);
    letter(host, 0, code, next, control, 80, &mut out)?;
    if out[0] == 0 {
        let derived = number_primitives::superscript(code as i32);
        if derived != 0 {
            code = (derived & 0x3fff) as u32;
            let modifier = match derived >> 14 {
                1 => Some(&b"_sub\0"[..]),
                2 => Some(&b"_sup\0"[..]),
                _ => None,
            };
            if let Some(modifier) = modifier.filter(|_| control & 4 != 0) {
                named(host, 0, modifier, 30, &mut capital)?;
                if capital[0] == 0 {
                    capital[2] = host.secondary(b"en\0") as u8;
                    let mut scratch = [0; PHONEME_BYTES];
                    named(host, 2, modifier, 27, &mut scratch)?;
                    let size = length(&scratch, 27)?;
                    capital[3..=size + 3].copy_from_slice(&scratch[..=size]);
                    if capital[3] != 0 {
                        finish_switch(&mut capital, back_table, 30)?;
                    }
                }
            }
        }
        letter(host, 0, code, next, control, 80, &mut out)?;
    }
    if out[0] == SWITCH {
        host.publish(true, &out)?;
        return Ok(true);
    }
    if out[0] == 0 {
        if let Some(number) = digit(code) {
            letter(host, 0, number, 0, control, 80, &mut out)?;
        }
    }
    let alphabet = language::alphabet_from_char(code as i32);
    let offset = alphabet.map_or(0, |a| a.offset);
    let flags = alphabet.map_or(0, |a| a.flags);
    if alphabet.map(|a| a.first) != current_first {
        if let Some(alphabet) =
            alphabet.filter(|a| a.flags & 1 == 0 && a.offset != host.value(Field::PrimaryOffset))
        {
            if offset != host.value(Field::PrimaryAlt)
                && offset != host.value(Field::PrimaryAlphabet)
            {
                let mut other = [0; PHONEME_BYTES];
                if named(host, 1, alphabet.name, 80, &mut alphabet_name)? == 0 {
                    alphabet_name[2] = host.secondary(b"en\0") as u8;
                    named(host, 2, alphabet.name, 80, &mut other)?;
                } else if host.value(Field::SamePrimary) == 0 {
                    back_table = host.value(Field::LocalTable);
                    other = alphabet_name;
                    alphabet_name[2] = host.value(Field::PrimaryTable) as u8;
                }
                if other[0] != 0 {
                    let size = length(&other, 80)?;
                    if size + 5 >= 80 {
                        return Err(Error::Capacity);
                    }
                    alphabet_name[3..=size + 3].copy_from_slice(&other[..=size]);
                    finish_switch(&mut alphabet_name, back_table, 80)?;
                }
            }
        }
    }
    if out[0] == 0 {
        let target = if offset != 0 && offset == host.value(Field::PrimaryAlt) {
            host.value(Field::PrimaryAltLanguage) as u32
        } else if let Some(a) = alphabet.filter(|a| a.language != 0 && a.flags & 2 == 0) {
            a.language as u32
        } else {
            ENGLISH
        };
        if target != host.value(Field::LocalLanguage) as u32 || target == KOREAN {
            let (name, size) = crate::clause_input::language_word(target);
            let mut terminated = [0; 5];
            terminated[..size].copy_from_slice(&name[..size]);
            out[2] = host.secondary(&terminated[..=size]) as u8;
            if host.value(Field::SecondaryPresent) != 0 {
                if (0xac00..=0xd7af).contains(&code) {
                    let mut scratch = [0; PHONEME_BYTES];
                    host.hangul(&mut hangul(code), &mut scratch)?;
                    let size = length(&scratch, 77)?;
                    out[3..=size + 3].copy_from_slice(&scratch[..=size]);
                } else {
                    child(host, 2, code, next, control, &mut out)?;
                }
                if out[3] == SWITCH {
                    let size = out[4..80]
                        .iter()
                        .position(|byte| *byte == 0)
                        .ok_or(Error::Phonemes)?;
                    let mut name = [0; 80];
                    name[..size].copy_from_slice(&out[4..4 + size]);
                    out[2] = host.secondary(&name[..=size]) as u8;
                    child(host, 2, code, next, control, &mut out)?;
                }
                host.restore_table();
                if out[3] != 0 {
                    finish_switch(&mut out, host.value(Field::LocalTable), 80)?;
                }
            }
        }
    }
    if out[0] == 0 {
        if flags & 0x10 == 0 {
            if host.classify(code, 1) {
                named(host, 1, b"_?A\0", 80, &mut out)?;
            }
            if out[0] == 0 && !host.classify(code, 2) {
                named(host, 1, b"_??\0", 80, &mut out)?;
            }
            if out[0] == 0 {
                host.encode(b"l'et@\0", &mut out)?;
                length(&out, 80)?;
            }
        }
        if control & 4 != 0 || flags & 8 == 0 {
            let mut key = Buffer::new(12);
            if offset == 0x2800 {
                for i in 0..8 {
                    if code & (1 << i) != 0 {
                        key.append(&[b'1' + i])?;
                    }
                }
            } else {
                write!(key, "{code:x}").map_err(|_| Error::Capacity)?;
            }
            let mut last = 0;
            for digit in key.bytes() {
                let end = length(&out, 80)?;
                if end + 1 >= 80 {
                    return Err(Error::Capacity);
                }
                out[end] = SHORT;
                last = end + 1;
                let mut scratch = [0; PHONEME_BYTES];
                letter(host, 1, u32::from(*digit), 0, 1, 80 - last, &mut scratch)?;
                if matches!(scratch[0], 0 | SWITCH) && (b'a'..=b'f').contains(digit) {
                    host.encode(HEX_NAMES[usize::from(*digit - b'a')], &mut scratch)?;
                }
                let size = length(&scratch, 80 - last)?;
                out[last..=last + size].copy_from_slice(&scratch[..=size]);
            }
            let end = last
                + out[last..80]
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or(Error::Phonemes)?;
            if end + 1 >= 80 {
                return Err(Error::Capacity);
            }
            out[end] = PAUSE;
            out[end + 1] = 0;
        }
    }
    let mut joined = Buffer::new(80);
    joined.append(&[255])?;
    joined.append(&alphabet_name[..length(&alphabet_name, 80)?])?;
    let capital_first = host.value(Field::Accents) & 2 == 0;
    for (item, capacity) in if capital_first {
        [(&capital, 30), (&out, 80)]
    } else {
        [(&out, 80), (&capital, 30)]
    } {
        joined.append(&item[..length(item, capacity)?])?;
    }
    let mut result = [0; PHONEME_BYTES];
    joined.publish(&mut result)?;
    host.publish(false, &result)?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    type Named = (u32, &'static [u8], usize, i32, Vec<u8>);
    type Letter = (u32, u32, i32, u32, usize, Vec<u8>);
    struct Fixture {
        fields: [i32; 11],
        names: VecDeque<Named>,
        letters: VecDeque<Letter>,
        setups: VecDeque<(&'static [u8], i32)>,
        output: Vec<u8>,
        effects: Vec<&'static str>,
        nested: bool,
        hangul: Vec<u8>,
        encoded: Vec<Vec<u8>>,
    }
    impl Fixture {
        fn new() -> Self {
            Self {
                fields: [3, 1, 0, 0, 0, ENGLISH as i32, 3, 0, 0, 1, 0],
                names: VecDeque::new(),
                letters: VecDeque::new(),
                setups: VecDeque::new(),
                output: Vec::new(),
                effects: Vec::new(),
                nested: false,
                hangul: Vec::new(),
                encoded: Vec::new(),
            }
        }
        fn letter(
            &mut self,
            which: u32,
            code: u32,
            next: i32,
            control: u32,
            capacity: usize,
            output: &[u8],
        ) {
            self.letters
                .push_back((which, code, next, control, capacity, output.to_vec()));
        }
        fn named(
            &mut self,
            which: u32,
            key: &'static [u8],
            capacity: usize,
            flags: i32,
            output: Vec<u8>,
        ) {
            self.names.push_back((which, key, capacity, flags, output));
        }
        fn done(&self) {
            assert!(self.names.is_empty());
            assert!(self.letters.is_empty());
            assert!(self.setups.is_empty());
        }
    }
    fn copy(output: &mut [u8; PHONEME_BYTES], bytes: &[u8]) {
        output[..bytes.len()].copy_from_slice(bytes);
        if bytes.len() < output.len() {
            output[bytes.len()] = 0;
        }
    }
    impl Host for Fixture {
        fn value(&self, field: Field) -> i32 {
            self.fields[field as usize]
        }
        fn classify(&self, code: u32, kind: u32) -> bool {
            match kind {
                0 => (b'A' as u32..=b'Z' as u32).contains(&code),
                1 => code < 128 && (code as u8).is_ascii_alphabetic(),
                _ => code == 32,
            }
        }
        fn named(
            &mut self,
            which: u32,
            key: &[u8],
            capacity: usize,
            output: &mut [u8; PHONEME_BYTES],
        ) -> Result<i32, Error> {
            self.effects.push("named");
            let (w, k, c, flags, bytes) = self.names.pop_front().expect("unexpected name");
            assert_eq!((which, key, capacity), (w, k, c));
            copy(output, &bytes);
            Ok(flags)
        }
        fn letter(
            &mut self,
            which: u32,
            code: u32,
            next: i32,
            control: u32,
            capacity: usize,
            output: &mut [u8; PHONEME_BYTES],
        ) -> Result<(), Error> {
            self.effects.push("letter");
            let (w, c, n, t, cap, bytes) = self.letters.pop_front().expect("unexpected letter");
            assert_eq!((which, code, next, control, capacity), (w, c, n, t, cap));
            copy(output, &bytes);
            if self.nested {
                self.output = b"nested".to_vec();
                self.fields[Field::Accents as usize] = 2;
            }
            Ok(())
        }
        fn secondary(&mut self, name: &[u8]) -> i32 {
            self.effects.push("secondary");
            let (expected, table) = self.setups.pop_front().unwrap();
            assert_eq!(name, expected);
            table
        }
        fn restore_table(&mut self) {
            self.effects.push("restore");
        }
        fn hangul(
            &mut self,
            source: &mut [u8; 12],
            output: &mut [u8; PHONEME_BYTES],
        ) -> Result<(), Error> {
            self.effects.push("hangul");
            self.hangul = source.to_vec();
            copy(output, b"han");
            Ok(())
        }
        fn encode(&mut self, text: &[u8], output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
            self.effects.push("encode");
            self.encoded.push(text.to_vec());
            copy(output, b"Z");
            Ok(())
        }
        fn publish(&mut self, replace: bool, output: &[u8; PHONEME_BYTES]) -> Result<(), Error> {
            self.effects.push("publish");
            let len = length(output, 200)?;
            if replace {
                self.output = output[..len].to_vec();
            } else if self.output.len() + len < 200 {
                self.output.extend_from_slice(&output[..len]);
            }
            Ok(())
        }
    }
    #[test]
    fn capital_order_and_publication_use_fresh_nested_state() {
        let mut host = Fixture::new();
        host.nested = true;
        host.named(0, b"_cap\0", 30, 1, b"CAP".to_vec());
        host.letter(0, b'a' as u32, 32, 0, 80, b"letter");
        assert_eq!(translate(&mut host, b'A' as u32, 32, 2, None), Ok(false));
        assert_eq!(host.output, b"nested\xffletterCAP");
        assert_eq!(host.effects, ["named", "letter", "publish"]);
        host.done();
    }
    #[test]
    fn superscript_modifier_preserves_embedded_zero_table_and_probe_order() {
        let mut host = Fixture::new();
        host.letter(0, 0x2074, 32, 0, 80, b"");
        host.named(0, b"_sup\0", 30, 0, vec![]);
        host.setups.push_back((b"en\0", 0));
        host.named(2, b"_sup\0", 27, 1, b"sup".to_vec());
        host.letter(0, b'4' as u32, 32, 0, 80, b"four");
        assert_eq!(translate(&mut host, 0x2074, 32, 4, None), Ok(false));
        assert_eq!(host.output, b"\xff\x09\x15four");
        assert_eq!(
            host.effects,
            ["letter", "named", "secondary", "named", "letter", "publish"]
        );
        host.done();
    }
    #[test]
    fn initial_language_switch_replaces_output_and_stops() {
        let mut host = Fixture::new();
        host.output = b"old".to_vec();
        host.letter(0, 0x1f600, 32, 1, 80, b"\x15el");
        assert_eq!(translate(&mut host, 0x1f600, 32, 1, None), Ok(true));
        assert_eq!(host.output, b"\x15el");
        host.done();
    }
    #[test]
    fn script_name_uses_original_voice_table_then_local_return_table() {
        let mut host = Fixture::new();
        host.fields[0] = 7;
        host.fields[1] = 0;
        host.fields[6] = 5;
        host.letter(0, 0x436, 32, 0, 80, b"zhe");
        host.named(1, b"_cyr\0", 80, 1, b"name".to_vec());
        assert_eq!(translate(&mut host, 0x416, 32, 0, None), Ok(false));
        assert_eq!(host.output, b"\xff\x09\x15\x07name\x15\x05zhe");
        host.done();
        let mut host = Fixture::new();
        host.letter(0, 0x436, 32, 0, 80, b"zhe");
        assert_eq!(translate(&mut host, 0x416, 32, 0, Some(0x400)), Ok(false));
        assert_eq!(host.output, b"\xffzhe");
        host.done();
    }
    #[test]
    fn hangul_decomposes_to_owned_padded_jamo_then_restores_voice() {
        let mut host = Fixture::new();
        host.letter(0, 0xac00, 32, 0, 80, b"");
        host.letter(0, 0xac00, 32, 0, 80, b"");
        host.setups.push_back((b"ko\0", 4));
        assert_eq!(translate(&mut host, 0xac00, 32, 0, Some(0xa700)), Ok(false));
        assert_eq!(
            &host.hangul[..11],
            b" \xe1\x84\x80\xe1\x85\xa1\xe1\x86\xa7 "
        );
        assert_eq!(host.output, b"\xff\x09\x15\x04han\x15\x03");
        assert_eq!(
            host.effects,
            [
                "letter",
                "letter",
                "secondary",
                "hangul",
                "restore",
                "publish"
            ]
        );
        host.done();
        assert_eq!(&hangul(0xc544)[..8], b" \xe1\x85\xa1\xe1\x86\xa7 ");
    }
    #[test]
    fn nested_language_request_is_copied_before_secondary_replacement() {
        let mut host = Fixture::new();
        host.fields[5] = 0x6875;
        host.letter(0, 0x1f600, -1, 0, 80, b"");
        host.letter(0, 0x1f600, -1, 0, 80, b"");
        host.setups.push_back((b"en\0", 2));
        host.setups.push_back((b"fr\0", 6));
        host.letter(2, 0x1f600, -1, 0, 77, b"\x15fr");
        host.letter(2, 0x1f600, -1, 0, 77, b"sym");
        assert_eq!(translate(&mut host, 0x1f600, -1, 0, None), Ok(false));
        assert_eq!(host.output, b"\xff\x09\x15\x06sym\x15\x03");
        host.done();
    }
    #[test]
    fn braille_lists_dots_without_unknown_symbol_and_blank_keeps_pause() {
        for (code, expected) in [
            (0x2805, &b"\xff\x171\x173\x09"[..]),
            (0x2800, &b"\xff\x09"[..]),
        ] {
            let mut host = Fixture::new();
            host.fields[2] = 0x2800;
            host.letter(0, code, 32, 0, 80, b"");
            host.letter(0, code, 32, 0, 80, b"");
            if code == 0x2805 {
                host.letter(1, b'1' as u32, 0, 1, 79, b"1");
                host.letter(1, b'3' as u32, 0, 1, 77, b"3");
            }
            assert_eq!(translate(&mut host, code, 32, 0, None), Ok(false));
            assert_eq!(host.output, expected);
            host.done();
        }
    }
    #[test]
    fn hexadecimal_names_fall_back_to_native_english_literals() {
        let mut host = Fixture::new();
        host.letter(0, 0xab, 32, 0, 80, b"");
        host.letter(0, 0xab, 32, 0, 80, b"");
        host.named(1, b"_??\0", 80, 1, b"unk".to_vec());
        host.letter(1, b'a' as u32, 0, 1, 76, b"\x15en");
        host.letter(1, b'b' as u32, 0, 1, 74, b"");
        assert_eq!(translate(&mut host, 0xab, 32, 0, None), Ok(false));
        assert_eq!(host.output, b"\xffunk\x17Z\x17Z\x09");
        assert_eq!(host.encoded, [b"'e:j\0".to_vec(), b"b'i:\0".to_vec()]);
        host.done();
    }
    #[test]
    fn scratch_overflow_rejects_without_replay_and_full_owner_skips_append() {
        let mut host = Fixture::new();
        host.letter(0, b'a' as u32, 32, 0, 80, &[8; 200]);
        assert_eq!(
            translate(&mut host, b'a' as u32, 32, 0, None),
            Err(Error::Phonemes)
        );
        assert_eq!(host.effects, ["letter"]);
        host.done();
        let mut host = Fixture::new();
        host.named(0, b"_cap\0", 30, 1, vec![b'C'; 29]);
        host.letter(0, b'a' as u32, 32, 0, 80, &[b'a'; 60]);
        assert_eq!(
            translate(&mut host, b'A' as u32, 32, 2, None),
            Err(Error::Capacity)
        );
        assert_eq!(host.effects, ["named", "letter"]);
        host.done();
        let mut host = Fixture::new();
        host.output = vec![8; 199];
        host.letter(0, b'a' as u32, 32, 0, 80, b"a");
        assert_eq!(translate(&mut host, b'a' as u32, 32, 0, None), Ok(false));
        assert_eq!(host.output, vec![8; 199]);
        host.done();
    }
    #[test]
    fn private_uppercase_uses_dotless_lowering_and_digits_keep_probe_order() {
        let mut host = Fixture::new();
        host.fields[8] = 1;
        host.named(0, b"_cap\0", 30, 1, b"CAP".to_vec());
        host.letter(0, 0x131, 32, 0, 80, b"i");
        assert_eq!(translate(&mut host, 0xe049, 32, 2, None), Ok(false));
        assert_eq!(host.output, b"\xffCAPi");
        host.done();
        let mut host = Fixture::new();
        host.letter(0, 0xe54, 32, 1, 80, b"");
        host.letter(0, 0xe54, 32, 1, 80, b"");
        host.letter(0, b'4' as u32, 0, 1, 80, b"four");
        // Thai still has a script record; current alphabet suppresses its name.
        assert_eq!(translate(&mut host, 0xe54, 32, 1, Some(0xe00)), Ok(false));
        assert_eq!(host.output, b"\xfffour");
        host.done();
    }
    #[test]
    fn absent_secondary_preserves_fallback_and_avoids_table_restore() {
        let mut host = Fixture::new();
        host.fields[5] = 0x6875;
        host.fields[9] = 0;
        host.letter(0, 0xab, 32, 0, 80, b"");
        host.letter(0, 0xab, 32, 0, 80, b"");
        host.setups.push_back((b"en\0", 2));
        host.named(1, b"_??\0", 80, 1, b"unknown".to_vec());
        host.letter(1, b'a' as u32, 0, 1, 72, b"a");
        host.letter(1, b'b' as u32, 0, 1, 70, b"b");
        assert_eq!(translate(&mut host, 0xab, 32, 0, None), Ok(false));
        assert_eq!(host.output, b"\xffunknown\x17a\x17b\x09");
        assert!(!host.effects.contains(&"restore"));
        host.done();
    }
}
