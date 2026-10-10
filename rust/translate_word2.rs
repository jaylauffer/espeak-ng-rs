//! Complete word-to-first-stage-phoneme policy over serialized live owners.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{phoneme_list::Source as Entry, translate_word::Error};
// Two predecessor bytes plus the primitive's complete 161-byte text result.
pub const REPLACEMENT: usize = 163;
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    Length,
    Source,
    RowFlags,
    PreviousFlags,
    NextFlags,
    Remaining,
    Count,
    Embedded,
    EmbeddedIndex,
    SayAs,
    Capitals,
    Emphasis,
    Timeout,
    EndVowel,
    PreviousDictionary0,
    PreviousDictionary1,
    AnyStressed,
    VoiceTable,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Phoneme {
    pub kind: i32,
    pub length: i32,
    pub program: u32,
}
pub trait Host {
    fn value(&self, field: Field) -> i32;
    fn store(&mut self, field: Field, value: i32) -> Result<(), Error>;
    fn byte(&self, offset: usize, pronunciation: bool) -> Option<u8>;
    fn write(&mut self, offset: usize, byte: u8, pronunciation: bool) -> Result<(), Error>;
    fn entry(&self, index: i32) -> Result<Entry, Error>;
    fn put(&mut self, index: i32, entry: Entry) -> Result<(), Error>;
    fn command(&self, index: i32) -> Result<u32, Error>;
    fn put_command(&mut self, index: i32, value: u32) -> Result<(), Error>;
    fn embedded(&mut self) -> Result<(), Error>;
    fn encode(&mut self, language: bool) -> Result<(), Error>;
    /// Main translator/original source, or alternate translator with original
    /// source (0) or owned replacement at byte 1 (1). Owned 163-byte scratch;
    /// mode 1 disables replacement output and scopes its rule source owner.
    fn translate(
        &mut self,
        alternate: bool,
        mode: bool,
        replacement: &mut [u8; REPLACEMENT],
    ) -> Result<u32, Error>;
    fn combine(&mut self, flags: u32) -> Result<u32, Error>;
    fn dictionary(&self, out: &mut [u8; 40]) -> Result<(), Error>;
    fn restore_dictionary(&mut self, value: &[u8; 40]) -> Result<(), Error>;
    fn alternate(&mut self, language: &[u8; 200]) -> Result<i32, Error>;
    fn select(&mut self, table: i32) -> Result<(), Error>;
    fn phoneme(&self, code: u8) -> Option<Phoneme>;
    fn alpha(&self, code: u8) -> bool;
    fn invalid(&mut self, code: u8);
}
const SWITCH: u8 = 21;
const SPELL: u32 = 0x1000;
const DOUBLING: u32 = 0x4000;
fn byte(h: &impl Host, offset: usize, pronunciation: bool) -> Result<u8, Error> {
    h.byte(offset, pronunciation).ok_or(Error::Source)
}
fn count(h: &impl Host) -> Result<i32, Error> {
    let n = h.value(Field::Count);
    if (0..1000).contains(&n) {
        Ok(n)
    } else {
        Err(Error::Capacity)
    }
}
fn set(h: &mut impl Host, index: i32, code: u8) -> Result<(), Error> {
    let mut e = h.entry(index)?;
    e.code = code;
    e.stress = 0;
    e.tone = 0;
    e.synthflags = h.value(Field::Embedded) as u16;
    e.source = 0;
    h.put(index, e)?;
    h.store(Field::Embedded, 0)
}
fn append(h: &mut impl Host, code: u8) -> Result<(), Error> {
    let n = count(h)?;
    set(h, n, code)?;
    h.store(Field::Count, n + 1)
}
fn edit(h: &mut impl Host, index: i32, f: impl FnOnce(&mut Entry)) -> Result<(), Error> {
    let mut e = h.entry(index)?;
    f(&mut e);
    h.put(index, e)
}
fn restore(h: &mut impl Host, saved: &[u8; REPLACEMENT], length: usize) -> Result<(), Error> {
    for (i, &b) in saved[..length].iter().enumerate() {
        h.write(i, b, false)?;
    }
    Ok(())
}
pub fn translate(h: &mut impl Host, mut pause: i32) -> Result<u32, Error> {
    let length = h.value(Field::Length).min(31);
    let source = (h.value(Field::Source) & 0x7ff) | (length << 11);
    let wf = h.value(Field::RowFlags) as u32;
    if wf & 0x40 != 0 {
        h.store(Field::RowFlags, (wf & !0x40) as i32)?;
        h.store(Field::Embedded, 2)?;
        h.embedded()?;
    }
    if count(h)? >= 998 {
        return Ok(0);
    }
    if byte(h, 0, false)? == 0 || wf & 0x100000 != 0 {
        if h.value(Field::Embedded) != 0 {
            let n = count(h)?;
            append(h, 15)?;
            edit(h, n, |e| e.word_stress = 0)?;
            h.store(Field::Embedded, 0)?;
        }
        h.write(0, 0, true)?;
        return Ok(0);
    }
    if count(h)? >= 991 {
        return Ok(0);
    }
    let timeout = h.value(Field::Timeout);
    if timeout > 0 {
        h.store(Field::Timeout, timeout - 1)?;
    }
    if h.value(Field::SayAs) & 0xf0 == 0x10 && wf & 0x200 == 0 {
        pause = pause.wrapping_add(4);
    }
    let mut pitch = 0i32;
    if wf & 2 != 0 && h.value(Field::Capitals) > 2 && h.value(Field::EmbeddedIndex) < 244 {
        if h.value(Field::Embedded) != 0 {
            let i = h.value(Field::EmbeddedIndex) - 1;
            h.put_command(i, h.command(i)? & !0x80)?;
        }
        pitch = h.value(Field::Capitals);
        if pitch == 3 {
            pitch = 20;
        }
        let i = h.value(Field::EmbeddedIndex);
        h.put_command(i, 0xc1u32.wrapping_add((pitch as u32) << 8))?;
        h.store(Field::EmbeddedIndex, i + 1)?;
        h.store(Field::Embedded, 2)?;
    }
    let mut flags;
    let mut switched = -1;
    let mut dictionary = [0; 40];
    if wf & 8 != 0 {
        let language =
            byte(h, 0, false)? == b'_' && byte(h, 1, false)? == b'^' && byte(h, 2, false)? == b'_';
        h.encode(language)?;
        flags = 0x80000000;
    } else {
        let mut saved = [0; REPLACEMENT];
        let mut length = 0;
        loop {
            let b = byte(h, length, false)?;
            saved[length] = b;
            if matches!(b, 0 | b' ') || length == 160 {
                break;
            }
            length += 1;
        }
        let mut replacement = [0; REPLACEMENT];
        flags = h.translate(false, false, &mut replacement)?;
        if flags & SPELL != 0 {
            restore(h, &saved, length)?;
            return Ok(flags);
        }
        if flags & 0x800000 != 0
            && h.value(Field::Remaining) > 1
            && h.value(Field::NextFlags) & 8 == 0
        {
            flags = h.combine(flags)?;
        }
        if byte(h, 0, true)? == SWITCH {
            h.dictionary(&mut dictionary)?;
            if !dictionary.contains(&0) {
                return Err(Error::Source);
            }
            for _ in 0..2 {
                restore(h, &saved, length)?;
                let mut language = [0; 200];
                let mut end = false;
                for (i, b) in language.iter_mut().enumerate().take(199) {
                    *b = byte(h, i + 1, true)?;
                    if *b == 0 {
                        end = true;
                        break;
                    }
                }
                if !end {
                    return Err(Error::Source);
                }
                // Empty language selects the configured default in the host.
                switched = h.alternate(&language)?;
                if switched >= 0 {
                    h.store(Field::RowFlags, h.value(Field::RowFlags) | 0x400000)?;
                    let replace = replacement[2] != 0;
                    if replace {
                        replacement[0] = 0;
                        replacement[1] = b' ';
                    }
                    flags = h.translate(true, replace, &mut replacement)?;
                }
                if byte(h, 0, true)? != SWITCH {
                    break;
                }
            }
            if byte(h, 0, true)? == SWITCH {
                return Ok(SPELL);
            }
            if switched < 0 {
                h.write(0, 13, true)?;
                h.write(1, 13, true)?;
                h.write(2, 0, true)?;
            }
            if switched == -1 {
                h.restore_dictionary(&dictionary)?;
                h.select(h.value(Field::VoiceTable))?;
                switched = h.value(Field::VoiceTable);
            }
        }
        if wf & 0x80 == 0 {
            if flags & 0x10000000 != 0 {
                pause = pause.max(1);
            }
            if flags & 0x100 != 0
                && wf & 0x210 == 0
                && h.value(Field::PreviousFlags) & 0x200 == 0
                && h.value(Field::Timeout) == 0
            {
                pause = pause.max(4);
                h.store(Field::Timeout, 3)?;
            }
        }
        if h.value(Field::Emphasis) >= 3 {
            pause = pause.max(1);
        }
    }
    let mut stress = 0;
    let mut next_stress = 1;
    let mut next_tone = 0;
    let mut src = 0;
    let mut max_stress = -1;
    let mut max_index = 0;
    let mut previous_vowel = -1;
    let mut first = true;
    let found = if flags & 0x80000000 != 0 && flags & 0x20000000 == 0 {
        16
    } else {
        0
    };
    while pause > 0 && count(h)? < 991 {
        if pause > 1 {
            append(h, 9)?;
            pause -= 2;
        } else {
            append(h, 11)?;
            pause -= 1;
        }
        h.store(Field::EndVowel, 0)?;
        h.store(Field::PreviousDictionary0, 0)?;
        h.store(Field::PreviousDictionary1, 0)?;
    }
    let first_entry = count(h)?;
    if h.value(Field::Capitals) == 1 && wf & 2 != 0 {
        append(h, 10)?;
        append(h, 18)?;
        if wf & 1 != 0 && h.alpha(byte(h, 1, false)?) {
            append(h, 10)?;
            append(h, 18)?;
        }
    }
    if switched >= 0 {
        let n = count(h)?;
        if byte(h, 0, true)? == 9 && byte(h, 1, true)? == SWITCH {
            if h.entry(n - 1)?.code == SWITCH {
                h.store(Field::Count, n - 1)?;
            }
        } else {
            let index = if h.entry(n - 1)?.code == SWITCH {
                n - 1
            } else {
                set(h, n, SWITCH)?;
                n
            };
            edit(h, index, |e| e.tone = switched as u8)?;
            h.store(Field::Count, index + 1)?;
        }
    }
    let mut cursor = 0;
    if wf & 0x80 != 0 && h.phoneme(byte(h, 0, true)?).ok_or(Error::Phonemes)?.kind == 0 {
        cursor = 1;
    }
    if byte(h, cursor, true)? == 0 && h.value(Field::Embedded) != 0 {
        h.write(cursor, 23, true)?;
        h.write(cursor + 1, 0, true)?;
    }
    loop {
        let code = byte(h, cursor, true)?;
        cursor += 1;
        if code == 0 || count(h)? >= 995 {
            break;
        }
        if code == 255 {
            continue;
        }
        let Some(ph) = h.phoneme(code) else {
            h.invalid(code);
            continue;
        };
        let n = count(h)?;
        if code == SWITCH {
            let table = byte(h, cursor, true)?;
            edit(h, n, |e| {
                e.code = code;
                e.stress = 0;
                e.source = 0;
                e.synthflags = 0;
                e.tone = table;
            })?;
            h.store(Field::Count, n + 1)?;
            h.select(table as i32)?;
            cursor += 1;
        } else if ph.kind == 1 {
            if ph.program == 0 {
                next_stress = ph.length;
            } else if previous_vowel >= 0 {
                edit(h, previous_vowel, |e| e.tone = code)?;
            } else {
                next_tone = code;
            }
        } else if code == 20 {
            previous_vowel = n - 1;
            edit(h, previous_vowel, |e| {
                e.synthflags |= 4;
                e.stress = next_stress as u8;
            })?;
        } else if code == 12 {
            edit(h, n - 1, |e| e.synthflags |= 8)?;
        } else if code == 15 {
            src = source.wrapping_add(1);
        } else if code == 22 {
            flags |= DOUBLING;
        } else {
            let mut e = h.entry(n)?;
            e.code = code;
            e.tone = 0;
            e.synthflags = h.value(Field::Embedded) as u16 | found;
            h.store(Field::Embedded, 0)?;
            e.source = src as u16;
            src = 0;
            // Publish the same fields before scalar/vowel effects below.
            h.put(n, e)?;
            if ph.kind == 2 {
                stress = next_stress;
                next_stress = 1;
                if stress >= 4 {
                    h.store(Field::AnyStressed, 1)?;
                }
                if previous_vowel >= 0 && n - 1 != previous_vowel {
                    edit(h, n - 1, |e| e.stress = stress as u8)?;
                }
                e = h.entry(n)?;
                e.synthflags |= 4;
                previous_vowel = n;
                if stress > max_stress {
                    max_stress = stress;
                    max_index = n;
                }
                if next_tone != 0 {
                    e.tone = next_tone;
                    next_tone = 0;
                }
            } else if first && h.value(Field::PreviousDictionary0) as u32 & DOUBLING != 0 {
                e.synthflags |= 8;
            }
            e.stress = stress as u8;
            h.put(n, e)?;
            h.store(Field::Count, n + 1)?;
            first = false;
        }
    }
    if wf & 0x20000 != 0 {
        append(h, 27)?;
    }
    if wf & 0x80 == 0 {
        edit(h, first_entry, |e| e.source = source as u16)?;
    }
    h.store(Field::EndVowel, 0)?;
    if stress >= 4
        && h.phoneme(h.entry(count(h)? - 1)?.code)
            .ok_or(Error::Phonemes)?
            .kind
            == 2
    {
        h.store(Field::EndVowel, 1)?;
    }
    if switched >= 0 {
        h.restore_dictionary(&dictionary)?;
        h.select(h.value(Field::VoiceTable))?;
        let n = count(h)?;
        set(h, n, SWITCH)?;
        let table = h.value(Field::VoiceTable) as u8;
        edit(h, n, |e| e.tone = table)?;
        h.store(Field::Count, n + 1)?;
    }
    if pitch > 0 {
        let i = h.value(Field::EmbeddedIndex);
        h.put_command(i, 0xe1u32.wrapping_add((pitch as u32) << 8))?;
        h.store(Field::EmbeddedIndex, i + 1)?;
        let n = count(h)?;
        set(h, n, 10)?;
        edit(h, n, |e| e.synthflags = 2)?;
        h.store(Field::Count, n + 1)?;
    }
    if flags & 0x400 != 0 {
        edit(h, max_index, |e| e.synthflags |= 0x40)?;
    }
    h.store(Field::PreviousDictionary0, flags as i32)?;
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        fields: [i32; 18],
        source: Vec<u8>,
        output: [u8; 200],
        entries: Vec<Entry>,
        commands: [u32; 250],
        calls: usize,
        switches: usize,
        mode: u8,
        fail: bool,
        result: u32,
        selected: Vec<i32>,
        dictionary: [u8; 40],
    }
    impl Fixture {
        fn new() -> Self {
            let mut f = Self {
                fields: [0; 18],
                source: b"word \0".to_vec(),
                output: [0; 200],
                entries: vec![
                    Entry {
                        word_stress: 0x53,
                        ..Entry::default()
                    };
                    1000
                ],
                commands: [0; 250],
                calls: 0,
                switches: 0,
                mode: 0,
                fail: false,
                result: 0x80000000,
                selected: vec![],
                dictionary: [0; 40],
            };
            f.fields[Field::Count as usize] = 1;
            f.fields[Field::Length as usize] = 4;
            f.fields[Field::Source as usize] = 7;
            f.fields[Field::VoiceTable as usize] = 1;
            f.output[..4].copy_from_slice(&[35, 40, 50, 0]);
            f.dictionary[..4].copy_from_slice(b"old\0");
            f
        }
        fn field(&mut self, field: Field, value: i32) {
            self.fields[field as usize] = value;
        }
    }
    impl Host for Fixture {
        fn value(&self, f: Field) -> i32 {
            self.fields[f as usize]
        }
        fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
            self.field(f, v);
            Ok(())
        }
        fn byte(&self, i: usize, p: bool) -> Option<u8> {
            if p {
                self.output.get(i)
            } else {
                self.source.get(i)
            }
            .copied()
        }
        fn write(&mut self, i: usize, b: u8, p: bool) -> Result<(), Error> {
            *if p {
                self.output.get_mut(i)
            } else {
                self.source.get_mut(i)
            }
            .ok_or(Error::Source)? = b;
            Ok(())
        }
        fn entry(&self, i: i32) -> Result<Entry, Error> {
            self.entries
                .get(usize::try_from(i).map_err(|_| Error::Capacity)?)
                .copied()
                .ok_or(Error::Capacity)
        }
        fn put(&mut self, i: i32, e: Entry) -> Result<(), Error> {
            *self
                .entries
                .get_mut(usize::try_from(i).map_err(|_| Error::Capacity)?)
                .ok_or(Error::Capacity)? = e;
            Ok(())
        }
        fn command(&self, i: i32) -> Result<u32, Error> {
            self.commands
                .get(usize::try_from(i).map_err(|_| Error::Capacity)?)
                .copied()
                .ok_or(Error::Capacity)
        }
        fn put_command(&mut self, i: i32, v: u32) -> Result<(), Error> {
            *self
                .commands
                .get_mut(usize::try_from(i).map_err(|_| Error::Capacity)?)
                .ok_or(Error::Capacity)? = v;
            Ok(())
        }
        fn embedded(&mut self) -> Result<(), Error> {
            self.field(Field::SayAs, 0x12);
            Ok(())
        }
        fn encode(&mut self, _: bool) -> Result<(), Error> {
            Ok(())
        }
        fn translate(
            &mut self,
            alternate: bool,
            mode: bool,
            r: &mut [u8; REPLACEMENT],
        ) -> Result<u32, Error> {
            self.calls += 1;
            self.source[0] = b'!';
            if alternate && mode {
                assert_eq!(&r[..3], b"\0 r");
            }
            if self.fail {
                return Err(Error::State);
            }
            if self.mode > 0 && self.calls <= self.mode as usize {
                self.output[..4].copy_from_slice(&[21, b'r', b'u', 0]);
                r[2..6].copy_from_slice(b"rep\0");
            } else {
                self.output[..4].copy_from_slice(&[35, 40, 50, 0]);
            }
            Ok(self.result)
        }
        fn combine(&mut self, f: u32) -> Result<u32, Error> {
            Ok(f)
        }
        fn dictionary(&self, o: &mut [u8; 40]) -> Result<(), Error> {
            *o = self.dictionary;
            Ok(())
        }
        fn restore_dictionary(&mut self, v: &[u8; 40]) -> Result<(), Error> {
            self.dictionary = *v;
            Ok(())
        }
        fn alternate(&mut self, name: &[u8; 200]) -> Result<i32, Error> {
            assert_eq!(&name[..3], b"ru\0");
            self.switches += 1;
            self.dictionary[..4].copy_from_slice(b"new\0");
            Ok(3)
        }
        fn select(&mut self, t: i32) -> Result<(), Error> {
            self.selected.push(t);
            Ok(())
        }
        fn phoneme(&self, c: u8) -> Option<Phoneme> {
            Some(Phoneme {
                kind: if c == 35 {
                    1
                } else if c == 40 {
                    2
                } else {
                    3
                },
                length: 4,
                program: 0,
            })
        }
        fn alpha(&self, c: u8) -> bool {
            c.is_ascii_alphabetic()
        }
        fn invalid(&mut self, _: u8) {}
    }
    #[test]
    fn nested_translation_updates_source_and_emits_stress_without_clobbering_entry_tail() {
        let mut f = Fixture::new();
        assert_eq!(translate(&mut f, 0), Ok(0x80000000));
        assert_eq!(f.source[0], b'!');
        assert_eq!(f.value(Field::Count), 3);
        assert_eq!(
            f.entries[1],
            Entry {
                code: 40,
                stress: 4,
                synthflags: 20,
                source: 7 | (4 << 11),
                word_stress: 0x53,
                ..Entry::default()
            }
        );
        assert_eq!(f.entries[2].stress, 4);
        assert_eq!(f.value(Field::AnyStressed), 1);
    }
    #[test]
    fn spelling_restores_only_saved_word_bytes_and_skips_assembly() {
        let mut f = Fixture::new();
        f.result = SPELL | 0x80000000;
        assert_eq!(translate(&mut f, 0), Ok(f.result));
        assert_eq!(&f.source, b"word \0");
        assert_eq!(f.value(Field::Count), 1);
    }
    #[test]
    fn two_failed_language_retries_preserve_effects_and_return_spelling() {
        let mut f = Fixture::new();
        f.mode = 3;
        assert_eq!(translate(&mut f, 0), Ok(SPELL));
        assert_eq!(f.calls, 3);
        assert_eq!(f.switches, 2);
        assert_eq!(f.value(Field::Count), 1);
        assert_eq!(&f.dictionary[..4], b"new\0");
    }
    #[test]
    fn successful_language_retry_uses_owned_replacement_and_restores_table_dictionary() {
        let mut f = Fixture::new();
        f.mode = 1;
        assert_eq!(translate(&mut f, 0), Ok(0x80000000));
        assert_eq!(f.switches, 1);
        assert_eq!(f.selected, vec![1]);
        assert_eq!(&f.dictionary[..4], b"old\0");
        assert_eq!(f.entries[1].code, 21);
        assert_eq!(f.entries[1].tone, 3);
        assert_eq!(f.entries[4].code, 21);
        assert_eq!(f.entries[4].tone, 1);
    }
    #[test]
    fn failed_primitive_retains_source_effect_without_replay_or_list_writes() {
        let mut f = Fixture::new();
        f.fail = true;
        assert_eq!(translate(&mut f, 0), Err(Error::State));
        assert_eq!(f.calls, 1);
        assert_eq!(f.source[0], b'!');
        assert_eq!(f.value(Field::Count), 1);
    }
    #[test]
    fn deleted_word_carries_embedded_command_before_capacity_refusal() {
        let mut f = Fixture::new();
        f.field(Field::RowFlags, 0x100040);
        f.field(Field::Count, 997);
        assert_eq!(translate(&mut f, 0), Ok(0));
        assert_eq!(f.value(Field::Count), 998);
        assert_eq!(f.entries[997].code, 15);
        assert_eq!(f.entries[997].word_stress, 0);
        assert_eq!(f.entries[997].synthflags, 2);
        assert_eq!(f.calls, 0);
    }
    #[test]
    fn insufficient_list_capacity_refuses_after_embedded_effects() {
        let mut f = Fixture::new();
        f.field(Field::RowFlags, 0x40);
        f.field(Field::Count, 998);
        assert_eq!(translate(&mut f, 0), Ok(0));
        assert_eq!(f.value(Field::Embedded), 2);
        assert_eq!(f.value(Field::SayAs), 0x12);
        assert_eq!(f.calls, 0);
    }
    #[test]
    fn missing_source_and_invalid_predecessor_are_bounded() {
        let mut f = Fixture::new();
        f.source.clear();
        assert_eq!(translate(&mut f, 0), Err(Error::Source));
        let mut f = Fixture::new();
        f.field(Field::RowFlags, 8);
        f.field(Field::Count, 0);
        f.output[..2].copy_from_slice(&[12, 0]);
        assert_eq!(translate(&mut f, 0), Err(Error::Capacity));
        assert_eq!(f.value(Field::Count), 0);
    }
    #[test]
    fn pitch_commands_and_final_pause_consume_pending_embedded_flag() {
        let mut f = Fixture::new();
        f.field(Field::RowFlags, 2);
        f.field(Field::Capitals, 3);
        f.field(Field::EmbeddedIndex, 1);
        assert_eq!(translate(&mut f, 0), Ok(0x80000000));
        assert_eq!(f.commands[1], 0xc1 + (20 << 8));
        assert_eq!(f.commands[2], 0xe1 + (20 << 8));
        assert_eq!(f.entries[1].synthflags, 22);
        assert_eq!(f.entries[3].code, 10);
        assert_eq!(f.entries[3].synthflags, 2);
    }
    #[test]
    fn pause_clears_previous_doubling_and_reaches_reserved_capacity() {
        let mut f = Fixture::new();
        f.field(Field::Count, 989);
        f.field(Field::PreviousDictionary0, DOUBLING as i32);
        f.field(Field::PreviousDictionary1, 99);
        assert_eq!(translate(&mut f, 100), Ok(0x80000000));
        assert_eq!(f.value(Field::Count), 993);
        assert_eq!(f.entries[989].code, 9);
        assert_eq!(f.entries[990].code, 9);
        assert_eq!(f.value(Field::PreviousDictionary1), 0);
    }
}
