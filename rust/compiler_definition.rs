//! Complete phoneme/property/procedure compilation over owned bytecode state.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_program::{Host, Program, Token},
    phoneme::{feature_from_name, mnemonic, Phoneme},
};
pub const PROCEDURES: usize = 50;
#[derive(Clone)]
struct Procedure {
    name: [u8; 256],
    address: i32,
}
#[derive(Default)]
pub struct Compiler {
    procedures: Vec<Procedure>,
}
pub trait DefinitionHost: Host {
    fn eof(&mut self) -> bool;
    fn lookup_mode(&mut self, name: &[u8], mode: i32) -> Result<i32, i32>;
    fn row(&mut self, index: i32) -> Result<Phoneme, i32>;
    fn project(&mut self, row: Phoneme) -> Result<(), i32>;
    fn find(&mut self, name: &[u8]) -> Result<Option<Phoneme>, i32>;
    fn import(&mut self, name: &[u8], row: Phoneme) -> Result<Phoneme, i32>;
    fn unknown_feature(&mut self, name: &[u8]);
}
impl Compiler {
    pub fn reset(&mut self) -> Result<(), i32> {
        if self.procedures.capacity() < PROCEDURES {
            self.procedures
                .try_reserve_exact(PROCEDURES - self.procedures.len())
                .map_err(|_| 12)?;
        }
        self.procedures.clear();
        Ok(())
    }
    pub fn count(&self) -> usize {
        self.procedures.len()
    }
    fn call(
        &mut self,
        p: &mut Program,
        row: &mut Phoneme,
        flags: &mut u32,
        h: &mut impl DefinitionHost,
    ) -> Result<(), i32> {
        let name = h.next(2)?;
        let address = if let Some(proc) = self
            .procedures
            .iter()
            .find(|proc| text(&proc.name) == name.name())
        {
            proc.address
        } else {
            let Some(source) = h.find(name.name())? else {
                return Ok(());
            };
            if row.kind == 15 {
                row.kind = source.kind;
                row.start_type = source.start_type;
                row.end_type = source.end_type;
                row.standard_length = source.standard_length;
                row.length_modifier = source.length_modifier;
                *flags = source.flags & !0xf0000;
            }
            i32::from(source.program)
        };
        p.emit(
            &[
                (0x9100i32.wrapping_add(address >> 16)) as u16,
                address as u16,
            ],
            h,
        )
    }
    pub fn compile(
        &mut self,
        p: &mut Program,
        phoneme: bool,
        log: bool,
        alnum: &[u8; 256],
        h: &mut impl DefinitionHost,
    ) -> Result<Option<Phoneme>, i32> {
        p.reset()?;
        let name = h.next(2)?;
        let mut row = if phoneme {
            let code = h.lookup_mode(name.name(), 1)?;
            if code == -1 {
                return Ok(None);
            }
            Phoneme {
                mnemonic: h.row(code)?.mnemonic,
                code: code as u8,
                kind: 15,
                ..Phoneme::default()
            }
        } else {
            if self.procedures.len() >= PROCEDURES {
                h.error(b"Too many procedures");
                return Ok(None);
            }
            let n = self.procedures.len();
            Phoneme {
                mnemonic: mnemonic(&[
                    b'0' + (n / 100) as u8,
                    b'0' + ((n / 10) % 10) as u8,
                    b'0' + (n % 10) as u8,
                    b'P',
                ]),
                kind: 15,
                ..Phoneme::default()
            }
        };
        h.project(row)?;
        let mut flags = 0u32;
        let mut ended = 0;
        while ended == 0 && !h.eof() {
            let token = h.next(7)?;
            let key = token.value;
            if key < 0 {
                if key == -2 {
                    h.error(b"Missing 'endphoneme' before end-of-file");
                    break;
                }
                if row.add_feature(feature_from_name(token.name())).is_err() {
                    h.unknown_feature(token.name());
                }
                h.project(row)?;
                continue;
            }
            match token.kind {
                1 => {
                    if row.kind != 15 && !(row.kind == 6 && key == 3) {
                        named_error(h, b"More than one phoneme type: ", token.name(), b"");
                    }
                    row.kind = key as u8;
                }
                2 => flags |= key as u32,
                5 => match key {
                    1..=6 | 0x10..=0x14 => {
                        let value = Program::brackets(h, 5, 0)?.value;
                        p.emit(&[(key.wrapping_shl(8).wrapping_add(value)) as u16], h)?;
                        p.decrement();
                    }
                    7 | 8 => {
                        let value = max_number(h, 255)?;
                        p.emit(&[(key.wrapping_shl(8).wrapping_add(value)) as u16], h)?;
                        p.decrement();
                    }
                    10 => {
                        let value = max_number(h, 511)?;
                        if !p.after_if() {
                            row.standard_length = (value / 2) as u8;
                        } else {
                            p.emit(&[(0xa00i32.wrapping_add(value / 2)) as u16], h)?;
                            p.decrement();
                        }
                    }
                    12 => {
                        let value = h.next(4)?.value / 2;
                        p.emit(&[(0xc00 + (value & 255)) as u16], h)?;
                        p.decrement();
                    }
                    9 => {
                        let value = h.next(3)?.value;
                        if value >= 10 {
                            h.error(b"Invalid length modifier");
                            return Err(22);
                        }
                        row.length_modifier = value as u8;
                    }
                    13 => {
                        let token = h.next(2)?;
                        let (words, count) = ipa(&token, alnum).inspect_err(|_| {
                            h.error(b"Invalid or oversized IPA name");
                        })?;
                        p.emit(&words[..count], h)?;
                        p.decrement();
                    }
                    _ => {}
                },
                4 => match key {
                    8 => {
                        let name = h.next(2)?;
                        row = h.import(name.name(), row)?;
                        flags = row.flags;
                    }
                    9..=12 => {
                        if key == 11 {
                            row.standard_length = h.next(3)?.value as u8;
                            if !p.words().is_empty() {
                                h.error(b"stress phonemes can't contain program instructions");
                                p.clear_words();
                            }
                        } else {
                            let token = h.next(5)?;
                            let mut code = token.value;
                            if code == -1 {
                                code = h.lookup_mode(token.name(), 1)?;
                            }
                            match key {
                                9 => {
                                    row.start_type = code as u8;
                                    if row.kind == 15 {
                                        h.error(b"a phoneme type or manner of articulation must be specified before starttype");
                                    }
                                }
                                10 => {
                                    if row.kind == 15 {
                                        h.error(b"a phoneme type or manner of articulation must be specified before endtype");
                                    } else if row.kind == 2 {
                                        row.end_type = code as u8;
                                    } else if code != i32::from(row.start_type) {
                                        h.error(b"endtype must equal starttype for consonants");
                                    }
                                }
                                12 => {
                                    if row.kind == 2 {
                                        h.error(b"voicingswitch cannot be used on vowels");
                                    } else {
                                        row.end_type = code as u8;
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                    13 => ended = p.condition(false, h)?,
                    14 => p.otherwise(h)?,
                    15 => p.elif(h)?,
                    16 => p.endif(h)?,
                    18 | 19 => p.switch(key - 17, h)?,
                    17 => {
                        self.call(p, &mut row, &mut flags, h)?;
                        p.decrement();
                    }
                    21..=25 => {
                        if key == 21 || key == 22 {
                            p.mark_returned();
                        }
                        p.decrement();
                        h.project(row)?;
                        p.sound(key, i32::from(key == 21 && row.kind == 2), h)?;
                    }
                    26 | 27 => {
                        p.decrement();
                        ended = {
                            p.transition(key - 25, i32::from(row.kind), h)?;
                            0
                        };
                    }
                    28 => {
                        p.decrement();
                        h.project(row)?;
                        let (a, b) = p.tone(h)?;
                        row.start_type = a;
                        row.end_type = b;
                    }
                    29 | 30 => {
                        p.emit(&[if key == 29 { 1 } else { 2 }], h)?;
                        p.decrement();
                    }
                    4..=7 => {
                        if key == 6 || key == 7 {
                            named_error(h, b"Missing 'endphoneme' before '", token.name(), b"'");
                        }
                        ended = 1;
                        p.finish(h)?;
                    }
                    _ => {}
                },
                _ => {}
            }
            h.project(row)?;
        }
        if ended != 1 {
            h.error(b"'endphoneme' not expected here");
        }
        if phoneme {
            if row.kind == 15 {
                h.error(b"Phoneme type is missing");
                row.kind = 0;
            }
            row.flags |= flags;
            if row.flags & 16 != 0 {
                if row.kind == 4 {
                    row.kind = 5;
                } else if row.kind == 6 {
                    row.kind = 7;
                }
            }
            if row.standard_length == 0 && row.kind == 2 {
                row.standard_length = 90;
            }
            row.flags |= 1 << 31;
            if row.kind == 14 {
                row.mnemonic = 1;
            }
        }
        h.project(row)?;
        if !p.words().is_empty() {
            if let Some(address) = p.publish(log, h)? {
                row.program = address as u16;
                if !phoneme {
                    self.procedures.push(Procedure {
                        name: name.text,
                        address,
                    });
                }
            }
        }
        h.project(row)?;
        Ok(Some(row))
    }
}
fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn named_error(h: &mut impl Host, prefix: &[u8], name: &[u8], suffix: &[u8]) {
    let mut bytes = [0; 320];
    let n = prefix.len() + name.len() + suffix.len();
    bytes[..prefix.len()].copy_from_slice(prefix);
    bytes[prefix.len()..prefix.len() + name.len()].copy_from_slice(name);
    bytes[prefix.len() + name.len()..n].copy_from_slice(suffix);
    h.error(&bytes[..n]);
}
fn max_number(h: &mut impl Host, max: i32) -> Result<i32, i32> {
    let mut n = h.next(3)?.value;
    if n > max {
        h.error(format!("Value {n} is greater than maximum {max}").as_bytes());
        n = max;
    }
    Ok(n)
}
/// Fixed IPA scratch, permissive legacy UTF-8 and partial four-digit hex.
pub fn ipa(token: &Token, alnum: &[u8; 256]) -> Result<([u16; 130], usize), i32> {
    let mut input = token.text;
    if token.name() == b"NULL" {
        input[0] = b' ';
        input[1] = 0;
    }
    let mut bytes = [0u8; 257];
    let (mut at, mut out, mut characters, mut flags) = (0usize, 1usize, 0usize, 0u32);
    while at < input.len() && input[at] != 0 {
        let character = crate::utf8::decode(&input, at, false).map_err(|_| 22)?;
        at = at.checked_add(character.width).ok_or(22)?;
        let mut code = character.code;
        if code == u32::from(b'|') && characters > 0 {
            if characters > 32 {
                return Err(22);
            }
            flags |= 1u32 << (characters - 1);
            continue;
        }
        if code == u32::from(b'U') && input.get(at) == Some(&b'+') {
            at += 1;
            let mut hex = [0u8; 4];
            for (i, b) in hex.iter_mut().enumerate() {
                *b = input.get(at + i).copied().unwrap_or(0);
            }
            code = parse_hex(&hex).unwrap_or(u32::from(b'#'));
            for _ in 0..4 {
                let b = input.get(at).copied().unwrap_or(0);
                if alnum[usize::from(b)] == 0 {
                    break;
                }
                at += 1;
            }
        }
        let (encoded, n) = crate::suffix::encode(code);
        if out + n >= bytes.len() {
            return Err(22);
        }
        bytes[out..out + n].copy_from_slice(&encoded[..n]);
        out += n;
        characters += 1;
    }
    bytes[0] = flags as u8;
    bytes[out] = 0;
    let start = usize::from(flags == 0);
    let n = text(&bytes[start..]).len();
    let mut words = [0u16; 130];
    words[0] = 0xd00u16.wrapping_add(n as u16);
    for i in (0..n).step_by(2) {
        words[1 + i / 2] = u16::from(bytes[start + i]) * 256 + u16::from(bytes[start + i + 1]);
    }
    Ok((words, 1 + n.div_ceil(2)))
}
fn parse_hex(bytes: &[u8; 4]) -> Option<u32> {
    let mut i = 0;
    while i < 4 && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let negative = bytes.get(i) == Some(&b'-');
    if negative || bytes.get(i) == Some(&b'+') {
        i += 1;
    }
    if bytes.get(i) == Some(&b'0') && matches!(bytes.get(i + 1), Some(b'x' | b'X')) {
        i += 2;
        // scanf("%x") accepts the leading zero even when the prefix has no
        // following hex digit; the resulting NUL truncates the IPA payload.
        if !bytes.get(i).is_some_and(u8::is_ascii_hexdigit) {
            return Some(0);
        }
    }
    let (mut count, mut value) = (0, 0u32);
    for b in &bytes[i..] {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => break,
        };
        value = value * 16 + u32::from(digit);
        count += 1;
    }
    if count == 0 {
        None
    } else {
        Some(if negative {
            value.wrapping_neg()
        } else {
            value
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(bytes: &[u8]) -> Token {
        let mut t = Token::default();
        t.text[..bytes.len()].copy_from_slice(bytes);
        t
    }
    fn map() -> [u8; 256] {
        std::array::from_fn(|i| u8::from((i as u8).is_ascii_alphanumeric()))
    }
    #[test]
    fn ipa_partial_hex_and_nul_follow_scanf_and_strlen() {
        for (source, expected) in [
            (b"U+0x!a".as_slice(), vec![0xd00]),
            (b"U+0000", vec![0xd00]),
            (b"U+ZZZA", vec![0xd01, 0x2300]),
            (b"U+03B1", vec![0xd02, 0xceb1]),
            (b"NULL", vec![0xd01, 0x2000]),
        ] {
            let (words, n) = ipa(&token(source), &map()).unwrap();
            assert_eq!(&words[..n], expected);
        }
    }
    #[test]
    fn ipa_join_flags_preserve_full_flag_test_before_byte_narrowing() {
        let (words, n) = ipa(&token(b"a|bc|d"), &map()).unwrap();
        assert_eq!(&words[..n], &[0xd05, 0x0561, 0x6263, 0x6400]);
        let (words, n) = ipa(&token(b"abcdefghij|"), &map()).unwrap();
        assert_eq!(&words[..n], &[0xd00]);
    }
    #[test]
    fn ipa_rejects_shift_and_expansion_beyond_owned_scratch() {
        assert!(ipa(&token(b"abcdefghijklmnopqrstuvwxyzabcdefgh|"), &map()).is_err());
        let mut malformed = vec![0xff; 254];
        malformed.push(b'a');
        assert!(ipa(&token(&malformed), &map()).is_err());
        let (words, n) = ipa(&token(&[b'a'; 255]), &map()).unwrap();
        assert_eq!(n, 129);
        assert_eq!(words[0], 0xdff);
    }
    #[test]
    fn procedure_storage_retains_scanner_width_and_reuses_allocation() {
        let mut c = Compiler::default();
        c.reset().unwrap();
        let address = c.procedures.as_ptr();
        let mut name = [b'x'; 256];
        name[255] = 0;
        for i in 0..PROCEDURES {
            c.procedures.push(Procedure {
                name,
                address: 65537 + i as i32,
            });
        }
        assert_eq!(text(&c.procedures[49].name).len(), 255);
        c.reset().unwrap();
        assert_eq!(c.count(), 0);
        assert_eq!(c.procedures.as_ptr(), address);
    }
}
