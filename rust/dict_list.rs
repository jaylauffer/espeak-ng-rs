//! Complete dictionary-list entry parsing, record ownership and bucket framing.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    dictionary, suffix, utf8,
    voice_reader::{Reader, TextMode},
};
use std::{io::Cursor, ops::Range};
pub const MAX_BYTES: usize = 200 * 1024 * 1024;
const EMPTY: usize = usize::MAX;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Resource,
    Io,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct State {
    pub line: i32,
    pub text_mode: bool,
    pub need_dictionary: i32,
}
pub struct Environment<'a> {
    pub alnums: &'a [u8; 256],
    pub language_text: bool,
}
pub trait Host {
    fn encode(&mut self, input: &[u8], output: &mut [u8; 200]) -> Option<i32>;
    fn translate(&mut self, input: &mut [u8], output: &mut [u8; 200]);
    fn case(&mut self, code: u32) -> Option<u32>;
    fn transpose(&mut self, word: &mut [u8]) -> Option<usize>;
    fn report(&mut self, state: &State, kind: i32, value: i32, first: &[u8], second: &[u8]);
    fn write(&mut self, bytes: &[u8]) -> bool;
}
pub struct Record {
    pub bytes: [u8; 256],
    pub length: usize,
    pub hash: usize,
}
fn space(c: u8) -> bool {
    c != 0 && c <= b' '
}
fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn terminated(bytes: &[u8]) -> Result<&[u8], Error> {
    let n = bytes.iter().position(|b| *b == 0).ok_or(Error::Bounds)?;
    Ok(&bytes[..=n])
}
fn flag(flags: &mut [u8; 100], count: &mut usize, value: u8) -> Result<(), Error> {
    *flags.get_mut(*count).ok_or(Error::Bounds)? = value;
    *count += 1;
    Ok(())
}
fn keyword(text: &[u8]) -> Option<u8> {
    const NAMES: &[(&[u8], u8)] = &[
        (b"$1", 0x41),
        (b"$2", 0x42),
        (b"$3", 0x43),
        (b"$4", 0x44),
        (b"$5", 0x45),
        (b"$6", 0x46),
        (b"$7", 0x47),
        (b"$u", 0x48),
        (b"$u1", 0x49),
        (b"$u2", 0x4a),
        (b"$u3", 0x4b),
        (b"$u+", 0x4c),
        (b"$u1+", 0x4d),
        (b"$u2+", 0x4e),
        (b"$u3+", 0x4f),
        (b"$pause", 8),
        (b"$strend", 9),
        (b"$strend2", 10),
        (b"$unstressend", 11),
        (b"$accent_before", 12),
        (b"$abbrev", 13),
        (b"$double", 14),
        (b"$alt", 15),
        (b"$alt1", 15),
        (b"$alt2", 16),
        (b"$alt3", 17),
        (b"$alt4", 18),
        (b"$alt5", 19),
        (b"$alt6", 20),
        (b"$alt7", 21),
        (b"$combine", 23),
        (b"$dot", 24),
        (b"$hasdot", 25),
        (b"$max3", 27),
        (b"$brk", 28),
        (b"$text", 29),
        (b"$verbf", 0x20),
        (b"$verbsf", 0x21),
        (b"$nounf", 0x22),
        (b"$pastf", 0x23),
        (b"$verb", 0x24),
        (b"$noun", 0x25),
        (b"$past", 0x26),
        (b"$verbextend", 0x28),
        (b"$capital", 0x29),
        (b"$allcaps", 0x2a),
        (b"$accent", 0x2b),
        (b"$sentence", 0x2d),
        (b"$only", 0x2e),
        (b"$onlys", 0x2f),
        (b"$stem", 0x30),
        (b"$atend", 0x31),
        (b"$atstart", 0x32),
        (b"$native", 0x33),
        (b"$?", 100),
        (b"$textmode", 200),
        (b"$phonememode", 201),
    ];
    NAMES
        .iter()
        .find_map(|(name, value)| (*name == text).then_some(*value))
}
fn character_code(bytes: &[u8]) -> Option<u32> {
    if !bytes.starts_with(b"U+") {
        return None;
    }
    let mut p = 2;
    while bytes.get(p).is_some_and(u8::is_ascii_whitespace) {
        p += 1;
    }
    let negative = bytes.get(p) == Some(&b'-');
    if negative || bytes.get(p) == Some(&b'+') {
        p += 1;
    }
    if bytes
        .get(p..p + 2)
        .is_some_and(|s| s == b"0x" || s == b"0X")
    {
        p += 2;
    }
    let mut value = 0u32;
    let mut digits = 0;
    while let Some(code) = bytes.get(p).and_then(|b| (*b as char).to_digit(16)) {
        value = value.wrapping_mul(16).wrapping_add(code);
        p += 1;
        digits += 1;
    }
    (digits != 0).then_some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}
/// The entire initialized source extent is owned. Prior initialized tails may
/// be observable after parser writes; no callback borrows foreign line storage.
pub fn compile_line(
    host: &mut impl Host,
    env: &Environment<'_>,
    state: &mut State,
    source: &mut [u8],
) -> Result<Option<Record>, Error> {
    let result = compile_inner(host, env, state, source);
    if result.is_err() {
        host.report(state, 8, 0, b"\0", b"\0");
    }
    result
}
fn compile_inner(
    host: &mut impl Host,
    env: &Environment<'_>,
    state: &mut State,
    line: &mut [u8],
) -> Result<Option<Record>, Error> {
    if line.len() > 200 || !line.contains(&0) {
        return Err(Error::Bounds);
    }
    let mut p = 0;
    let mut c = line[0];
    let mut step = 0;
    let mut word = None;
    let mut phonetic = None;
    let mut phonetic_end = 0;
    let mut multi = 0;
    let mut numeric_hyphen = false;
    let mut multi_start = None;
    let mut multi_end = 0;
    let mut flags = [0; 100];
    let mut flag_count = 0;
    let mut text = false;
    while c != b'\n' && c != 0 {
        c = *line.get(p).ok_or(Error::Bounds)?;
        if c == b'?' && step == 0 {
            p += 1;
            let mut offset = 100;
            if line.get(p) == Some(&b'!') {
                offset = 132;
                p += 1;
            }
            let mut n = 0;
            if line.get(p).is_some_and(u8::is_ascii_digit) {
                n = line[p] - b'0';
                p += 1;
            }
            if line.get(p).is_some_and(u8::is_ascii_digit) {
                n = n * 10 + line[p] - b'0';
                p += 1;
            }
            flag(&mut flags, &mut flag_count, n.wrapping_add(offset))?;
            c = *line.get(p).ok_or(Error::Bounds)?;
        }
        if c == b'$'
            && line
                .get(p + 1)
                .is_some_and(|b| env.alnums[*b as usize] != 0)
        {
            let start = p;
            loop {
                c = *line.get(p).ok_or(Error::Bounds)?;
                if space(c) {
                    break;
                }
                p += 1;
            }
            line[p] = 0;
            match keyword(string(&line[start..p])) {
                Some(200) => state.text_mode = true,
                Some(201) => state.text_mode = false,
                Some(29) => text = true,
                Some(value) => flag(&mut flags, &mut flag_count, value)?,
                None => host.report(state, 1, 0, &line[start..=p], b"\0"),
            }
        }
        if c == b'/' && line.get(p + 1) == Some(&b'/') && multi == 0 {
            c = b'\n';
        }
        match step {
            0 => {
                if c == b'(' {
                    multi = 1;
                    word = Some(p + 1);
                    step = 1;
                } else if !space(c) {
                    word = Some(p);
                    step = 1;
                }
            }
            1 => {
                if c == b'-' && multi != 0 {
                    numeric_hyphen |= word.is_some_and(|w| line[w].is_ascii_digit());
                    flag(&mut flags, &mut flag_count, 0x2c)?;
                    c = b' ';
                }
                if space(c) {
                    line[p] = 0;
                    if multi != 0 {
                        multi_start = Some(p + 1);
                        multi_end = p + 1;
                        step = 2;
                    } else {
                        step = 3;
                    }
                } else if c == b')' {
                    if multi != 0 {
                        line[p] = 0;
                        multi = 0;
                        step = 3;
                    } else if word.is_some_and(|w| line[w] != b'_') {
                        host.report(state, 2, 0, b"\0", b"\0");
                        step = 3;
                    }
                }
            }
            2 => {
                if space(c) {
                    multi += 1;
                } else if c == b')' {
                    line[p] = b' ';
                    multi_end = p + 1;
                    step = 3;
                }
            }
            3 => {
                if !space(c) {
                    phonetic = Some(p);
                    step = 4;
                }
            }
            4 => {
                if space(c) {
                    phonetic_end = p;
                    line[p] = 0;
                    step = 5;
                }
            }
            _ => {
                if !space(c) {
                    line[phonetic_end] = b' ';
                    step = 4;
                }
            }
        }
        p += 1;
    }
    let Some(w) = word.filter(|w| line[*w] != 0) else {
        return Ok(None);
    };
    let mut key = [0; 204];
    let tail = line.get(w..).ok_or(Error::Bounds)?;
    key[..tail.len()].copy_from_slice(tail);
    let mut phon = [0; 204];
    if let Some(start) = phonetic {
        let bytes = terminated(&line[start..])?;
        phon[..bytes.len()].copy_from_slice(bytes);
    }
    text |= state.text_mode;
    let mut encoded = [0; 200];
    if text {
        if key[0] == b'_' {
            let n = string(&phon).len();
            if n + 2 > phon.len() {
                return Err(Error::Bounds);
            }
            phon[n] = b' ';
            phon[n + 1] = 0;
            host.translate(&mut phon, &mut encoded);
            text = false;
            if encoded[0] == 0 && state.need_dictionary < 3 {
                state.need_dictionary += 1;
                host.report(state, 3, 0, b"\0", b"\0");
            }
        } else {
            let n = string(&phon).len().min(155);
            encoded[..n].copy_from_slice(&phon[..n]);
        }
    } else {
        let Some(bad) = host.encode(terminated(&phon)?, &mut encoded) else {
            host.report(state, 4, 0, b"\0", b"\0");
            return Ok(None);
        };
        let coded = terminated(&encoded)?;
        if coded.contains(&21) {
            flag(&mut flags, &mut flag_count, 0x2f)?;
        }
        if bad != 0 {
            host.report(state, 5, bad, terminated(&key)?, terminated(&phon)?);
        }
    }
    if text != env.language_text {
        flag(&mut flags, &mut flag_count, 29)?;
    }
    if let Some(code) = character_code(string(&key)) {
        let (bytes, width) = suffix::encode(code);
        key[..width].copy_from_slice(&bytes[..width]);
        key[width] = 0;
    } else if key[0] != b'_' {
        let mut upper = true;
        let mut cursor = 0;
        loop {
            let ch = utf8::decode(&key, cursor, false).map_err(|_| Error::Bounds)?;
            if ch.code == 0 {
                break;
            }
            if let Some(lower) = host.case(ch.code) {
                let (bytes, width) = suffix::encode(lower);
                key.get_mut(cursor..cursor + width)
                    .ok_or(Error::Bounds)?
                    .copy_from_slice(&bytes[..width]);
            } else {
                upper = false;
            }
            cursor += ch.width;
        }
        if upper {
            flag(&mut flags, &mut flag_count, 0x2a)?;
        }
    }
    let mut input = 0;
    let mut output = 0;
    while *key.get(input).ok_or(Error::Bounds)? != 0 {
        let ch = utf8::decode(&key, input, false).map_err(|_| Error::Bounds)?;
        let end = input
            .checked_add(ch.width)
            .filter(|n| *n <= key.len())
            .ok_or(Error::Bounds)?;
        if ch.code != 0xfe0e && ch.code != 0xfe0f {
            key.copy_within(input..end, output);
            output += ch.width;
        }
        input = end;
    }
    *key.get_mut(output).ok_or(Error::Bounds)? = 0;
    if key[0] == 0 {
        return Ok(None);
    }
    let descriptor = host.transpose(&mut key).ok_or(Error::Bounds)?;
    // Lowercase may widen a legacy character (Turkish I) and overwrite the
    // word delimiter. The old in-place normalization then also changes the
    // initialized multiword tail. Retain those mutations before packing it.
    let tail_length = line.len() - w;
    line[w..].copy_from_slice(&key[..tail_length]);
    let hash = dictionary::hash(&key);
    let word_length = descriptor & 0x3f;
    let mut record = Record {
        bytes: [0; 256],
        length: word_length + 2,
        hash,
    };
    record.bytes[1] = descriptor as u8;
    record.bytes[2..2 + word_length].copy_from_slice(&key[..word_length]);
    let phon_length = terminated(&encoded)?.len() - 1;
    if phon_length == 0 {
        record.bytes[1] |= 0x80;
    } else {
        let length = word_length + phon_length + 3;
        if length < 256 {
            record.bytes[2 + word_length..length].copy_from_slice(&encoded[..=phon_length]);
            record.length = length;
        } else {
            host.report(state, 6, length as i32, b"\0", b"\0");
            record.bytes[1] |= 0x80;
        }
    }
    append(&mut record, &flags[..flag_count])?;
    if let Some(start) = multi_start.filter(|_| multi > 0) {
        if multi > 10 {
            host.report(state, 7, multi, b"\0", b"\0");
        } else {
            append(&mut record, &[80 + multi as u8])?;
            if numeric_hyphen {
                append(&mut record, b" ")?;
            }
            append(
                &mut record,
                line.get(start..multi_end).ok_or(Error::Bounds)?,
            )?;
        }
    }
    record.bytes[0] = record.length as u8;
    Ok(Some(record))
}
fn append(record: &mut Record, bytes: &[u8]) -> Result<(), Error> {
    let end = record
        .length
        .checked_add(bytes.len())
        .filter(|n| *n < 256)
        .ok_or(Error::Bounds)?;
    record.bytes[record.length..end].copy_from_slice(bytes);
    record.length = end;
    Ok(())
}
struct Node {
    span: Range<usize>,
    next: usize,
}
pub struct Workspace {
    bytes: Vec<u8>,
    nodes: Vec<Node>,
    heads: [usize; 1024],
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            nodes: Vec::new(),
            heads: [EMPTY; 1024],
        }
    }
}
fn reserve<T>(data: &mut Vec<T>, count: usize, limit: usize) -> Result<(), Error> {
    let n = data
        .len()
        .checked_add(count)
        .filter(|n| *n <= limit)
        .ok_or(Error::Resource)?;
    if n > data.capacity() {
        data.try_reserve_exact(n.next_power_of_two().min(limit) - data.len())
            .map_err(|_| Error::Resource)?;
    }
    Ok(())
}
impl Workspace {
    pub fn reset(&mut self) {
        self.bytes.clear();
        self.nodes.clear();
        self.heads.fill(EMPTY);
    }
    pub fn source(
        &mut self,
        source: &[u8],
        mode: TextMode,
        env: &Environment<'_>,
        state: &mut State,
        host: &mut impl Host,
    ) -> Result<usize, Error> {
        state.line = 0;
        state.text_mode = false;
        let mut reader = Reader::new(Cursor::new(source), 200, mode).map_err(|_| Error::Io)?;
        let mut count = 0;
        let mut line = [0; 200];
        let mut initialized = 0;
        while let Some(chunk) = reader.next_chunk().map_err(|_| Error::Io)? {
            let n = chunk.len();
            line[..n].copy_from_slice(chunk);
            line[n] = 0;
            initialized = initialized.max(n + 1);
            // Preserve the C long-line discard: only the admitted first chunk
            // is parsed. A trailing comment cannot become a second dictionary key.
            let logical = string(chunk);
            if !logical.is_empty() && logical.last() != Some(&b'\n') {
                while let Some(rest) = reader.next_chunk().map_err(|_| Error::Io)? {
                    if rest.last() == Some(&b'\n') {
                        break;
                    }
                }
            }
            state.line = state.line.checked_add(1).ok_or(Error::Bounds)?;
            if let Some(record) = compile_line(host, env, state, &mut line[..initialized])? {
                reserve(&mut self.bytes, record.length, MAX_BYTES)?;
                reserve(&mut self.nodes, 1, MAX_BYTES / std::mem::size_of::<Node>())?;
                let start = self.bytes.len();
                self.bytes.extend_from_slice(&record.bytes[..record.length]);
                self.nodes.push(Node {
                    span: start..self.bytes.len(),
                    next: self.heads[record.hash],
                });
                self.heads[record.hash] = self.nodes.len() - 1;
                count += 1;
            }
        }
        Ok(count)
    }
    pub fn write(&self, host: &mut impl Host) -> Result<(), Error> {
        for &head in &self.heads {
            let mut node = head;
            while node != EMPTY {
                let entry = &self.nodes[node];
                if !host.write(&self.bytes[entry.span.clone()]) {
                    return Err(Error::Io);
                }
                node = entry.next;
            }
            if !host.write(&[0]) {
                return Err(Error::Io);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        diagnostics: Vec<i32>,
        bytes: Vec<u8>,
        translations: usize,
        refuse: bool,
        dotless: bool,
    }
    impl Host for Effects {
        fn encode(&mut self, input: &[u8], output: &mut [u8; 200]) -> Option<i32> {
            if input[0] != 0 {
                output[0] = if input.starts_with(b"switch") { 21 } else { 7 };
            }
            Some(0)
        }
        fn translate(&mut self, _: &mut [u8], _: &mut [u8; 200]) {
            self.translations += 1;
        }
        fn case(&mut self, code: u32) -> Option<u32> {
            if self.dotless && code == u32::from(b'I') {
                return Some(0x131);
            }
            (code <= 127 && (code as u8).is_ascii_uppercase()).then_some(code + 32)
        }
        fn transpose(&mut self, word: &mut [u8]) -> Option<usize> {
            Some(string(word).len())
        }
        fn report(&mut self, _: &State, kind: i32, _: i32, _: &[u8], _: &[u8]) {
            self.diagnostics.push(kind);
        }
        fn write(&mut self, bytes: &[u8]) -> bool {
            if self.refuse {
                false
            } else {
                self.bytes.extend_from_slice(bytes);
                true
            }
        }
    }
    fn alnums() -> [u8; 256] {
        std::array::from_fn(|i| u8::from((i as u8).is_ascii_alphanumeric()))
    }
    fn line(input: &[u8], host: &mut Effects, state: &mut State) -> Result<Option<Record>, Error> {
        let mut source = input.to_vec();
        source.push(0);
        compile_line(
            host,
            &Environment {
                alnums: &alnums(),
                language_text: false,
            },
            state,
            &mut source,
        )
    }
    #[test]
    fn conditional_case_language_switch_and_numeric_multiword_records() {
        let mut host = Effects::default();
        let mut state = State::default();
        let record = line(b"?!12 CAPS switch\n", &mut host, &mut state)
            .unwrap()
            .unwrap();
        assert_eq!(
            &record.bytes[..record.length],
            &[11, 4, b'c', b'a', b'p', b's', 21, 0, 144, 0x2f, 0x2a][..]
        );
        let record = line(b"(2-three) a\n", &mut host, &mut state)
            .unwrap()
            .unwrap();
        assert_eq!(
            &record.bytes[record.length - 9..record.length],
            b"\x2c\x51 three "
        );
        assert!(host.diagnostics.is_empty());
    }
    #[test]
    fn file_text_modes_special_words_and_three_retry_reports() {
        let mut host = Effects::default();
        let mut state = State::default();
        let mut workspace = Workspace::default();
        workspace.source(b"$textmode\nword replacement\n_special word\n_special word\n_special word\n_special word\n",TextMode::Binary,&Environment{alnums:&alnums(),language_text:false},&mut state,&mut host).unwrap();
        assert_eq!(host.translations, 4);
        assert_eq!(state.need_dictionary, 3);
        assert_eq!(host.diagnostics, [3, 3, 3]);
        assert!(state.text_mode);
        workspace
            .source(
                b"word a\n",
                TextMode::Binary,
                &Environment {
                    alnums: &alnums(),
                    language_text: false,
                },
                &mut state,
                &mut host,
            )
            .unwrap();
        assert!(!state.text_mode);
        assert_eq!(state.line, 1);
    }
    #[test]
    fn variation_selectors_and_unicode_character_keys() {
        let mut host = Effects::default();
        let mut state = State::default();
        assert!(line("️ a\n".as_bytes(), &mut host, &mut state)
            .unwrap()
            .is_none());
        let record = line(b"U+1f600 a\n", &mut host, &mut state)
            .unwrap()
            .unwrap();
        assert_eq!(&record.bytes[2..6], "😀".as_bytes());
        let record = line("😀️ a\n".as_bytes(), &mut host, &mut state)
            .unwrap()
            .unwrap();
        assert_eq!(record.bytes[1], 4);
        assert_eq!(&record.bytes[2..6], "😀".as_bytes());
    }
    #[test]
    fn empty_special_replacement_uses_owned_writable_translation_input() {
        let mut host = Effects::default();
        let mut state = State {
            text_mode: true,
            ..State::default()
        };
        let record = line(b"_special  $onlys\n", &mut host, &mut state)
            .unwrap()
            .unwrap();
        assert_eq!(host.translations, 1);
        assert_eq!(state.need_dictionary, 1);
        assert_eq!(host.diagnostics, [3]);
        assert_eq!(record.bytes[1], 0x88);
    }
    #[test]
    fn widening_lowercase_preserves_mutated_multiword_tail() {
        let mut host = Effects {
            dotless: true,
            ..Effects::default()
        };
        let mut state = State::default();
        let record = line(b"(I TWO) a\n", &mut host, &mut state)
            .unwrap()
            .unwrap();
        assert_eq!(&record.bytes[record.length - 5..record.length], b"\x51two ");
        assert!(host.diagnostics.is_empty());
    }
    #[test]
    fn duplicate_buckets_reverse_insertion_and_reuse_arenas() {
        let mut host = Effects::default();
        let mut state = State::default();
        let mut workspace = Workspace::default();
        let env = Environment {
            alnums: &alnums(),
            language_text: false,
        };
        let input = b"word a\nword b $pause\n";
        workspace
            .source(input, TextMode::Binary, &env, &mut state, &mut host)
            .unwrap();
        workspace.write(&mut host).unwrap();
        let bucket = dictionary::hash(b"word");
        assert_eq!(host.bytes[bucket], 9);
        assert_eq!(host.bytes[bucket + 8], 8);
        assert_eq!(host.bytes[bucket + 9], 8);
        let bytes = workspace.bytes.as_ptr();
        let nodes = workspace.nodes.as_ptr();
        let expected = host.bytes.clone();
        for _ in 0..10 {
            workspace.reset();
            host.bytes.clear();
            workspace
                .source(input, TextMode::Binary, &env, &mut state, &mut host)
                .unwrap();
            workspace.write(&mut host).unwrap();
            assert_eq!(host.bytes, expected);
            assert_eq!(workspace.bytes.as_ptr(), bytes);
            assert_eq!(workspace.nodes.as_ptr(), nodes);
        }
    }
    #[test]
    fn windows_text_and_long_line_discard_cannot_create_spurious_entries() {
        let mut source = b"word a // ".to_vec();
        source.resize(650, b'x');
        source.extend_from_slice(b"\r\n7 b\r\nlast a\x1aignored a\n");
        let mut host = Effects::default();
        let mut state = State::default();
        let mut workspace = Workspace::default();
        assert_eq!(
            workspace.source(
                &source,
                TextMode::Windows,
                &Environment {
                    alnums: &alnums(),
                    language_text: false
                },
                &mut state,
                &mut host
            ),
            Ok(3)
        );
        assert_eq!(state.line, 3);
    }
    #[test]
    fn invalid_bounds_unterminated_keyword_and_writer_errors_are_refused() {
        let mut host = Effects::default();
        let mut state = State::default();
        assert!(line(b"word a $text", &mut host, &mut state).is_err());
        let mut unterminated = [b'x'; 200];
        assert!(compile_line(
            &mut host,
            &Environment {
                alnums: &alnums(),
                language_text: false
            },
            &mut state,
            &mut unterminated
        )
        .is_err());
        let mut workspace = Workspace::default();
        workspace
            .source(
                b"word a\n",
                TextMode::Binary,
                &Environment {
                    alnums: &alnums(),
                    language_text: false,
                },
                &mut state,
                &mut host,
            )
            .unwrap();
        host.refuse = true;
        assert_eq!(workspace.write(&mut host), Err(Error::Io));
        let mut bytes = vec![0u8];
        assert_eq!(
            reserve(&mut bytes, MAX_BYTES, MAX_BYTES),
            Err(Error::Resource)
        );
    }
}
