//! Complete resident rule-translation orchestration over serialized primitives.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{common_text, language, letters, number_lookup::PHONEME_BYTES, utf8};
pub const WORD_BYTES: usize = 160;
pub const NO_TRACE: u32 = 0x1000_0000;
pub const NO_PREFIX: u32 = 0x2000_0000;
pub const UNPRON: u32 = 0x8000_0000;
pub const DONT_SWITCH: u32 = 0x1000;
pub const PREFIX: i32 = 0x400;
pub const END_UNPRON: i32 = 0x8000;
pub const NO_DELETE: isize = isize::MIN;
const SWITCH: u8 = 21;
const PAUSE: u8 = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Source,
    State,
    Phonemes,
    Capacity,
}
/// Fully owned match pronunciation; cursor/deletion are checked source offsets.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Match {
    pub points: i32,
    pub ending: i32,
    pub cursor: usize,
    pub delete: isize,
    pub phonemes: [u8; PHONEME_BYTES],
}
impl Default for Match {
    fn default() -> Self {
        Self {
            points: 0,
            ending: 0,
            cursor: 0,
            delete: NO_DELETE,
            phonemes: [0; PHONEME_BYTES],
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    Ready,
    Offset,
    Tone,
    SayAs,
    Diereses,
    AltAlphabet,
    AltLanguage,
    Trace,
    BracketAnnounced,
    BracketPause,
    PrePause,
    GroupCount,
    GroupStart,
    GroupName,
    Flags,
    SignedBytes,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Store {
    Vowels,
    Stressed,
    Repeat,
    PrePause,
    Flags,
}
/// No foreign storage loan survives a nested match/letter/symbol operation.
/// Output projections preserve shared C-output effects of nested translation.
/// Append is the native phoneme-word primitive over fresh output/table/counts.
pub trait Host {
    fn byte(&self, position: isize) -> Option<u8>;
    fn write(&mut self, position: isize, byte: u8) -> Result<(), Error>;
    fn value(&self, field: Field, index: u32) -> i32;
    fn store(&mut self, field: Store, value: i32);
    fn locale(&self, code: u32, digit: bool) -> bool;
    /// 0: relative alphabet group, 1: two-byte group, 2: single/default group.
    fn group(&self, kind: u32, index: u32) -> Result<Option<usize>, Error>;
    fn match_group(
        &mut self,
        group: Option<usize>,
        width: usize,
        flags: u32,
        dictionary: u32,
        matched: &mut Match,
    ) -> Result<(), Error>;
    fn symbol(&mut self, key: &[u8; 8], output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error>;
    fn letter(&mut self, code: u32, output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error>;
    /// 0: pronunciation, 1: ending. Absent ending is queried separately.
    fn publish(&mut self, kind: u32, output: &[u8; PHONEME_BYTES]) -> Result<(), Error>;
    fn has_ending(&self) -> bool;
    fn append(&mut self, addition: &[u8; PHONEME_BYTES]) -> Result<(), Error>;
    /// 0: unpronounceable header, 1: ordinary header, 2: newline.
    fn trace(&mut self, kind: u32, word: &[u8; 120]);
}
fn byte(host: &impl Host, position: usize) -> Result<u8, Error> {
    host.byte(isize::try_from(position).map_err(|_| Error::Source)?)
        .ok_or(Error::Source)
}
fn decode(host: &impl Host, mut position: usize) -> Result<utf8::Character, Error> {
    while byte(host, position)? & 0xc0 == 0x80 {
        position = position.checked_add(1).ok_or(Error::Source)?;
    }
    utf8::head(|offset| {
        position
            .checked_add(offset)
            .and_then(|p| byte(host, p).ok())
    })
    .map_err(|_| Error::Source)
}
fn length(phonemes: &[u8; PHONEME_BYTES]) -> Result<usize, Error> {
    phonemes.iter().position(|b| *b == 0).ok_or(Error::Phonemes)
}
fn run_match(
    host: &mut impl Host,
    group: Option<usize>,
    width: usize,
    flags: u32,
    dictionary: u32,
    matched: &mut Match,
) -> Result<(), Error> {
    let previous = matched.cursor;
    host.match_group(group, width, flags, dictionary, matched)?;
    if matched.cursor <= previous {
        return Err(Error::State);
    }
    byte(host, matched.cursor)?;
    length(&matched.phonemes)?;
    Ok(())
}
fn restore(host: &mut impl Host, original: &[u8; WORD_BYTES]) -> Result<(), Error> {
    let length = original.iter().position(|b| *b == 0).ok_or(Error::Source)?;
    for (index, value) in original[..length].iter().enumerate() {
        host.write(index as isize, *value)?;
    }
    Ok(())
}
fn switch(host: &mut impl Host, language: u32) -> Result<(), Error> {
    let mut out = [0; PHONEME_BYTES];
    out[0] = SWITCH;
    let (name, length) = crate::clause_input::language_word(language);
    out[1..1 + length].copy_from_slice(&name[..length]);
    host.publish(0, &out)
}
/// Match groups in legacy order, retry deaccented text, return endings and append
/// pronunciation. Ordinary/ending completion restores the original non-NUL
/// source prefix; early language/unpronounceable returns deliberately do not.
/// Errors retain executed primitive/source/output effects and never replay them.
pub fn translate(host: &mut impl Host, flags: u32) -> Result<i32, Error> {
    if host.value(Field::Ready, 0) == 0 {
        return Ok(0);
    }
    let dictionary = host.value(Field::Flags, 0) as u32;
    let mut original = [0; WORD_BYTES];
    for (index, slot) in original[..WORD_BYTES - 1].iter_mut().enumerate() {
        *slot = byte(host, index)?;
        if *slot == 0 {
            break;
        }
    }
    let traced = |host: &dyn Host| host.value(Field::Trace, 0) != 0 && flags & NO_TRACE == 0;
    if traced(host) {
        let mut word = [0; 120];
        for (index, slot) in word[..119].iter_mut().enumerate() {
            let ch = byte(host, index)?;
            if ch == 0 || ch == b' ' {
                break;
            }
            *slot = ch;
        }
        host.trace(u32::from(flags & UNPRON == 0), &word);
    }
    host.store(Store::Vowels, 0);
    host.store(Store::Stressed, 0);
    if host.has_ending() {
        host.publish(1, &[0; PHONEME_BYTES])?;
    }
    let mut cursor = 0usize;
    let mut alpha = 0u32;
    let mut digits = 0u32;
    let mut matched = Match::default();
    let mut double = Match::default();
    loop {
        let first = byte(host, cursor)?;
        if matches!(first, 0 | b' ') {
            break;
        }
        let character = decode(host, cursor)?;
        let wc = character.code;
        let width = character.width;
        if common_text::word_alpha(wc, |c| host.locale(c, false)) {
            alpha = alpha.checked_add(1).ok_or(Error::State)?;
        }
        let count = host.value(Field::GroupCount, u32::from(first));
        if common_text::digit(wc, |c| host.locale(c, true))
            && (host.value(Field::Tone, 0) == 0 || alpha == 0)
        {
            let mut key = [0; 8];
            key[0] = b'_';
            for (i, slot) in key[1..=width].iter_mut().enumerate() {
                *slot = byte(host, cursor + i)?;
            }
            let mut out = [0; PHONEME_BYTES];
            host.symbol(&key, &mut out)?;
            let end = length(&out)?;
            if end >= 40 {
                return Err(Error::Capacity);
            }
            digits += 1;
            if digits >= 2 {
                out[end] = PAUSE;
                out[end + 1] = 0;
                digits = 0;
            }
            host.append(&out)?;
            cursor = cursor.checked_add(width).ok_or(Error::Source)?;
            continue;
        }
        digits = 0;
        let relative = i64::from(wc) - i64::from(host.value(Field::Offset, 0));
        let mut found = false;
        matched.cursor = cursor;
        if (0..128).contains(&relative) {
            if let Some(group) = host.group(0, relative as u32)? {
                run_match(host, Some(group), width, flags, dictionary, &mut matched)?;
                cursor = matched.cursor;
                found = true;
            }
        }
        if !found && count > 0 {
            let pair = u32::from(first) + (u32::from(byte(host, cursor + 1)?) << 8);
            let start = host.value(Field::GroupStart, u32::from(first));
            let finish = start.checked_add(count).ok_or(Error::State)?;
            if start < 0 || finish > 120 {
                return Err(Error::State);
            }
            for index in start..finish {
                if host.value(Field::GroupName, index as u32) as u32 == pair {
                    found = true;
                    double.cursor = cursor;
                    let group = host.group(1, index as u32)?;
                    run_match(host, group, 2, flags, dictionary, &mut double)?;
                    if double.points > 0 {
                        double.points = double.points.checked_add(35).ok_or(Error::State)?;
                    }
                    matched.cursor = cursor;
                    let group = host.group(2, u32::from(first))?;
                    run_match(host, group, 1, flags, dictionary, &mut matched)?;
                    if double.points >= matched.points {
                        matched = double;
                    }
                    cursor = matched.cursor;
                }
            }
        }
        if !found {
            matched.cursor = cursor;
            if let Some(group) = host.group(2, u32::from(first))? {
                run_match(host, Some(group), 1, flags, dictionary, &mut matched)?;
            } else {
                let group = host.group(2, 0)?;
                run_match(host, group, 0, flags, dictionary, &mut matched)?;
                cursor = matched.cursor;
                if matched.points == 0 && host.value(Field::SayAs, 0) & 0x10 == 0 {
                    let previous = cursor.checked_sub(1).ok_or(Error::Source)?;
                    let fallback = decode(host, previous)?;
                    let letter = fallback.code;
                    let removed = fallback.width - 1;
                    if host.value(Field::Offset, 0) > 0
                        && letter <= 0x241
                        && host.locale(letter, false)
                    {
                        switch(host, 0x656e)?;
                        return Ok(0);
                    }
                    if letter == 0xe028 {
                        let pause = host.value(Field::BracketAnnounced, 0);
                        if host.value(Field::PrePause, 0) < pause {
                            host.store(Store::PrePause, pause);
                        }
                    }
                    if common_text::bracket(letter as i32) != 0 {
                        let pause = host.value(Field::BracketPause, 0);
                        if host.value(Field::PrePause, 0) < pause {
                            host.store(Store::PrePause, pause);
                        }
                    }
                    if let Some(replacement) = letters::remove_accent(letter) {
                        let before = isize::try_from(cursor)
                            .map_err(|_| Error::Source)?
                            .checked_sub(2)
                            .ok_or(Error::Source)?;
                        if host.byte(before).ok_or(Error::Source)? != b' '
                            || byte(host, cursor + removed)? != b' '
                        {
                            host.write(previous as isize, replacement)?;
                            let mut destination = cursor;
                            loop {
                                let value = byte(
                                    host,
                                    destination.checked_add(removed).ok_or(Error::Source)?,
                                )?;
                                if value == 0 {
                                    return Err(Error::Source);
                                }
                                host.write(destination as isize, value)?;
                                if value == b' ' {
                                    break;
                                }
                                destination += 1;
                            }
                            for i in 0..removed {
                                host.write((destination + i) as isize, b' ')?;
                            }
                            if host.value(Field::Diereses, 0) != 0
                                && matches!(letter, 0xe4 | 0xeb | 0xef | 0xf6 | 0xfc | 0xff)
                            {
                                cursor = previous;
                                continue;
                            }
                            host.publish(0, &[0; PHONEME_BYTES])?;
                            cursor = 0;
                            host.store(Store::Vowels, 0);
                            host.store(Store::Stressed, 0);
                            continue;
                        }
                    }
                    if let Some(alphabet) = language::alphabet_from_char(letter as i32) {
                        if alphabet.offset != host.value(Field::Offset, 0) {
                            if host.value(Field::AltAlphabet, 0) == alphabet.offset {
                                let target = host.value(Field::AltLanguage, 0) as u32;
                                switch(host, target)?;
                                return Ok(0);
                            }
                            if alphabet.flags & 4 != 0 {
                                switch(host, alphabet.language as u32)?;
                                return Ok(0);
                            }
                        }
                    }
                }
            }
            cursor = matched.cursor;
            if matched.points == 0 {
                if !(0x300..=0x36f).contains(&wc) {
                    if common_text::word_alpha(wc, |c| host.locale(c, false)) {
                        let following = byte(host, cursor + width - 1)?;
                        let following = if host.value(Field::SignedBytes, 0) != 0 {
                            i32::from(following as i8)
                        } else {
                            i32::from(following)
                        };
                        if alpha > 1 || following > i32::from(b' ') {
                            host.publish(0, &[0; PHONEME_BYTES])?;
                            host.store(Store::Flags, host.value(Field::Flags, 0) | 0x1000);
                            break;
                        }
                    } else {
                        let mut out = [0; PHONEME_BYTES];
                        host.letter(wc, &mut out)?;
                        let end = length(&out)?;
                        if end >= 160 {
                            return Err(Error::Capacity);
                        }
                        if out[0] != 0 {
                            matched.phonemes = out;
                            matched.points = 1;
                        }
                    }
                }
                cursor = cursor.checked_add(width - 1).ok_or(Error::Source)?;
            } else {
                host.store(Store::Repeat, 0);
            }
        }
        if matched.points > 0 {
            if flags & UNPRON != 0 {
                return Ok(matched.ending | 1);
            }
            if matched.phonemes[0] == SWITCH && flags & DONT_SWITCH == 0 {
                host.publish(0, &matched.phonemes)?;
                return Ok(0);
            }
            if traced(host) {
                host.trace(2, &[0; 120]);
            }
            matched.ending &= !END_UNPRON;
            if matched.ending != 0
                && host.has_ending()
                && !(matched.ending & PREFIX != 0 && flags & NO_PREFIX != 0)
            {
                if matched.ending & PREFIX != 0 && matched.ending & 0x7f == 0 {
                    matched.ending |= i32::try_from(cursor).map_err(|_| Error::State)?;
                }
                host.publish(1, &matched.phonemes)?;
                restore(host, &original)?;
                return Ok(matched.ending);
            }
            if matched.delete != NO_DELETE {
                host.write(matched.delete, b'E')?;
            }
            host.append(&matched.phonemes)?;
        }
    }
    restore(host, &original)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Fixture {
        source: Vec<u8>,
        original: Vec<u8>,
        values: [i32; 16],
        counts: [i32; 3],
        output: [u8; PHONEME_BYTES],
        ending: [u8; PHONEME_BYTES],
        ending_enabled: bool,
        matches: VecDeque<Match>,
        calls: Vec<(Option<usize>, usize, u32, u32)>,
        groups: Vec<(u32, u32, usize)>,
        keys: Vec<[u8; 8]>,
        additions: Vec<Vec<u8>>,
        snapshots: Vec<Vec<u8>>,
        headers: Vec<u32>,
        shared: bool,
        mutate: bool,
    }
    fn pronunciation(bytes: &[u8]) -> [u8; PHONEME_BYTES] {
        let mut result = [0; PHONEME_BYTES];
        result[..bytes.len()].copy_from_slice(bytes);
        result
    }
    fn matched(cursor: usize, points: i32, ending: i32, bytes: &[u8]) -> Match {
        Match {
            cursor,
            points,
            ending,
            phonemes: pronunciation(bytes),
            ..Match::default()
        }
    }
    impl Fixture {
        fn new(source: &[u8]) -> Self {
            let mut text = vec![b' '];
            text.extend_from_slice(source);
            text.push(0);
            let mut values = [0; 16];
            values[Field::Ready as usize] = 1;
            Self {
                original: text.clone(),
                source: text,
                values,
                counts: [7, 8, 5],
                output: pronunciation(b"p\0"),
                ending: pronunciation(b"old\0"),
                ending_enabled: true,
                matches: VecDeque::new(),
                calls: vec![],
                groups: vec![],
                keys: vec![],
                additions: vec![],
                snapshots: vec![],
                headers: vec![],
                shared: false,
                mutate: false,
            }
        }
        fn result(&self) -> &[u8] {
            &self.output[..length(&self.output).unwrap()]
        }
    }
    impl Host for Fixture {
        fn byte(&self, position: isize) -> Option<u8> {
            usize::try_from(position + 1)
                .ok()
                .and_then(|index| self.source.get(index))
                .copied()
        }
        fn write(&mut self, position: isize, byte: u8) -> Result<(), Error> {
            let index = usize::try_from(position + 1).map_err(|_| Error::Source)?;
            *self.source.get_mut(index).ok_or(Error::Source)? = byte;
            Ok(())
        }
        fn value(&self, field: Field, _: u32) -> i32 {
            self.values[field as usize]
        }
        fn store(&mut self, field: Store, value: i32) {
            match field {
                Store::Vowels => self.counts[0] = value,
                Store::Stressed => self.counts[1] = value,
                Store::Repeat => self.counts[2] = value,
                Store::PrePause => self.values[Field::PrePause as usize] = value,
                Store::Flags => self.values[Field::Flags as usize] = value,
            }
        }
        fn locale(&self, code: u32, digit: bool) -> bool {
            if digit {
                (48..=57).contains(&code)
            } else {
                (65..=90).contains(&code) || (97..=122).contains(&code)
            }
        }
        fn group(&self, kind: u32, index: u32) -> Result<Option<usize>, Error> {
            Ok(self
                .groups
                .iter()
                .find(|(k, i, _)| *k == kind && *i == index)
                .map(|(_, _, id)| *id))
        }
        fn match_group(
            &mut self,
            group: Option<usize>,
            width: usize,
            flags: u32,
            dictionary: u32,
            out: &mut Match,
        ) -> Result<(), Error> {
            self.calls.push((group, width, flags, dictionary));
            *out = self
                .matches
                .pop_front()
                .unwrap_or_else(|| matched(out.cursor + width.max(1), 0, 0, b"\0"));
            if self.mutate {
                self.source[1] = b'Z';
            }
            Ok(())
        }
        fn symbol(&mut self, key: &[u8; 8], out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
            self.keys.push(*key);
            *out = pronunciation(b"d\0");
            if self.shared {
                self.output = pronunciation(b"shared\0");
            }
            Ok(())
        }
        fn letter(&mut self, _: u32, out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
            *out = pronunciation(b"l\0");
            Ok(())
        }
        fn publish(&mut self, kind: u32, out: &[u8; PHONEME_BYTES]) -> Result<(), Error> {
            let end = length(out)?;
            let target = if kind == 0 {
                &mut self.output
            } else {
                &mut self.ending
            };
            target[..=end].copy_from_slice(&out[..=end]);
            Ok(())
        }
        fn has_ending(&self) -> bool {
            self.ending_enabled
        }
        fn append(&mut self, out: &[u8; PHONEME_BYTES]) -> Result<(), Error> {
            let end = length(out)?;
            let size = length(&self.output)?;
            self.additions.push(out[..end].to_vec());
            self.snapshots.push(self.source.clone());
            if size + end < PHONEME_BYTES {
                self.output[size..=size + end].copy_from_slice(&out[..=end]);
                self.counts[0] += end as i32;
            }
            Ok(())
        }
        fn trace(&mut self, kind: u32, _: &[u8; 120]) {
            self.headers.push(kind);
        }
    }
    #[test]
    fn paired_digits_preserve_pause_order_and_nested_shared_output() {
        for shared in [false, true] {
            let mut host = Fixture::new(b"123 ");
            host.shared = shared;
            assert_eq!(translate(&mut host, 0), Ok(0));
            assert_eq!(host.keys.iter().map(|k| k[1]).collect::<Vec<_>>(), b"123");
            assert_eq!(
                host.additions,
                [b"d".to_vec(), b"d\x0b".to_vec(), b"d".to_vec()]
            );
            assert_eq!(
                host.result(),
                if shared {
                    b"sharedd".as_slice()
                } else {
                    b"pdd\x0bd".as_slice()
                }
            );
            assert_eq!(host.source, host.original);
        }
    }
    #[test]
    fn double_group_ties_win_after_ordered_single_probe() {
        let mut host = Fixture::new(b"ab ");
        host.values[Field::GroupCount as usize] = 1;
        host.values[Field::GroupName as usize] = i32::from(b'a') + (i32::from(b'b') << 8);
        host.groups = vec![(1, 0, 10), (2, u32::from(b'a'), 20)];
        host.matches = [matched(2, 1, 0, b"x\0"), matched(1, 36, 0, b"y\0")].into();
        assert_eq!(translate(&mut host, 0), Ok(0));
        assert_eq!(host.result(), b"px");
        assert_eq!(host.calls, [(Some(10), 2, 0, 0), (Some(20), 1, 0, 0)]);
        assert_eq!(host.counts[2], 5);
    }
    #[test]
    fn relative_alphabet_group_bypasses_pair_and_single_probes() {
        let mut host = Fixture::new("é ".as_bytes());
        host.values[Field::Offset as usize] = 0xc0;
        host.groups = vec![(0, 0xe9 - 0xc0, 3)];
        host.matches.push_back(matched(2, 1, 0, b"x\0"));
        assert_eq!(translate(&mut host, 0), Ok(0));
        assert_eq!(host.calls, [(Some(3), 2, 0, 0)]);
        assert_eq!(host.result(), b"px");
        assert_eq!(host.counts[2], 5);
    }
    #[test]
    fn prefix_length_and_ending_publication_restore_only_normal_completion() {
        let mut host = Fixture::new(b"abc ");
        host.mutate = true;
        host.groups = vec![(2, u32::from(b'a'), 1)];
        host.matches
            .push_back(matched(2, 1, PREFIX | END_UNPRON, b"end\0"));
        assert_eq!(translate(&mut host, 0), Ok(PREFIX | 2));
        assert_eq!(&host.ending[..4], b"end\0");
        assert_eq!(host.result(), b"p");
        assert_eq!(host.source, host.original);
        assert!(host.additions.is_empty());
    }
    #[test]
    fn ignored_prefix_deletes_before_append_then_restores_source() {
        let mut host = Fixture::new(b"abc ");
        host.groups = vec![(2, u32::from(b'a'), 1)];
        let mut rule = matched(3, 1, PREFIX | END_UNPRON, b"x\0");
        rule.delete = 1;
        host.matches.push_back(rule);
        assert_eq!(translate(&mut host, NO_PREFIX), Ok(0));
        assert_eq!(host.result(), b"px");
        assert_eq!(host.snapshots[0], b" aEc \0");
        assert_eq!(host.source, host.original);
        assert_eq!(host.ending[0], 0);
        assert_eq!(host.counts[2], 0);
    }
    #[test]
    fn unpronounceable_early_return_retains_source_effect_and_ending_bits() {
        let mut host = Fixture::new(b"abc ");
        host.mutate = true;
        host.values[Field::Trace as usize] = 1;
        host.groups = vec![(2, u32::from(b'a'), 1)];
        host.matches
            .push_back(matched(2, 1, END_UNPRON | PREFIX, b"x\0"));
        assert_eq!(translate(&mut host, UNPRON), Ok(END_UNPRON | PREFIX | 1));
        assert_eq!(host.source[1], b'Z');
        assert!(host.additions.is_empty());
        assert_eq!(host.headers, [0]);
    }
    #[test]
    fn alphabet_switch_uses_native_mnemonic_packing_and_preserves_early_effects() {
        let mut host = Fixture::new("α ".as_bytes());
        host.values[Field::AltAlphabet as usize] =
            language::alphabet_from_char(0x3b1).unwrap().offset;
        host.values[Field::AltLanguage as usize] = 0x610062;
        host.groups = vec![(2, 0, 1)];
        host.matches.push_back(matched(1, 0, 0, b"\0"));
        assert_eq!(translate(&mut host, 0), Ok(0));
        assert_eq!(host.result(), b"\x15ab");
        assert!(host.additions.is_empty());
        let mut latin = Fixture::new(b"abc ");
        latin.values[Field::Offset as usize] = 0x400;
        latin.mutate = true;
        latin.matches.push_back(matched(1, 0, 0, b"\0"));
        assert_eq!(translate(&mut latin, 0), Ok(0));
        assert_eq!(latin.result(), b"\x15en");
        assert_eq!(latin.source[1], b'Z');
    }
    #[test]
    fn nonprogress_and_unterminated_match_reject_without_replay() {
        for malformed in [false, true] {
            let mut host = Fixture::new(b"abc ");
            host.groups = vec![(2, u32::from(b'a'), 1)];
            let mut rule = matched(if malformed { 1 } else { 0 }, 1, 0, b"x\0");
            if malformed {
                rule.phonemes.fill(0x82);
            }
            host.matches.push_back(rule);
            assert_eq!(
                translate(&mut host, 0),
                Err(if malformed {
                    Error::Phonemes
                } else {
                    Error::State
                })
            );
            assert_eq!(host.calls.len(), 1);
            assert_eq!(host.result(), b"p");
            assert!(host.additions.is_empty());
        }
    }
    #[test]
    fn accent_retry_restores_original_and_missing_space_rejects_boundedly() {
        let mut host = Fixture::new("éx ".as_bytes());
        host.groups = vec![(2, 0, 1)];
        assert_eq!(translate(&mut host, 0), Ok(0));
        assert_eq!(host.source, host.original);
        assert_eq!(host.values[Field::Flags as usize] & 0x1000, 0x1000);
        assert_eq!(host.result(), b"");
        let mut malformed = Fixture::new("éx".as_bytes());
        malformed.groups = vec![(2, 0, 1)];
        assert_eq!(translate(&mut malformed, 0), Err(Error::Source));
        assert_eq!(malformed.calls.len(), 1);
        assert_eq!(malformed.result(), b"p");
        assert_eq!(malformed.source[1], b'e');
    }
    #[test]
    fn announced_bracket_pause_is_fresh_and_letter_fallback_keeps_repeat_state() {
        let mut host = Fixture::new("\u{e028} ".as_bytes());
        host.groups = vec![(2, 0, 1)];
        host.values[Field::BracketAnnounced as usize] = 29;
        host.values[Field::BracketPause as usize] = 17;
        assert_eq!(translate(&mut host, 0), Ok(0));
        assert_eq!(host.values[Field::PrePause as usize], 29);
        assert_eq!(host.result(), b"pl");
        assert_eq!(host.counts[2], 5);
    }
    #[test]
    fn diaeresis_retry_keeps_prefix_and_resumes_at_replacement() {
        let mut host = Fixture::new("aüx ".as_bytes());
        host.values[Field::Diereses as usize] = 1;
        host.groups = vec![(2, u32::from(b'a'), 1), (2, 0, 0), (2, u32::from(b'u'), 2)];
        host.matches = [
            matched(1, 1, 0, b"x\0"),
            matched(2, 0, 0, b"\0"),
            matched(3, 1, 0, b"y\0"),
        ]
        .into();
        assert_eq!(translate(&mut host, 0), Ok(0));
        assert_eq!(host.result(), b"pxy");
        assert_eq!(host.source, host.original);
        assert_eq!(host.counts[0], 2);
        assert_eq!(
            host.calls
                .iter()
                .map(|(g, w, _, _)| (*g, *w))
                .collect::<Vec<_>>(),
            [(Some(1), 1), (Some(0), 0), (Some(2), 1)]
        );
    }
}
