//! Complete bounded dictionary rule-file compiler and group bytecode packing.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    rule_compile,
    voice_reader::{Reader, TextMode},
};
use std::{io::Cursor, ops::Range};
pub const MAX_RULES: usize = 3000;
pub const MAX_GROUPS: usize = 120;
pub const MAX_BYTES: usize = 200 * 1024 * 1024;

pub trait Host: rule_compile::Host {
    fn context(&mut self, line: i32, group: &[u8; 13]);
    fn report(&mut self, kind: i32, value: i32, text: &[u8]);
    fn write(&mut self, bytes: &[u8]) -> bool;
    fn position(&mut self) -> Option<u64>;
}
pub struct Environment<'a> {
    pub letters: &'a [u8; 95],
    pub spaces: &'a [u8; 256],
    pub debug: bool,
    pub signed_char: bool,
    pub letter_offset: i32,
    pub text_mode: TextMode,
}
#[derive(Clone)]
pub struct State {
    pub line: i32,
    pub group: [u8; 13],
    pub group3: i32,
    pub letters: [u8; 95],
    pub rules: i32,
    pub groups: i32,
    pub groups3: i32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            line: 0,
            group: [0; 13],
            group3: 0,
            letters: [0; 95],
            rules: 0,
            groups: 0,
            groups3: 0,
        }
    }
}
#[derive(Clone)]
struct Record {
    whole: Range<usize>,
    phon: Range<usize>,
    body: Range<usize>,
}
struct Group {
    name: [u8; 13],
    group3: i32,
    packed: Range<usize>,
    index: usize,
}
#[derive(Default)]
pub struct Workspace {
    records: Vec<u8>,
    order: Vec<Record>,
    groups: Vec<Group>,
    packed: Vec<u8>,
}
#[derive(Clone, Copy, Debug)]
pub enum Error {
    Resource,
    Io,
}
fn reserve<T>(data: &mut Vec<T>, count: usize, limit: usize) -> Result<(), Error> {
    let needed = data
        .len()
        .checked_add(count)
        .filter(|&n| n <= limit)
        .ok_or(Error::Resource)?;
    if needed > data.capacity() {
        let target = needed.next_power_of_two().min(limit);
        data.try_reserve_exact(target - data.len())
            .map_err(|_| Error::Resource)?;
    }
    Ok(())
}

fn push_bytes(data: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Error> {
    reserve(data, bytes.len(), MAX_BYTES)?;
    data.extend_from_slice(bytes);
    Ok(())
}
fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
}
fn write(host: &mut impl Host, bytes: &[u8]) -> Result<(), Error> {
    if host.write(bytes) {
        Ok(())
    } else {
        Err(Error::Io)
    }
}
fn terminate<const N: usize>(bytes: &[u8]) -> [u8; N] {
    let mut out = [0; N];
    out[..bytes.len().min(N - 1)].copy_from_slice(&bytes[..bytes.len().min(N - 1)]);
    out
}

impl Workspace {
    fn record(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let first = bytes.iter().position(|&b| b == 0).ok_or(Error::Resource)?;
        let second = bytes[first + 1..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::Resource)?
            + first
            + 1;
        reserve(&mut self.order, 1, MAX_RULES)?;
        reserve(
            &mut self.records,
            bytes.len(),
            MAX_RULES * rule_compile::OUTPUT_BYTES,
        )?;
        let start = self.records.len();
        self.records.extend_from_slice(bytes);
        self.order.push(Record {
            whole: start..start + bytes.len(),
            phon: start..start + first,
            body: start + first + 1..start + second,
        });
        Ok(())
    }
    fn flush(&mut self, state: &mut State) -> Result<(), Error> {
        if self.order.is_empty() {
            return Ok(());
        }
        reserve(&mut self.groups, 1, MAX_GROUPS)?;
        self.order.sort_unstable_by(|a, b| {
            self.records[a.phon.clone()]
                .cmp(&self.records[b.phon.clone()])
                .then_with(|| self.records[a.body.clone()].cmp(&self.records[b.body.clone()]))
        });
        let start = self.packed.len();
        let name = string(&state.group);
        let skip = if name == b"9" { 0 } else { name.len() };
        let mut common: Option<usize> = None;
        for index in 0..self.order.len() {
            let record = &self.order[index];
            let tail = record
                .body
                .start
                .checked_add(skip)
                .filter(|&n| n < record.whole.end)
                .ok_or(Error::Resource)?;
            let remainder = &self.records[tail..record.whole.end];
            let end = remainder
                .iter()
                .position(|&b| b == 0)
                .ok_or(Error::Resource)?;
            let phon = &self.records[record.phon.clone()];
            if common.is_some_and(|c| {
                !self.order[c].phon.is_empty() && self.records[self.order[c].phon.clone()] == *phon
            }) {
                push_bytes(&mut self.packed, &remainder[..end])?;
                push_bytes(&mut self.packed, &[0])?;
            } else {
                if self
                    .order
                    .get(index + 1)
                    .is_some_and(|next| self.records[next.phon.clone()] == *phon)
                {
                    push_bytes(&mut self.packed, &[4])?;
                    common = Some(index);
                }
                push_bytes(&mut self.packed, &remainder[..end])?;
                push_bytes(&mut self.packed, &[3])?;
                push_bytes(&mut self.packed, phon)?;
                push_bytes(&mut self.packed, &[0])?;
            }
        }
        let index = self.groups.len();
        self.groups.push(Group {
            name: state.group,
            group3: state.group3,
            packed: start..self.packed.len(),
            index,
        });
        state.rules += self.order.len() as i32;
        self.order.clear();
        self.records.clear();
        Ok(())
    }
    /// Reuses records/order/group/packed arenas across calls. Primitive writer
    /// effects retain legacy streaming order; an error can leave partial output.
    pub fn compile(
        &mut self,
        source: &[u8],
        env: &Environment<'_>,
        host: &mut impl Host,
    ) -> Result<State, Error> {
        self.records.clear();
        self.order.clear();
        self.groups.clear();
        self.packed.clear();
        let mut state = State {
            letters: *env.letters,
            ..State::default()
        };
        let result = self.compile_inner(source, env, host, &mut state);
        if let Err(error) = result {
            host.report(
                match error {
                    Error::Resource => 17,
                    Error::Io => 19,
                },
                0,
                b"\0",
            );
            return Err(error);
        }
        Ok(state)
    }
    fn compile_inner(
        &mut self,
        source: &[u8],
        env: &Environment<'_>,
        host: &mut impl Host,
        state: &mut State,
    ) -> Result<(), Error> {
        if source.len() > MAX_BYTES {
            return Err(Error::Resource);
        }
        let mut reader =
            Reader::new(Cursor::new(source), 500, env.text_mode).map_err(|_| Error::Io)?;
        let mut mode = 0;
        let mut limit_reported = false;
        let mut input = [0u8; 500];
        let mut initialized = 0;
        loop {
            state.line = state.line.checked_add(1).ok_or(Error::Resource)?;
            host.context(state.line, &state.group);
            let chunk = reader.next_chunk().map_err(|_| Error::Io)?;
            let eof = chunk.is_none();
            if let Some(bytes) = chunk {
                input[..bytes.len()].copy_from_slice(bytes);
                input[bytes.len()] = 0;
                initialized = initialized.max(bytes.len() + 1);
                if let Some(comment) = string(&input[..initialized])
                    .windows(2)
                    .position(|w| w == b"//")
                {
                    input[comment] = 0;
                }
            }
            let begin = usize::from(input[0] == b'\r');
            let line = if eof {
                None
            } else {
                Some(string(&input[begin..initialized]))
            };
            if line.is_none_or(|l| l.starts_with(b".")) {
                self.flush(state)?;
                limit_reported = false;
                if mode == 2 {
                    write(host, &[0, 0, 0, 0, 7])?;
                    mode = 0;
                }
                let Some(bytes) = line else {
                    break;
                };
                if bytes.starts_with(b".L") {
                    // Retain initialized comment/prior-line tails: C's item
                    // scanner advances past an inserted NUL before checking
                    // the next item. Only the initialized prefix is admitted.
                    letter_group(&mut input[begin + 2..initialized], env, state, host)?;
                    continue;
                }
                if bytes.starts_with(b".replace") {
                    mode = 2;
                    write(host, &[6, 20])?;
                    let position = host.position().ok_or(Error::Io)?;
                    let padding = (4 - (position & 3)) & 3;
                    write(host, &[0; 3][..padding as usize])?;
                }
                if bytes.starts_with(b".group") {
                    mode = 1;
                    state.group = group_name(&bytes[6..], env, state, host)?;
                    host.context(state.line, &state.group);
                    continue;
                }
                // Every dot directive closes the current batch and is skipped,
                // including unknown directives and .replace itself.
                continue;
            }
            let bytes = line.unwrap();
            match mode {
                1 => {
                    let rule_env = rule_compile::Environment {
                        group: string(&state.group),
                        letters: &state.letters,
                        spaces: env.spaces,
                        line: state.line,
                        debug: env.debug,
                        signed_char: env.signed_char,
                    };
                    let mut output = [0; rule_compile::OUTPUT_BYTES];
                    let length = rule_compile::compile(host, &rule_env, bytes, &mut output);
                    if length > 0 {
                        if self.order.len() < MAX_RULES {
                            self.record(&output[..length])?;
                        } else if !limit_reported {
                            host.report(16, MAX_RULES as i32, &state.group);
                            limit_reported = true;
                        }
                    }
                }
                2 => {
                    let mut cursor = 0;
                    while bytes
                        .get(cursor)
                        .is_some_and(|&b| env.spaces[b as usize] != 0)
                    {
                        cursor += 1;
                    }
                    if bytes.get(cursor).is_some_and(|&b| b > 32) {
                        for _ in 0..2 {
                            let start = cursor;
                            while bytes.get(cursor).is_some_and(|&b| b > 32) {
                                cursor += 1;
                            }
                            write(host, &bytes[start..cursor])?;
                            write(host, &[0])?;
                            while bytes
                                .get(cursor)
                                .is_some_and(|&b| env.spaces[b as usize] != 0)
                            {
                                cursor += 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        self.groups.sort_unstable_by(|a, b| {
            string(&b.name)
                .len()
                .cmp(&string(&a.name).len())
                .then_with(|| string(&a.name).cmp(string(&b.name)))
                .then_with(|| a.index.cmp(&b.index))
        });
        let mut previous: [u8; 13] = terminate(b"\n");
        for (index, group) in self.groups.iter().enumerate() {
            if string(&group.name) != string(&previous) {
                if index > 0 {
                    write(host, &[7])?;
                }
                write(host, &[6])?;
                if group.group3 != 0 {
                    state.groups3 += 1;
                    write(host, &[1, group.group3 as u8])?;
                    // Legacy deliberately leaves prev_rgroup_name unchanged here.
                } else {
                    write(host, string(&group.name))?;
                    previous = group.name;
                }
                write(host, &[0])?;
            }
            write(host, &self.packed[group.packed.clone()])?;
        }
        write(host, &[7, 0])?;
        state.groups = self.groups.len() as i32;
        let summary = terminate::<80>(
            format!(
                "\t{} rules, {} groups ({})\n\n",
                state.rules, state.groups, state.groups3
            )
            .as_bytes(),
        );
        host.report(18, 0, &summary);
        Ok(())
    }
}

fn group_name(
    bytes: &[u8],
    env: &Environment<'_>,
    state: &mut State,
    host: &mut impl Host,
) -> Result<[u8; 13], Error> {
    let start = bytes
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(bytes.len());
    let bytes = &bytes[start..];
    let len = bytes
        .iter()
        .position(|&b| b <= 32)
        .unwrap_or(bytes.len())
        .min(12);
    let mut group = terminate::<13>(&bytes[..len]);
    state.group3 = 0;
    if let Some(value) = hex_group(string(&group)) {
        group = [0; 13];
        if value > 0x100 {
            group[0] = (value >> 8) as u8;
            group[1] = value as u8;
        } else {
            group[0] = value as u8;
        }
    } else {
        if env.letter_offset > 0 {
            let code = crate::word_key::decode(&group, 0).0;
            let ix = i64::from(code) - i64::from(env.letter_offset);
            if (0..128).contains(&ix) {
                state.group3 = ix as i32 + 1;
            }
        }
        if state.group3 == 0 && string(&group).len() > 2 {
            if crate::word_key::decode(&group, 0).1 < 2 {
                host.report(15, 0, b"\0");
            }
            group[2] = 0;
        }
    }
    Ok(group)
}
fn hex_group(bytes: &[u8]) -> Option<u32> {
    if !bytes.starts_with(b"0x") {
        return None;
    }
    let mut position = 2;
    while bytes.get(position).is_some_and(|b| b.is_ascii_whitespace()) {
        position += 1;
    }
    let negative = bytes.get(position) == Some(&b'-');
    if negative || bytes.get(position) == Some(&b'+') {
        position += 1;
    }
    if bytes
        .get(position..position + 2)
        .is_some_and(|b| b == b"0x" || b == b"0X")
    {
        position += 2;
    }
    let mut value = 0u32;
    let mut digits = 0;
    while let Some(code) = bytes.get(position).and_then(|b| (*b as char).to_digit(16)) {
        value = value.wrapping_mul(16).wrapping_add(code);
        position += 1;
        digits += 1;
    }
    if digits == 0 {
        None
    } else {
        Some(if negative {
            value.wrapping_neg()
        } else {
            value
        })
    }
}
fn letter_group(
    bytes: &mut [u8],
    env: &Environment<'_>,
    state: &mut State,
    host: &mut impl Host,
) -> Result<(), Error> {
    if !bytes.first().is_some_and(u8::is_ascii_digit)
        || !bytes.get(1).is_some_and(u8::is_ascii_digit)
    {
        host.report(12, 0, b"\0");
        return Ok(());
    }
    let mut group = 0usize;
    let mut cursor = 0;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        group = group
            .checked_mul(10)
            .and_then(|g| g.checked_add((bytes[cursor] - b'0') as usize))
            .ok_or(Error::Resource)?;
        cursor += 1;
    }
    if group >= 95 {
        host.report(13, 0, b"\0");
        return Ok(());
    }
    while bytes
        .get(cursor)
        .is_some_and(|&b| env.spaces[b as usize] == 0)
    {
        cursor += 1;
    }
    if cursor == bytes.len() {
        return Err(Error::Resource);
    }
    write(host, &[6, 18, (group as u8) + b'A'])?;
    if state.letters[group] != 0 {
        host.report(14, group as i32, b"\0");
    }
    state.letters[group] = 1;
    let mut items = [(0usize, 0usize); 200];
    let mut count = 0;
    while count < 200 {
        while bytes
            .get(cursor)
            .is_some_and(|&b| env.spaces[b as usize] != 0)
        {
            cursor += 1;
        }
        if cursor == bytes.len() || bytes[cursor] == 0 {
            break;
        }
        let start = cursor;
        while bytes.get(cursor).is_some_and(|&b| b > 32) {
            bytes[cursor] = if bytes[cursor] == b'_' {
                b' '
            } else {
                bytes[cursor]
            };
            cursor += 1;
        }
        // A virtual NUL terminates a final item without reading the old
        // uninitialized bytes past fgets' terminator.
        let ended = cursor == bytes.len();
        if !ended {
            bytes[cursor] = 0;
        }
        cursor += 1;
        let length = cursor - start;
        let byte = length as u8;
        let length = if env.signed_char {
            i32::from(byte as i8)
        } else {
            i32::from(byte)
        };
        items[count] = (start, length.max(0) as usize);
        count += 1;
        if ended {
            break;
        }
    }
    // Same length-first order and stable source order as the legacy scan;
    // preserve plain-char truncation/omission of long items.
    for length in (2..=255).rev() {
        for &(start, size) in &items[..count] {
            if size == length {
                // A token ending at the initialized boundary receives a
                // virtual terminator; no legacy uninitialized tail is read.
                let available = bytes.len() - start;
                write(host, &bytes[start..start + length.min(available)])?;
                if length > available {
                    write(host, &[0])?;
                }
            }
        }
    }
    write(host, &[7])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        output: Vec<u8>,
        reports: Vec<i32>,
        line: i32,
        fail: bool,
    }
    impl rule_compile::Host for Effects {
        fn encode(&mut self, input: &[u8], out: &mut [u8; 80]) -> Option<i32> {
            out[0] = match input[0] {
                b'a' => 41,
                b'b' => 42,
                0 => 0,
                _ => 43,
            };
            out[1] = 0;
            Some(0)
        }
        fn digit(&mut self, code: u32) -> bool {
            (48..=57).contains(&code)
        }
        fn diagnostic(&mut self, kind: rule_compile::Diagnostic, _: i32, _: &[u8]) {
            self.reports.push(kind as i32);
        }
    }
    impl Host for Effects {
        fn context(&mut self, line: i32, _: &[u8; 13]) {
            self.line = line;
        }
        fn report(&mut self, kind: i32, _: i32, _: &[u8]) {
            self.reports.push(kind);
        }
        fn write(&mut self, bytes: &[u8]) -> bool {
            if self.fail {
                false
            } else {
                self.output.extend_from_slice(bytes);
                true
            }
        }
        fn position(&mut self) -> Option<u64> {
            Some(self.output.len() as u64)
        }
    }
    fn run(
        workspace: &mut Workspace,
        source: &[u8],
        host: &mut Effects,
        offset: i32,
        signed_char: bool,
        mode: TextMode,
    ) -> Result<State, Error> {
        let mut spaces = [0; 256];
        for (i, b) in spaces.iter_mut().enumerate().take(33).skip(1) {
            *b = u8::from(i != 0);
        }
        workspace.compile(
            source,
            &Environment {
                letters: &[0; 95],
                spaces: &spaces,
                debug: false,
                signed_char,
                letter_offset: offset,
                text_mode: mode,
            },
            host,
        )
    }
    #[test]
    fn group_packing_common_phonemes_and_prefix_removal() {
        let mut host = Effects::default();
        let mut workspace = Workspace::default();
        let state = run(
            &mut workspace,
            b".group a\na a\nab a\nac b\n",
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(
            host.output,
            [6, b'a', 0, 4, 3, 41, 0, b'b', 0, b'c', 3, 42, 0, 7, 0]
        );
        assert_eq!((state.rules, state.groups, state.groups3), (3, 1, 0));
    }
    #[test]
    fn indexed_duplicate_groups_preserve_repeated_legacy_headers() {
        let mut host = Effects::default();
        let mut workspace = Workspace::default();
        let state = run(
            &mut workspace,
            b".group a\na a\n.group a\na b\n",
            &mut host,
            97,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(
            host.output,
            [6, 1, 1, 0, 3, 41, 0, 7, 6, 1, 1, 0, 3, 42, 0, 7, 0]
        );
        assert_eq!(state.groups3, 2);
    }
    #[test]
    fn replacement_alignment_uses_existing_writer_position() {
        let mut host = Effects {
            output: vec![0x55],
            ..Effects::default()
        };
        let mut workspace = Workspace::default();
        run(
            &mut workspace,
            b".replace\na x\nb\n.group a\na a\n",
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(
            host.output,
            [
                0x55, 6, 20, 0, b'a', 0, b'x', 0, b'b', 0, 0, 0, 0, 0, 0, 7, 6, b'a', 0, 3, 41, 0,
                7, 0
            ]
        );
    }
    #[test]
    fn long_letter_items_follow_plain_char_length_conversion() {
        let mut source = b".L94 ".to_vec();
        source.extend_from_slice(&[b'a'; 128]);
        source.push(b'\n');
        for signed in [true, false] {
            let mut host = Effects::default();
            let mut workspace = Workspace::default();
            let state = run(
                &mut workspace,
                &source,
                &mut host,
                0,
                signed,
                TextMode::Binary,
            )
            .unwrap();
            assert_eq!(state.letters[94], 1);
            if signed {
                assert_eq!(host.output, [6, 18, 159, 7, 7, 0]);
            } else {
                assert_eq!(host.output.len(), 135);
                assert_eq!(&host.output[3..131], &[b'a'; 128]);
                assert_eq!(&host.output[131..], &[0, 7, 7, 0]);
            }
        }
    }
    #[test]
    fn initialized_comment_and_prior_line_tails_remain_observable() {
        let mut host = Effects::default();
        let mut workspace = Workspace::default();
        run(
            &mut workspace,
            b".L01 j// voiced\n",
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(host.output, b"\x06\x12Bvoiced\0j\0/\0\x07\x07\0");
        host = Effects::default();
        run(
            &mut workspace,
            b".L01 abc defghi xyz\n.L02 z",
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(
            host.output,
            b"\x06\x12Bdefghi\0abc\0xyz\0\x07\x06\x12Cdefghi\0xyz\0z\0c\0\x07\x07\0"
        );
    }
    #[test]
    fn windows_text_translation_stops_at_ctrl_z_and_final_items_are_bounded() {
        let mut host = Effects::default();
        let mut workspace = Workspace::default();
        let state = run(
            &mut workspace,
            b".group a\r\na a\r\n\x1a.group b\r\nb b\r\n",
            &mut host,
            0,
            true,
            TextMode::Windows,
        )
        .unwrap();
        assert_eq!(state.line, 3);
        assert_eq!(state.rules, 1);
        assert_eq!(host.output, [6, b'a', 0, 3, 41, 0, 7, 0]);
        host = Effects::default();
        run(
            &mut workspace,
            b".L01 abc",
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(host.output, [6, 18, 66, b'a', b'b', b'c', 0, 7, 7, 0]);
    }
    #[test]
    fn writer_failure_and_group_limit_reject_with_retained_effects() {
        let mut host = Effects {
            fail: true,
            ..Effects::default()
        };
        let mut workspace = Workspace::default();
        assert!(matches!(
            run(
                &mut workspace,
                b".replace\n",
                &mut host,
                0,
                true,
                TextMode::Binary
            ),
            Err(Error::Io)
        ));
        assert_eq!(host.reports, [19]);
        host = Effects::default();
        let source = b".group a\na a\n".repeat(121);
        assert!(matches!(
            run(
                &mut workspace,
                &source,
                &mut host,
                0,
                true,
                TextMode::Binary
            ),
            Err(Error::Resource)
        ));
        assert_eq!(host.reports.last(), Some(&17));
    }
    #[test]
    fn rule_limit_is_reported_once_and_workspace_storage_reused() {
        let mut source = b".group a\n".to_vec();
        source.extend_from_slice(&b"a a\n".repeat(3002));
        let mut host = Effects::default();
        let mut workspace = Workspace::default();
        let state = run(
            &mut workspace,
            &source,
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(state.rules, 3000);
        assert_eq!(host.reports.iter().filter(|&&k| k == 16).count(), 1);
        let addresses = (
            workspace.records.as_ptr(),
            workspace.order.as_ptr(),
            workspace.packed.as_ptr(),
            workspace.groups.as_ptr(),
        );
        host = Effects::default();
        run(
            &mut workspace,
            b".group a\na b\n",
            &mut host,
            0,
            true,
            TextMode::Binary,
        )
        .unwrap();
        assert_eq!(
            addresses,
            (
                workspace.records.as_ptr(),
                workspace.order.as_ptr(),
                workspace.packed.as_ptr(),
                workspace.groups.as_ptr()
            )
        );
    }
}
