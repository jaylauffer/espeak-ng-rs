//! Mnemonic/IPA/stress spelling and complete translated-clause text assembly.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme::Phoneme;

pub const MAX_LIST: usize = 1001;
pub const NAME_BYTES: usize = 64;
// A table name has at most31 bytes plus parentheses. Each input byte can
// produce at most4 UTF-8 bytes plus a4-byte tie. Three appended names add at
// most33 bytes each; word/phoneme separator/stress/NUL add at most8. Thus371
// bytes suffice for one node, including malformed-but-terminated names.
pub const NODE_BYTES: usize = 384;
pub const CLAUSE_BYTES: usize = MAX_LIST * NODE_BYTES + 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Context,
    Terminator,
    Mnemonic,
    Capacity,
    Effect,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Node {
    pub phoneme: Option<Phoneme>,
    pub flags: u16,
    pub stress: u8,
    pub kind: u8,
    pub new_word: u8,
    pub tone: u8,
}
/// None selects a standalone caller's list node (possibly absent). Effects
/// may replace list metadata/selected table slots. Phoneme records themselves
/// remain immutable; returned copies do not borrow an engine owner.
pub trait Host {
    fn count(&self) -> Result<usize, Error>;
    fn node(&self, slot: Option<usize>) -> Option<Node>;
    fn phoneme(&self, slot: Option<usize>) -> Option<Phoneme> {
        self.node(slot)?.phoneme
    }
    fn stress_node(&self, slot: Option<usize>) -> Option<Node> {
        self.node(slot)
    }
    fn table_code(&self, slot: Option<usize>) -> Option<u8> {
        Some(self.node(slot)?.tone)
    }
    fn special(&self, code: u8) -> Option<Phoneme>;
    fn table_name(&self, table: u8) -> Option<[u8; 32]>;
    fn tone(&self, slot: Option<usize>) -> Option<Phoneme>;
    fn ipa(&mut self, code: u8, slot: Option<usize>, out: &mut [u8; 18]) -> Result<(), Error>;
    fn alpha(&self, code: u32) -> bool;
}

const IPA: [u16; 96] = [
    0x20, 0x21, 0x22, 0x2b0, 0x24, 0x25, 0xe6, 0x2c8, 0x28, 0x29, 0x27e, 0x2b, 0x2cc, 0x2d, 0x2e,
    0x2f, 0x252, 0x31, 0x32, 0x25c, 0x34, 0x35, 0x36, 0x37, 0x275, 0x39, 0x2d0, 0x2b2, 0x3c, 0x3d,
    0x3e, 0x294, 0x259, 0x251, 0x3b2, 0xe7, 0xf0, 0x25b, 0x46, 0x262, 0x127, 0x26a, 0x25f, 0x4b,
    0x26b, 0x271, 0x14b, 0x254, 0x3a6, 0x263, 0x280, 0x283, 0x3b8, 0x28a, 0x28c, 0x153, 0x3c7,
    0xf8, 0x292, 0x32a, 0x5c, 0x5d, 0x5e, 0x5f, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x261,
    0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77,
    0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x303, 0x7f,
];
struct Writer<'a> {
    output: &'a mut [u8],
    len: usize,
}
impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self.len.checked_add(bytes.len()).ok_or(Error::Capacity)?;
        self.output
            .get_mut(self.len..end)
            .ok_or(Error::Capacity)?
            .copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }
    fn code(&mut self, code: u32) -> Result<(), Error> {
        let (bytes, len) = crate::suffix::encode(code);
        self.bytes(&bytes[..len])
    }
    fn stress(&mut self, node: Node, ipa: bool) -> Result<(), Error> {
        if node.flags & 4 != 0 && node.stress > 1 {
            let level = node.stress.min(5);
            self.code(if ipa {
                if level > 3 {
                    0x2c8
                } else {
                    0x2cc
                }
            } else {
                u32::from(b"==,,''"[usize::from(level)])
            })?;
        }
        Ok(())
    }
}
pub struct Text {
    bytes: [u8; NAME_BYTES],
    len: usize,
    flags: Option<i32>,
}
impl Text {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn terminated(&self) -> &[u8] {
        &self.bytes[..=self.len]
    }
    /// None leaves the caller's possibly-uninitialized flags integer untouched.
    pub fn flags(&self) -> Option<i32> {
        self.flags
    }
}
pub fn mnemonic(
    host: &mut impl Host,
    ph: Phoneme,
    slot: Option<usize>,
    ipa: bool,
    stress: bool,
) -> Result<Text, Error> {
    let mut text = Text {
        bytes: [0; NAME_BYTES],
        len: 0,
        flags: None,
    };
    let mut writer = Writer {
        output: &mut text.bytes,
        len: 0,
    };
    if stress {
        writer.stress(host.stress_node(slot).ok_or(Error::Context)?, ipa)?;
    }
    if ph.code == 15 {
        text.len = writer.len;
        return Ok(text);
    }
    if ph.code == 21 {
        let table = host.table_code(slot).ok_or(Error::Context)?;
        let name = host.table_name(table).ok_or(Error::Context)?;
        let len = name.iter().position(|c| *c == 0).ok_or(Error::Terminator)?;
        writer.bytes(b"(")?;
        writer.bytes(&name[..len])?;
        writer.bytes(b")")?;
        text.len = writer.len;
        return Ok(text);
    }
    if ipa {
        let mut name = [0; 18];
        host.ipa(ph.code, slot, &mut name)?;
        if name[0] == b' ' {
            text.len = writer.len;
            return Ok(text);
        }
        let start = usize::from(name[0] != 0 && name[0] < 32);
        if start != 0 {
            text.flags = Some(i32::from(name[0]));
        }
        let length = name[start..]
            .iter()
            .position(|b| *b == 0)
            .ok_or(Error::Terminator)?;
        if length != 0 {
            writer.bytes(&name[start..start + length])?;
            text.len = writer.len;
            return Ok(text);
        }
    }
    let mut first = true;
    for (index, byte) in ph.mnemonic.to_le_bytes().into_iter().enumerate() {
        if byte == 0 || byte == b'/' {
            break;
        }
        if ipa {
            if (first && byte == b'_') || (byte == b'#' && ph.kind == 2) {
                break;
            }
            if !first && byte.is_ascii_digit() {
                continue;
            }
            writer.code(if (32..128).contains(&byte) {
                u32::from(IPA[usize::from(byte - 32)])
            } else {
                u32::from(byte)
            })?;
        } else {
            writer.bytes(&[byte])?;
        }
        first = false;
        // Signed C right-shift would repeat0xff forever after four nonzero
        // bytes with the high bit set. Reject that nonterminating domain.
        if index == 3 && ph.mnemonic & 0x8000_0000 != 0 {
            return Err(Error::Mnemonic);
        }
    }
    text.len = writer.len;
    Ok(text)
}

fn decoration(code: u32) -> bool {
    (0x2b0..=0x36f).contains(&code)
}
/// Assembly uses owned node scratch and then appends its exact prefix; no
/// global owner or foreign output borrow crosses interpreter effects.
pub fn clause(host: &mut impl Host, mode: i32, output: &mut [u8]) -> Result<usize, Error> {
    let ipa = mode & 2 != 0;
    let (tie, separator) = if mode & 0x80 != 0 {
        ((mode >> 8) as u32, 0)
    } else {
        (0, (mode >> 8) as u32)
    };
    let mut destination = Writer { output, len: 0 };
    let mut index = 1;
    loop {
        let count = host.count()?;
        if count > MAX_LIST {
            return Err(Error::Context);
        }
        if index >= count.saturating_sub(2) {
            break;
        }
        let slot = Some(index);
        let ph = host.phoneme(slot).ok_or(Error::Context)?;
        let name = mnemonic(host, ph, slot, ipa, false)?;
        let node = host.node(slot).ok_or(Error::Context)?;
        let mut scratch = [0; NODE_BYTES];
        let mut writer = Writer {
            output: &mut scratch,
            len: 0,
        };
        if node.new_word & 1 != 0 && node.new_word & 12 == 0 {
            writer.bytes(b" ")?;
        }
        if (node.new_word == 0 || separator == 32)
            && separator != 0
            && index > 1
            && !decoration(crate::word_key::decode(name.terminated(), 0).0)
        {
            writer.code(separator)?;
        }
        writer.stress(node, ipa)?;
        // Legacy resets flags to0 before this loop, so IPA flag bits do not
        // suppress ties here. Preserve that behavior without its shift UB.
        let mut cursor = 0;
        let mut count = 0;
        while cursor < name.bytes().len() {
            let (code, width) = crate::word_key::decode(name.terminated(), cursor);
            if tie != 0 && count > 0 && !decoration(code) && host.alpha(code) {
                writer.code(tie)?;
            }
            writer.code(code)?;
            cursor += width;
            count += 1;
            // Forward UTF-8 decoding skips stray continuation bytes. A run at
            // the tail decodes terminal NUL; preserve its embedded-NUL write.
        }
        if host.phoneme(slot).ok_or(Error::Context)?.code != 21 {
            if host.node(slot).ok_or(Error::Context)?.flags & 8 != 0 {
                let ph = host.special(12).ok_or(Error::Context)?;
                writer.bytes(mnemonic(host, ph, slot, ipa, false)?.bytes())?;
            }
            let node = host.node(slot).ok_or(Error::Context)?;
            if node.flags & 4 != 0 && node.kind != 2 {
                let ph = host.special(20).ok_or(Error::Context)?;
                writer.bytes(mnemonic(host, ph, slot, ipa, false)?.bytes())?;
            }
            if host.node(slot).ok_or(Error::Context)?.tone > 0 {
                if let Some(ph) = host.tone(slot) {
                    writer.bytes(mnemonic(host, ph, slot, ipa, false)?.bytes())?;
                }
            }
        }
        let length = writer.len;
        // C strcpy copies only through the first NUL, yet advances by the
        // complete scratch length. Preserve that case and unused destination
        // bytes without reading or clearing the destination tail.
        let copied = scratch[..length]
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(length);
        let end = destination.len.checked_add(length).ok_or(Error::Capacity)?;
        if end >= destination.output.len() {
            return Err(Error::Capacity);
        }
        destination.output[destination.len..destination.len + copied]
            .copy_from_slice(&scratch[..copied]);
        destination.output[destination.len + copied] = 0;
        destination.len = end;
        index += 1;
    }
    *destination
        .output
        .get_mut(destination.len)
        .ok_or(Error::Capacity)? = 0;
    Ok(destination.len)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ph(name: &[u8], code: u8) -> Phoneme {
        Phoneme {
            mnemonic: crate::phoneme::mnemonic(name),
            code,
            kind: 2,
            ..Phoneme::default()
        }
    }
    struct Fixture {
        node: Node,
        name: [u8; 18],
        count: usize,
        calls: usize,
        refresh: bool,
    }
    impl Host for Fixture {
        fn count(&self) -> Result<usize, Error> {
            Ok(self.count)
        }
        fn node(&self, _: Option<usize>) -> Option<Node> {
            Some(self.node)
        }
        fn special(&self, code: u8) -> Option<Phoneme> {
            Some(ph(if code == 12 { b":" } else { b"=" }, code))
        }
        fn table_name(&self, _: u8) -> Option<[u8; 32]> {
            let mut name = [0; 32];
            name[..2].copy_from_slice(b"en");
            Some(name)
        }
        fn tone(&self, _: Option<usize>) -> Option<Phoneme> {
            Some(ph(b"T", 55))
        }
        fn ipa(&mut self, _: u8, _: Option<usize>, out: &mut [u8; 18]) -> Result<(), Error> {
            self.calls += 1;
            *out = self.name;
            if self.refresh {
                self.node.flags = 4;
                self.node.stress = 5;
                self.node.new_word = 1;
                self.node.kind = 2;
                self.node.phoneme = Some(ph(b"switch", 21));
            }
            Ok(())
        }
        fn alpha(&self, code: u32) -> bool {
            char::from_u32(code).is_some_and(char::is_alphabetic)
        }
    }
    fn fixture() -> Fixture {
        Fixture {
            node: Node {
                phoneme: Some(ph(b"ts", 40)),
                flags: 4,
                stress: 3,
                kind: 2,
                ..Node::default()
            },
            name: [0; 18],
            count: 4,
            calls: 0,
            refresh: false,
        }
    }
    #[test]
    fn flags_empty_name_fallback_and_stress_end_word() {
        let mut host = fixture();
        host.name[0] = 7;
        let text = mnemonic(&mut host, ph(b"ts/", 40), None, true, true).unwrap();
        assert_eq!(text.bytes(), "ˌts".as_bytes());
        assert_eq!(text.flags(), Some(7));
        let text = mnemonic(&mut host, ph(b"ignored", 15), None, false, true).unwrap();
        assert_eq!(text.bytes(), b",");
        assert_eq!(text.flags(), None);
        assert_eq!(host.calls, 1);
        host.name[0] = b' ';
        assert_eq!(
            mnemonic(&mut host, ph(b"a", 40), None, true, false)
                .unwrap()
                .bytes(),
            b""
        );
    }
    #[test]
    fn fallback_variants_digits_vowels_pauses_and_signed_nontermination() {
        let mut host = fixture();
        assert_eq!(
            mnemonic(&mut host, ph(b"t12/", 40), None, true, false)
                .unwrap()
                .bytes(),
            b"t"
        );
        assert_eq!(
            mnemonic(&mut host, ph(b"_:", 40), None, true, false)
                .unwrap()
                .bytes(),
            b""
        );
        assert_eq!(
            mnemonic(&mut host, ph(b"a#", 40), None, true, false)
                .unwrap()
                .bytes(),
            b"a"
        );
        assert!(matches!(
            mnemonic(
                &mut host,
                ph(&[b'a', b'b', b'c', 255], 40),
                None,
                false,
                false
            ),
            Err(Error::Mnemonic)
        ));
    }
    #[test]
    fn clause_reads_fresh_metadata_after_ipa_and_ignores_tie_flags() {
        let mut host = fixture();
        host.name[..3].copy_from_slice(&[7, b't', b's']);
        host.refresh = true;
        let mut out = [0xaa; 64];
        let length = clause(&mut host, 2 | 0x80 | (0x361 << 8), &mut out).unwrap();
        assert_eq!(&out[..length], " ˈt͡s".as_bytes());
        assert_eq!(out[length], 0);
        assert_eq!(host.calls, 1);
    }
    #[test]
    fn clause_markers_stress_length_and_tone_and_capacity_guard() {
        let mut host = fixture();
        host.node.flags = 12;
        host.node.kind = 6;
        host.node.tone = 4;
        let mut out = [0; 64];
        let length = clause(&mut host, 0, &mut out).unwrap();
        assert_eq!(&out[..length], b",ts:=T");
        let mut short = [0; 2];
        assert_eq!(clause(&mut host, 0, &mut short), Err(Error::Capacity));
        host.count = MAX_LIST + 1;
        assert_eq!(clause(&mut host, 0, &mut out), Err(Error::Context));
    }
    #[test]
    fn terminated_ipa_and_all_stress_levels() {
        let mut host = fixture();
        host.name = [b'a'; 18];
        assert!(matches!(
            mnemonic(&mut host, ph(b"a", 40), None, true, false),
            Err(Error::Terminator)
        ));
        host.name = [0; 18];
        for level in 0..=255 {
            host.node.stress = level;
            let text = mnemonic(&mut host, ph(b"a", 40), None, false, true).unwrap();
            let expected = if level < 2 {
                b"a".as_slice()
            } else if level < 4 {
                b",a"
            } else {
                b"'a"
            };
            assert_eq!(text.bytes(), expected);
        }
    }
    #[test]
    fn permissive_continuation_tail_preserves_previous_destination_bytes() {
        let mut host = fixture();
        host.node.flags = 0;
        host.node.phoneme = Some(ph(b"abcd", 40));
        let mut out = [0xaa; 8];
        assert_eq!(clause(&mut host, 0, &mut out), Ok(4));
        host.node.phoneme = Some(ph(&[b't', 0x80, 0x80], 40));
        assert_eq!(clause(&mut host, 0, &mut out), Ok(3));
        assert_eq!(&out[..5], &[b't', 0, b'c', 0, 0]);
    }
}
