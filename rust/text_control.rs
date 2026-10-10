//! Embedded commands, combination, phoneme-language switching and text reset.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{translate_word::Error, utf8};
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    EmbeddedIndex,
    EmbeddedRead,
    SayAs,
    SayAs2,
    Digits,
    WordEmphasis,
    Emphasis,
    Pause,
    CombineLimit,
    Remaining,
    DictionarySkip,
    CurrentFlags,
    Sentences,
    Words,
    EndCharacter,
    SkipSentences,
    SkipWords,
    SkipCharacters,
    Skipping,
    NewSentence,
    EmbeddedFlag,
    SourceLength,
}
pub trait Host {
    fn value(&self, field: Field) -> i32;
    fn store(&mut self, field: Field, value: i32) -> Result<(), Error>;
    /// Domains: clause 0, word 1, pronunciation 2, syllable source 3, marker 4.
    /// Checked numeric offsets; no foreign loan is held across callbacks.
    fn byte(&self, kind: u32, index: usize) -> Option<u8>;
    fn write(&mut self, kind: u32, index: usize, value: u8) -> Result<(), Error>;
    fn command(&self, index: i32) -> Result<u32, Error>;
    fn put_command(&mut self, index: i32, value: u32) -> Result<(), Error>;
    fn default(&self, command: usize) -> Result<i32, Error>;
    fn alpha(&self, code: u32) -> bool;
    fn lower(&self, byte: u8) -> u8;
    fn row(&self, index: usize) -> Result<u32, Error>;
    fn put_row(&mut self, index: usize, flags: u32) -> Result<(), Error>;
    /// The original bounded word owner remains live; nested rule contexts are
    /// scoped and restored. Fresh source/row/pronunciation effects remain live.
    fn translate(
        &mut self,
        main: bool,
        offset: usize,
        row: usize,
        remaining: usize,
    ) -> Result<u32, Error>;
    fn vowel(&self, code: u8) -> Result<bool, Error>;
    fn lookup(&mut self, name: &[u8; 12]) -> Result<i32, Error>;
    fn select(&mut self, table: i32) -> Result<(), Error>;
    fn init(&mut self, which: u32) -> Result<(), Error>;
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultValue {
    pub value: u32,
    pub index: usize,
}
fn byte(h: &impl Host, kind: u32, index: usize, limit: usize) -> Result<u8, Error> {
    if index >= limit {
        return Err(Error::Source);
    }
    h.byte(kind, index).ok_or(Error::Source)
}
fn count(h: &impl Host, f: Field, max: i32) -> Result<i32, Error> {
    let n = h.value(f);
    if !(0..=max).contains(&n) {
        Err(Error::State)
    } else {
        Ok(n)
    }
}
pub fn embedded(h: &mut impl Host, input: usize) -> Result<ResultValue, Error> {
    let mut index = input;
    let mut sign = 0u32;
    match byte(h, 0, index, 840)? {
        b'+' => {
            sign = 0x40;
            index += 1;
        }
        b'-' => {
            sign = 0x60;
            index += 1;
        }
        _ => {}
    }
    let mut value = None;
    loop {
        let c = byte(h, 0, index, 840)?;
        if !c.is_ascii_digit() {
            break;
        }
        value = Some(
            value
                .unwrap_or(0i32)
                .checked_mul(10)
                .and_then(|v| v.checked_add(i32::from(c - b'0')))
                .ok_or(Error::Capacity)?,
        );
        index += 1;
    }
    let code = byte(h, 0, index, 840)?;
    index += 1;
    let ix = count(h, Field::EmbeddedIndex, 250)?;
    let rejected = ResultValue {
        value: 0,
        index: input,
    };
    if ix >= 248 {
        return Ok(rejected);
    }
    // strchr accepts NUL at the command string terminator as command 14.
    let Some(command) = b"PSARHTIVYMUBF\0"
        .iter()
        .position(|c| *c == code)
        .map(|n| n + 1)
    else {
        return Ok(rejected);
    };
    let value = if let Some(value) = value {
        value
    } else {
        sign = 0;
        h.default(command)?
    };
    if command == 9 {
        h.store(Field::SayAs2, value)?;
        h.store(Field::Digits, 0)?;
    }
    if command == 13 {
        h.store(Field::WordEmphasis, if value >= 3 { 0x800 } else { 0 })?;
    }
    h.put_command(ix, command as u32 + sign + ((value as u32) << 8))?;
    h.store(Field::EmbeddedIndex, ix + 1)?;
    Ok(ResultValue { value: 1, index })
}
pub fn word_embedded(h: &mut impl Host) -> Result<(), Error> {
    for _ in 0..250 {
        let read = count(h, Field::EmbeddedRead, 249)?;
        let cmd = h.command(read)?;
        h.store(Field::EmbeddedRead, read + 1)?;
        let value = (cmd as i32) >> 8;
        match cmd & 0x1f {
            9 => h.store(Field::SayAs, value)?,
            13 => h.store(Field::Emphasis, value)?,
            12 => {
                let pause = if value == 0 {
                    0
                } else {
                    h.value(Field::Pause)
                        .checked_add(value)
                        .ok_or(Error::Capacity)?
                };
                h.store(Field::Pause, pause)?;
            }
            _ => {}
        }
        if cmd & 0x80 != 0 || h.value(Field::EmbeddedRead) >= count(h, Field::EmbeddedIndex, 250)? {
            return Ok(());
        }
    }
    Err(Error::State)
}
fn word_length(h: &impl Host) -> Result<usize, Error> {
    let n = usize::try_from(h.value(Field::SourceLength)).map_err(|_| Error::Source)?;
    if n == 0 || n > 840 {
        Err(Error::Source)
    } else {
        Ok(n)
    }
}
fn decode(h: &impl Host, mut index: usize, limit: usize) -> Result<utf8::Character, Error> {
    while byte(h, 1, index, limit)? & 0xc0 == 0x80 {
        index = index.checked_add(1).ok_or(Error::Source)?;
    }
    utf8::head(|n| index.checked_add(n).and_then(|i| byte(h, 1, i, limit).ok()))
        .map_err(|_| Error::Source)
}
pub fn syllables(h: &impl Host) -> Result<u32, Error> {
    let mut n = 0;
    for i in 0..200 {
        let code = byte(h, 3, i, 200)?;
        if code == 0 {
            return Ok(n);
        }
        if h.vowel(code)? {
            n += 1;
        }
    }
    Err(Error::Phonemes)
}
pub fn combine(h: &mut impl Host) -> Result<(), Error> {
    let limit = h.value(Field::CombineLimit);
    let length = word_length(h)?;
    let remaining = usize::try_from(count(h, Field::Remaining, 300)?).map_err(|_| Error::State)?;
    let mut separator = 0;
    while byte(h, 1, separator, length)? != 32 {
        if byte(h, 1, separator, length)? == 0 {
            return Err(Error::Source);
        }
        separator += 1;
    }
    let mut ok = remaining > 1;
    if !h.alpha(decode(h, separator + 1, length)?.code) {
        ok = false;
    }
    let mut second_flags = 0;
    if ok {
        let mut saved = [0u8; 200];
        let mut used = 0;
        loop {
            saved[used] = byte(h, 2, used, 200)?;
            if saved[used] == 0 {
                break;
            }
            used += 1;
            if used >= 200 {
                return Err(Error::Phonemes);
            }
        }
        second_flags = h.translate(false, separator + 1, 1, remaining - 1)?;
        if second_flags & 0x04000000 != 0 || byte(h, 2, 0, 200)? == 21 {
            ok = false;
        }
        if limit & 0x100 != 0 && second_flags & 0x8000 == 0 {
            ok = false;
        }
        if limit & 0x200 != 0 && h.row(1)? & 0x10 != 0 {
            ok = false;
        }
        if !ok {
            for (i, v) in saved.iter().take(used + 1).enumerate() {
                h.write(2, i, *v)?;
            }
        }
    }
    if ok {
        h.write(1, separator, b'-')?;
        h.put_row(0, h.row(0)? & !1)?;
        let flags = h.translate(true, 0, 0, remaining)?;
        h.store(Field::CurrentFlags, flags as i32)?;
        if limit > 0 && syllables(h)? > (limit & 0x1f) as u32 {
            h.write(1, separator, 32)?;
            let flags = h.translate(true, 0, 0, remaining)?;
            h.store(Field::CurrentFlags, flags as i32)?;
        } else {
            let mut flags = h.value(Field::CurrentFlags) as u32;
            if flags == 0 {
                flags = second_flags;
            }
            h.store(Field::CurrentFlags, (flags | 0x80) as i32)?;
            h.store(Field::DictionarySkip, 1)?;
        }
    }
    Ok(())
}
pub fn switch_language(h: &mut impl Host) -> Result<(), Error> {
    let limit = word_length(h)?;
    let mut name = [0u8; 12];
    let mut used = 0;
    loop {
        let c = byte(h, 1, 3 + used, limit)?;
        if matches!(c, 0 | 32) {
            break;
        }
        if used >= 11 {
            return Err(Error::Capacity);
        }
        name[used] = h.lower(c);
        used += 1;
    }
    let table = h.lookup(&name)?;
    if table > 0 {
        h.select(table)?;
        h.write(2, 0, 21)?;
        h.write(2, 1, table as u8)?;
        h.write(2, 2, 0)?;
    }
    Ok(())
}
pub fn init_text(h: &mut impl Host, control: u32) -> Result<(), Error> {
    for f in [
        Field::Sentences,
        Field::Words,
        Field::EndCharacter,
        Field::SkipSentences,
    ] {
        h.store(f, 0)?;
    }
    h.write(4, 0, 0)?;
    for f in [Field::SkipWords, Field::SkipCharacters, Field::Skipping] {
        h.store(f, 0)?;
    }
    h.store(Field::NewSentence, 1)?;
    for f in [
        Field::SayAs,
        Field::SayAs2,
        Field::Emphasis,
        Field::WordEmphasis,
        Field::EmbeddedFlag,
    ] {
        h.store(f, 0)?;
    }
    h.init(0)?;
    if control & 0x2000 == 0 {
        h.init(1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        values: [i32; 22],
        source: [u8; 840],
        word: [u8; 840],
        ph: [u8; 200],
        p: [u8; 200],
        marker: [u8; 16],
        commands: [u32; 250],
        rows: [u32; 3],
        flags: [u32; 3],
        calls: usize,
        events: Vec<u32>,
        switch: bool,
        fail: bool,
        stuck: bool,
        fresh_ix: bool,
        aliased: bool,
        table: i32,
    }
    impl Fixture {
        fn new() -> Self {
            let mut h = Self {
                values: [0; 22],
                source: [0; 840],
                word: [0; 840],
                ph: [0; 200],
                p: [0; 200],
                marker: [0x55; 16],
                commands: [0; 250],
                rows: [0; 3],
                flags: [0x8000, 0, 0x42],
                calls: 0,
                events: Vec::new(),
                switch: false,
                fail: false,
                stuck: false,
                fresh_ix: false,
                aliased: false,
                table: 91,
            };
            h.values[Field::SourceLength as usize] = 840;
            h.values[Field::Remaining as usize] = 3;
            h.word[..7].copy_from_slice(b"ab cd \0");
            h.ph[..3].copy_from_slice(&[1, 2, 0]);
            h
        }
    }
    impl Host for Fixture {
        fn value(&self, f: Field) -> i32 {
            self.values[f as usize]
        }
        fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
            if !(self.stuck && matches!(f, Field::EmbeddedRead)) {
                self.values[f as usize] = v;
            }
            if self.fresh_ix && matches!(f, Field::SayAs) {
                self.values[Field::EmbeddedIndex as usize] = 3;
            }
            Ok(())
        }
        fn byte(&self, k: u32, i: usize) -> Option<u8> {
            match k {
                0 => self.source.get(i),
                1 => self.word.get(i),
                2 => self.ph.get(i),
                3 => {
                    if self.aliased {
                        self.ph.get(i)
                    } else {
                        self.p.get(i)
                    }
                }
                4 => self.marker.get(i),
                _ => None,
            }
            .copied()
        }
        fn write(&mut self, k: u32, i: usize, v: u8) -> Result<(), Error> {
            let b = match k {
                0 => self.source.get_mut(i),
                1 => self.word.get_mut(i),
                2 => self.ph.get_mut(i),
                4 => self.marker.get_mut(i),
                _ => None,
            };
            *b.ok_or(Error::Source)? = v;
            Ok(())
        }
        fn command(&self, i: i32) -> Result<u32, Error> {
            self.commands.get(i as usize).copied().ok_or(Error::State)
        }
        fn put_command(&mut self, i: i32, v: u32) -> Result<(), Error> {
            *self.commands.get_mut(i as usize).ok_or(Error::State)? = v;
            Ok(())
        }
        fn default(&self, c: usize) -> Result<i32, Error> {
            Ok(c as i32 * 3)
        }
        fn alpha(&self, c: u32) -> bool {
            matches!(c,65..=90|97..=122)
        }
        fn lower(&self, c: u8) -> u8 {
            c.to_ascii_lowercase()
        }
        fn row(&self, i: usize) -> Result<u32, Error> {
            self.rows.get(i).copied().ok_or(Error::State)
        }
        fn put_row(&mut self, i: usize, v: u32) -> Result<(), Error> {
            *self.rows.get_mut(i).ok_or(Error::State)? = v;
            Ok(())
        }
        fn translate(
            &mut self,
            main: bool,
            offset: usize,
            row: usize,
            remaining: usize,
        ) -> Result<u32, Error> {
            self.events
                .extend([main as u32, offset as u32, row as u32, remaining as u32]);
            let value = self.flags[self.calls.min(2)];
            self.calls += 1;
            self.ph[..5].copy_from_slice(&[if self.switch { 21 } else { 7 }, 8, 9, 10, 0]);
            if self.fail {
                Err(Error::State)
            } else {
                Ok(value)
            }
        }
        fn vowel(&self, c: u8) -> Result<bool, Error> {
            Ok(c & 1 != 0)
        }
        fn lookup(&mut self, name: &[u8; 12]) -> Result<i32, Error> {
            assert_eq!(&name[..3], b"en\0");
            self.events.push(31);
            Ok(self.table)
        }
        fn select(&mut self, t: i32) -> Result<(), Error> {
            self.events.push(t as u32);
            self.ph[3] = 71;
            Ok(())
        }
        fn init(&mut self, k: u32) -> Result<(), Error> {
            self.events.push(40 + k);
            self.values[Field::Words as usize] = 99;
            Ok(())
        }
    }
    #[test]
    fn embedded_defaults_signs_nul_and_capacity_preserve_public_index() {
        let mut h = Fixture::new();
        h.source[..4].copy_from_slice(b"+3F\0");
        assert_eq!(embedded(&mut h, 0), Ok(ResultValue { value: 1, index: 3 }));
        assert_eq!(h.commands[0], 13 + 0x40 + (3 << 8));
        assert_eq!(h.value(Field::WordEmphasis), 0x800);
        h.source[..3].copy_from_slice(b"-Y\0");
        embedded(&mut h, 0).unwrap();
        assert_eq!(h.commands[1], 9 + (27 << 8));
        assert_eq!(h.value(Field::SayAs2), 27);
        h.source[0] = 0;
        assert_eq!(embedded(&mut h, 0).unwrap().index, 1);
        assert_eq!(h.commands[2], 14 + (42 << 8));
        h.values[0] = 248;
        assert_eq!(embedded(&mut h, 0), Ok(ResultValue { value: 0, index: 0 }));
        h.values[0] = 0;
        h.source[..12].copy_from_slice(b"2147483648Y\0");
        assert_eq!(embedded(&mut h, 0), Err(Error::Capacity));
        assert_eq!(h.value(Field::EmbeddedIndex), 0);
        h.source[..12].copy_from_slice(b"2147483647P\0");
        assert_eq!(embedded(&mut h, 0).unwrap().value, 1);
        assert_eq!(h.commands[0], 0xffffff01);
    }
    #[test]
    fn word_commands_use_signed_values_and_fresh_command_extent() {
        let mut h = Fixture::new();
        h.values[0] = 1;
        h.fresh_ix = true;
        h.commands[..3].copy_from_slice(&[9 + (7 << 8), 12 + (0xfffffe00), 13 + 0x80 + (3 << 8)]);
        h.values[Field::Pause as usize] = 9;
        word_embedded(&mut h).unwrap();
        assert_eq!(h.value(Field::SayAs), 7);
        assert_eq!(h.value(Field::Pause), 7);
        assert_eq!(h.value(Field::Emphasis), 3);
        assert_eq!(h.value(Field::EmbeddedRead), 3);
        h.values[1] = 0;
        h.commands[0] = 12 + 0x80;
        word_embedded(&mut h).unwrap();
        assert_eq!(h.value(Field::Pause), 0);
    }
    #[test]
    fn malformed_command_progress_and_pause_overflow_keep_executed_read_effect() {
        let mut h = Fixture::new();
        h.values[0] = 250;
        h.stuck = true;
        assert_eq!(word_embedded(&mut h), Err(Error::State));
        h.stuck = false;
        h.values[Field::Pause as usize] = i32::MAX;
        h.commands[0] = 12 + (1 << 8);
        assert_eq!(word_embedded(&mut h), Err(Error::Capacity));
        assert_eq!(h.value(Field::EmbeddedRead), 1);
    }
    #[test]
    fn combination_preserves_second_flags_and_live_row_source_effects() {
        let mut h = Fixture::new();
        h.rows[0] = 0x41;
        h.values[Field::CombineLimit as usize] = 0x100;
        combine(&mut h).unwrap();
        assert_eq!(h.calls, 2);
        assert_eq!(h.word[2], b'-');
        assert_eq!(h.rows[0], 0x40);
        assert_eq!(h.value(Field::CurrentFlags), 0x8080);
        assert_eq!(h.value(Field::DictionarySkip), 1);
        assert_eq!(h.events, [0, 3, 1, 2, 1, 0, 0, 3]);
    }
    #[test]
    fn rejected_combination_restores_only_original_initialized_phoneme_prefix() {
        let mut h = Fixture::new();
        h.switch = true;
        combine(&mut h).unwrap();
        assert_eq!(h.calls, 1);
        assert_eq!(&h.ph[..5], &[1, 2, 0, 10, 0]);
        assert_eq!(h.word[2], 32);
        assert_eq!(h.value(Field::DictionarySkip), 0);
    }
    #[test]
    fn excessive_live_syllables_revert_separator_and_publish_second_translation() {
        let mut h = Fixture::new();
        h.aliased = true;
        h.values[Field::CombineLimit as usize] = 1;
        combine(&mut h).unwrap();
        assert_eq!(h.calls, 3);
        assert_eq!(h.word[2], 32);
        assert_eq!(h.value(Field::CurrentFlags), 0x42);
        assert_eq!(h.value(Field::DictionarySkip), 0);
        h.word.fill(b'a');
        assert_eq!(combine(&mut h), Err(Error::Source));
    }
    #[test]
    fn language_switch_selects_before_prefix_writes_and_refuses_overlong_name() {
        let mut h = Fixture::new();
        h.word[..7].copy_from_slice(b"_^_EN \0");
        switch_language(&mut h).unwrap();
        assert_eq!(h.events, [31, 91]);
        assert_eq!(&h.ph[..4], &[21, 91, 0, 71]);
        h.word[3..16].copy_from_slice(b"abcdefghijkl\0");
        assert_eq!(switch_language(&mut h), Err(Error::Capacity));
        assert_eq!(h.events, [31, 91]);
    }
    #[test]
    fn text_reset_preserves_marker_tail_and_fresh_initializer_effects() {
        let mut h = Fixture::new();
        h.values.fill(13);
        init_text(&mut h, 0x2000).unwrap();
        assert_eq!(h.events, [40]);
        assert_eq!(h.value(Field::Words), 99);
        assert_eq!(h.value(Field::NewSentence), 1);
        assert_eq!(h.value(Field::WordEmphasis), 0);
        assert_eq!(h.marker[0], 0);
        assert!(h.marker[1..].iter().all(|c| *c == 0x55));
        init_text(&mut h, 0).unwrap();
        assert_eq!(h.events, [40, 40, 41]);
    }
}
