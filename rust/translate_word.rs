//! Complete word translation over bounded numeric source identities and effects.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{common_text, language, number_primitives, utf8};
pub const PHONEMES: usize = 200;
pub const WORD: usize = 160;
const SWITCH: u8 = 21;
const PREFIX: i32 = 0x400;
const BREAK: i32 = 0x20000;
const AFTER_STRESS: i32 = 0x10000;
const MORE: i32 = 0x80000;
const SKIP: u32 = 0x80;
const TEXT: u32 = 0x20000000;
const ABBREV: u32 = 0x2000;
const SPELL: u32 = 0x1000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Source,
    State,
    Phonemes,
    Capacity,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Source {
    pub slot: u32,
    pub offset: isize,
}
impl Source {
    pub fn advance(self, count: isize) -> Result<Self, Error> {
        Ok(Self {
            offset: self.offset.checked_add(count).ok_or(Error::Source)?,
            ..self
        })
    }
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    Ready,
    WordFlags,
    Remaining,
    SayAs,
    Skip,
    Numbers,
    Numbers2,
    Emphasize,
    Lower,
    Upper,
    Prefixes,
    Trace,
    StressFlags,
    AnyPresent,
    Language,
    Alt,
    Verb,
    VerbS,
    Noun,
    Past,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Store {
    Skip,
    Verb,
    VerbS,
    Noun,
    Past,
}
/// Serialized primitives retain live source/row/translator/output owners.
/// Controller loans are only owned pronunciation/flags/copy buffers through a
/// single callback. No foreign source, state or output is borrowed across calls.
pub trait Host {
    fn byte(&self, source: Source) -> Option<u8>;
    fn write(&mut self, source: Source, byte: u8) -> Result<(), Error>;
    fn value(&self, field: Field, index: u32) -> i32;
    fn store(&mut self, field: Store, value: i32);
    /// Raw CRT alpha/digit classification (0/1); common IsAlpha also uses alpha.
    fn locale(&self, code: u32, digit: bool) -> bool;
    fn list(
        &mut self,
        source: &mut Source,
        phonemes: &mut [u8; PHONEMES],
        flags: &mut [u32; 2],
        ending: i32,
    ) -> Result<bool, Error>;
    fn emoji(&mut self, source: &mut Source, flags: &mut [u32; 2]) -> Result<(), Error>;
    fn text(&mut self, source: Source) -> Result<(), Error>;
    fn dotted(&mut self, source: Source) -> Result<i32, Error>;
    /// The named number-language probe writes the actual shared word output.
    fn number_language(&mut self) -> Result<(), Error>;
    fn number(
        &mut self,
        roman: bool,
        source: Source,
        phonemes: &mut [u8; PHONEMES],
        flags: &mut [u32; 2],
    ) -> Result<bool, Error>;
    fn spell(
        &mut self,
        source: &mut Source,
        phonemes: &mut [u8; PHONEMES],
        mode: i32,
    ) -> Result<bool, Error>;
    fn letter(
        &mut self,
        source: Source,
        phonemes: &mut [u8; PHONEMES],
        non_initial: bool,
    ) -> Result<usize, Error>;
    fn unpronounceable(&mut self, source: Source, position: i32) -> Result<bool, Error>;
    fn spelling_stress(
        &mut self,
        phonemes: &mut [u8; PHONEMES],
        position: i32,
    ) -> Result<(), Error>;
    fn rules(
        &mut self,
        source: Source,
        phonemes: &mut [u8; PHONEMES],
        ending: Option<&mut [u8; PHONEMES]>,
        word_flags: u32,
        flags: &mut [u32; 2],
    ) -> Result<i32, Error>;
    fn remove(
        &mut self,
        source: Source,
        ending: i32,
        copy: Option<&mut [u8; WORD]>,
    ) -> Result<i32, Error>;
    fn prefix(&mut self, source: &[u8; 65]) -> Result<Source, Error>;
    fn trace_suffix(&mut self, phonemes: &[u8; PHONEMES]);
    fn append(
        &mut self,
        phonemes: &mut [u8; PHONEMES],
        ending: &[u8; PHONEMES],
    ) -> Result<(), Error>;
    fn plural(&mut self, word_flags: u32, last: u32) -> Result<(), Error>;
    fn stress(
        &mut self,
        phonemes: Option<&mut [u8; PHONEMES]>,
        flags: &mut [u32; 2],
        position: i32,
        control: i32,
    ) -> Result<(), Error>;
    fn snapshot(&self, phonemes: &mut [u8; PHONEMES]) -> Result<(), Error>;
    /// Joined publication also clears owner byte 199, like the legacy snprintf.
    fn publish(&mut self, phonemes: &[u8; PHONEMES], joined: bool) -> Result<(), Error>;
    fn change_stress(&mut self, level: i32) -> Result<(), Error>;
    fn special(&mut self, flags: u32) -> Result<(), Error>;
}
fn byte(host: &impl Host, source: Source) -> Result<u8, Error> {
    host.byte(source).ok_or(Error::Source)
}
fn code(host: &impl Host, mut source: Source) -> Result<utf8::Character, Error> {
    while byte(host, source)? & 0xc0 == 0x80 {
        source = source.advance(1)?;
    }
    utf8::head(|i| source.advance(i as isize).ok().and_then(|p| host.byte(p)))
        .map_err(|_| Error::Source)
}
fn alpha(host: &impl Host, code: u32) -> bool {
    common_text::word_alpha(code, |c| host.locale(c, false))
}
fn length(phonemes: &[u8; PHONEMES]) -> Result<usize, Error> {
    phonemes.iter().position(|b| *b == 0).ok_or(Error::Phonemes)
}
fn restore(host: &mut impl Host, source: Source, copy: &[u8; WORD]) -> Result<(), Error> {
    let end = copy.iter().position(|b| *b == 0).ok_or(Error::Source)?;
    for (i, b) in copy[..end].iter().enumerate() {
        host.write(source.advance(i as isize)?, *b)?;
    }
    Ok(())
}
fn append_truncated(out: &mut [u8; PHONEMES], addition: &[u8; PHONEMES]) -> Result<(), Error> {
    let used = length(out)?;
    let count = length(addition)?.min(PHONEMES - 1 - used);
    out[used..used + count].copy_from_slice(&addition[..count]);
    out[used + count] = 0;
    Ok(())
}
fn joined(parts: [&[u8; PHONEMES]; 3]) -> Result<[u8; PHONEMES], Error> {
    let mut out = [0; PHONEMES];
    for part in parts {
        append_truncated(&mut out, part)?;
    }
    Ok(out)
}
fn copy(out: &mut [u8; PHONEMES], input: &[u8; PHONEMES]) -> Result<(), Error> {
    let end = length(input)?;
    out[..=end].copy_from_slice(&input[..=end]);
    Ok(())
}
fn rules(
    host: &mut impl Host,
    source: Source,
    phonemes: &mut [u8; PHONEMES],
    ending: Option<&mut [u8; PHONEMES]>,
    word_flags: u32,
    flags: &mut [u32; 2],
) -> Result<i32, Error> {
    match ending {
        Some(ending) => {
            let result = host.rules(source, phonemes, Some(ending), word_flags, flags)?;
            length(phonemes)?;
            length(ending)?;
            Ok(result)
        }
        None => {
            let result = host.rules(source, phonemes, None, word_flags, flags)?;
            length(phonemes)?;
            Ok(result)
        }
    }
}
fn list(
    host: &mut impl Host,
    source: &mut Source,
    phonemes: &mut [u8; PHONEMES],
    flags: &mut [u32; 2],
    ending: i32,
) -> Result<bool, Error> {
    let found = host.list(source, phonemes, flags, ending)?;
    byte(host, *source)?;
    length(phonemes)?;
    Ok(found)
}
fn signature(host: &impl Host, source: Source) -> Result<(usize, u64), Error> {
    let mut cursor = source;
    let mut count = 0;
    let mut hash = 0xcbf29ce484222325u64;
    loop {
        let b = byte(host, cursor)?;
        if b == 0 || b == b' ' {
            return Ok((count, hash));
        }
        hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        count += 1;
        cursor = cursor.advance(1)?;
    }
}
/// Complete legacy word policy. Ordinary completion restores only the admitted
/// original prefix; early returns deliberately preserve source/output effects.
/// Malformed inputs retain executed effects and never replay the C controller.
pub fn translate(host: &mut impl Host) -> Result<u32, Error> {
    let mut word_flags = host.value(Field::WordFlags, 0) as u32;
    let remaining = host.value(Field::Remaining, 0);
    let mut flags = [0u32; 2];
    let mut flags2 = [0u32; 2];
    host.store(Store::Skip, 0);
    let mut phon = [0; PHONEMES];
    let mut unpron = [0; PHONEMES];
    let mut prefix = [0; PHONEMES];
    let mut ending = [0; PHONEMES];
    if host.value(Field::Ready, 0) == 0 {
        host.publish(&phon, false)?;
        return Ok(0);
    }
    let start = Source::default();
    let mut word1 = start;
    if byte(host, word1)? == b' ' {
        word1 = word1.advance(1)?;
    }
    let mut wordx = word1;
    let first = code(host, wordx)?.code;
    let mut last = 0;
    let mut count = 0usize;
    while !matches!(byte(host, wordx)?, 0 | b' ') {
        let ch = code(host, wordx)?;
        last = ch.code;
        wordx = wordx.advance(ch.width as isize)?;
        count = count.checked_add(1).ok_or(Error::State)?;
    }
    let original_length = usize::try_from(wordx.offset)
        .map_err(|_| Error::Source)?
        .min(WORD - 1);
    let mut original = [0; WORD];
    for (i, slot) in original[..original_length].iter_mut().enumerate() {
        *slot = byte(host, start.advance(i as isize)?)?;
    }
    let mut spell = 0;
    let mut found = false;
    let mut emphasized = 0u32;
    let mut was_unpron = 0;
    if count == 1 && word_flags & 0x400000 != 0 {
        let next = code(host, wordx.advance(1)?)?.code;
        if !alpha(host, next)
            || language::alphabet_from_char(last as i32).map(|a| a.first)
                != language::alphabet_from_char(next as i32).map(|a| a.first)
        {
            spell = 1;
        }
    }
    if host.value(Field::SayAs, 0) == 0x24 {
        if count == 1 {
            spell = 4;
        } else {
            word1 = word1.advance(-1)?;
            host.write(word1, b'_')?;
            found = list(host, &mut word1, &mut phon, &mut flags, 0)?;
        }
    }
    if host.value(Field::SayAs, 0) & 0x10 != 0 {
        spell = host.value(Field::SayAs, 0) & 0xf;
    } else {
        if !found {
            found = list(host, &mut word1, &mut phon, &mut flags, 2)?;
        }
        if !found && flags[0] & TEXT == 0 && common_text::emoji(first) {
            host.emoji(&mut word1, &mut flags)?;
            byte(host, word1)?;
        }
        if flags[0] & 0x03000000 != 0 && byte(host, wordx.advance(1)?)? == b'.' {
            host.write(wordx.advance(1)?, b' ')?;
        }
        if flags[0] & TEXT != 0 {
            host.text(word1)?;
            return Ok(flags[0]);
        } else if !found && flags[0] & SKIP != 0 && flags[0] & ABBREV == 0 {
            wordx = word1;
            let mut skipped = 0;
            while skipped < host.value(Field::Skip, 0) {
                let b = byte(host, wordx)?;
                if b == 0 {
                    return Err(Error::Source);
                }
                if b == b' ' {
                    host.write(wordx, b'-')?;
                    skipped += 1;
                }
                wordx = wordx.advance(1)?;
            }
        }
        if count == 1 && host.value(Field::Skip, 0) == 0 && host.dotted(word1)? != 0 {
            flags = [0; 2];
            spell = 1;
            if host.value(Field::Skip, 0) != 0 {
                flags[0] = SKIP;
            }
        }
        if phon[0] == SWITCH {
            host.publish(&phon, false)?;
            return Ok(0);
        }
        if !found && flags[0] & ABBREV != 0 {
            spell = 1;
        }
        if !found && host.locale(first, true) {
            host.number_language()?;
            let mut current = [0; PHONEMES];
            host.snapshot(&mut current)?;
            length(&current)?;
            if current[0] == SWITCH {
                return Ok(0);
            }
            if host.value(Field::Numbers2, 0) & 0x8000 != 0
                && host.value(Field::WordFlags, 0) as u32 & 0x200000 == 0
            {
                current = [0; PHONEMES];
                current[0] = SWITCH;
                host.publish(&current, false)?;
                return Ok(0);
            }
            found = host.number(false, word1, &mut phon, &mut flags)?;
            length(&phon)?;
        }
        if !found && word_flags & 3 != 2 {
            let numbers = host.value(Field::Numbers, 0) as u32;
            if (numbers & 0x01000000 != 0 || numbers & 0x02000000 != 0 && word_flags & 1 != 0)
                && (word_flags & 0x10 != 0
                    || remaining <= 1
                    || host.value(Field::WordFlags, 1) as u32 & 0x100 == 0)
            {
                found = host.number(true, word1, &mut phon, &mut flags)?;
                length(&phon)?;
                if found {
                    flags[0] |= ABBREV;
                }
            }
        }
        if word_flags & 1 != 0 && count > 1 && host.locale(first, false) {
            if host.value(Field::Emphasize, 0) & 0x100 != 0 && flags[0] & ABBREV == 0 {
                emphasized = 0x800;
            } else if !found
                && flags[0] & SKIP == 0
                && count < 4
                && host.value(Field::Lower, 0) > 3
                && host.value(Field::Upper, 0) <= host.value(Field::Lower, 0)
            {
                spell = 1;
            }
        }
    }
    if spell > 0 {
        phon[0] = 0;
        if host.spell(&mut word1, &mut phon, spell)? {
            return Ok(if count > 1 { SPELL } else { 0 });
        }
        length(&phon)?;
        host.publish(&phon, false)?;
        if word_flags & 0x400000 != 0 {
            return Ok(0);
        }
        host.plural(word_flags, last)?;
        return Ok(flags[0] & SKIP);
    }
    let mut end_type1 = 0;
    let mut prefix_type = 0;
    let mut prefix_flags = false;
    if !found {
        let mut position = 0i32;
        let mut length_remaining = 999usize;
        wordx = word1;
        while (0 < length_remaining && length_remaining < 3)
            || (count > 1 && host.unpronounceable(wordx, position)?)
        {
            was_unpron = 0x04000000;
            emphasized = 0;
            if byte(host, wordx)? == b'\'' {
                break;
            }
            let consumed = host.letter(wordx, &mut unpron, position > 0)?;
            length(&unpron)?;
            position = position.checked_add(1).ok_or(Error::State)?;
            if unpron[0] == SWITCH {
                host.publish(&unpron, false)?;
                return Ok(if &unpron[1..=3] == b"en\0" { SPELL } else { 0 });
            }
            if consumed == 0 || consumed > 4 {
                return Err(Error::State);
            }
            wordx = wordx.advance(consumed as isize)?;
            length_remaining = 0;
            loop {
                let b = byte(host, wordx.advance(length_remaining as isize)?)?;
                if b == b' ' {
                    break;
                }
                if b == 0 {
                    return Err(Error::Source);
                }
                length_remaining += 1;
            }
        }
        host.spelling_stress(&mut unpron, position)?;
        length(&unpron)?;
        if byte(host, wordx)? != b' ' {
            if unpron[0] != 0 && byte(host, wordx)? != b'\'' {
                host.write(wordx.advance(-1)?, b' ')?;
            }
            let mut end_type = rules(
                host,
                wordx,
                &mut phon,
                Some(&mut ending),
                word_flags,
                &mut flags,
            )?;
            if phon[0] == SWITCH {
                host.publish(&phon, false)?;
                return Ok(0);
            }
            if phon[0] == 0 && ending[0] == 0 {
                let c = code(host, wordx)?.code;
                if count == 1 && (alpha(host, c) || number_primitives::superscript(c as i32) != 0) {
                    if host.spell(&mut wordx, &mut phon, spell)? {
                        return Ok(0);
                    }
                    length(&phon)?;
                    host.publish(&phon, false)?;
                    return Ok(0);
                }
            }
            let mut preceding = byte(host, wordx.advance(-1)?)?;
            let mut confirm = true;
            let mut saved = [0; WORD];
            for _ in 0..50 {
                if end_type & PREFIX == 0 {
                    break;
                }
                if confirm && end_type & BREAK == 0 {
                    let mut probe = [0; PHONEMES];
                    let mut probe_ending = [0; PHONEMES];
                    let second = rules(
                        host,
                        wordx,
                        &mut probe,
                        Some(&mut probe_ending),
                        word_flags | 0x30000000,
                        &mut flags,
                    )?;
                    if second != 0 {
                        host.remove(wordx, second, Some(&mut saved))?;
                        end_type = rules(
                            host,
                            wordx,
                            &mut phon,
                            Some(&mut ending),
                            word_flags | 0x10000000,
                            &mut flags,
                        )?;
                        restore(host, wordx, &saved)?;
                        if end_type & PREFIX == 0 {
                            end_type = second;
                            copy(&mut phon, &probe)?;
                            copy(&mut ending, &probe_ending)?;
                            if host.value(Field::Trace, 0) != 0 {
                                host.trace_suffix(&ending);
                            }
                        }
                        confirm = false;
                        continue;
                    }
                }
                prefix_type = end_type;
                if prefix_type & 0x800 != 0 {
                    host.store(Store::Verb, 1);
                }
                host.write(wordx.advance(-1)?, preceding)?;
                let mut prefix_chars = [0; 65];
                if prefix_type & BREAK == 0 {
                    for _ in 0..prefix_type & 0xf {
                        wordx = wordx.advance(1)?;
                        while byte(host, wordx)? & 0xc0 == 0x80 {
                            wordx = wordx.advance(1)?;
                        }
                    }
                } else {
                    let size = (prefix_type & 0x3f) as usize;
                    for i in 0..size {
                        prefix_chars[i + 1] = byte(host, wordx)?;
                        wordx = wordx.advance(1)?;
                        if i == size - 1 {
                            prefix_chars[i + 1] = 0;
                        }
                    }
                }
                preceding = byte(host, wordx.advance(-1)?)?;
                host.write(wordx.advance(-1)?, b' ')?;
                confirm = true;
                word_flags |= 0x800000;
                if prefix_type & BREAK != 0 {
                    let mut prefix_source = host.prefix(&prefix_chars)?;
                    copy(&mut prefix, &phon)?;
                    found = list(host, &mut prefix_source, &mut phon, &mut flags, 0)?;
                    if found {
                        copy(&mut prefix, &phon)?;
                    }
                    if flags[0] & ABBREV != 0 {
                        prefix[0] = 0;
                        host.spell(&mut prefix_source, &mut prefix, 1)?;
                        length(&prefix)?;
                    }
                } else {
                    append_truncated(&mut prefix, &ending)?;
                }
                ending[0] = 0;
                end_type = 0;
                found = list(host, &mut wordx, &mut phon, &mut flags2, PREFIX)?;
                if flags[0] == 0 {
                    flags = flags2;
                } else {
                    prefix_flags = true;
                }
                if !found {
                    end_type = rules(
                        host,
                        wordx,
                        &mut phon,
                        Some(&mut ending),
                        word_flags & 0x804000,
                        &mut flags,
                    )?;
                    if phon[0] == SWITCH {
                        host.write(wordx.advance(-1)?, preceding)?;
                        host.publish(&phon, false)?;
                        return Ok(0);
                    }
                }
            }
            if end_type != 0 && end_type & PREFIX == 0 {
                end_type1 = end_type;
                let mut previous_phon = [0; PHONEMES];
                copy(&mut previous_phon, &phon)?;
                let mut end_flags = host.remove(wordx, end_type, Some(&mut saved))?;
                let mut more = true;
                let mut passes = 0usize;
                while more {
                    passes += 1;
                    if passes > 65536 {
                        return Err(Error::State);
                    }
                    more = false;
                    phon[0] = 0;
                    if prefix[0] != 0 {
                        host.write(wordx.advance(-1)?, preceding)?;
                        found = list(host, &mut word1, &mut phon, &mut flags2, end_flags)?;
                        host.write(wordx.advance(-1)?, b' ')?;
                        if phon[0] == SWITCH {
                            restore(host, wordx, &saved)?;
                            host.publish(&phon, false)?;
                            return Ok(0);
                        }
                        if flags[0] == 0 {
                            flags = flags2;
                        }
                        if found {
                            prefix[0] = 0;
                        }
                        if !found && flags2[0] != 0 {
                            prefix_flags = true;
                        }
                    }
                    if !found {
                        found = list(host, &mut wordx, &mut phon, &mut flags2, end_flags)?;
                        if phon[0] == SWITCH {
                            restore(host, wordx, &saved)?;
                            host.publish(&phon, false)?;
                            return Ok(0);
                        }
                        if flags[0] == 0 {
                            flags = flags2;
                        }
                    }
                    if !found {
                        if end_type & 0x4000 != 0 {
                            copy(&mut phon, &previous_phon)?;
                        } else {
                            if end_flags & 4 != 0 {
                                word_flags |= 0x2000;
                            }
                            if end_type & 0x40000 != 0 {
                                word_flags |= 0x08000000;
                            }
                            if end_type & MORE != 0 {
                                let previous = ending;
                                end_type = rules(
                                    host,
                                    wordx,
                                    &mut phon,
                                    Some(&mut ending),
                                    word_flags,
                                    &mut flags,
                                )?;
                                append_truncated(&mut ending, &previous)?;
                                if end_type != 0 && end_type & PREFIX == 0 {
                                    let before = (
                                        signature(host, wordx)?,
                                        byte(host, wordx.advance(-1)?)?,
                                        host.value(Field::Verb, 0),
                                        end_flags,
                                    );
                                    end_flags = host.remove(wordx, end_type, None)?;
                                    let after = (
                                        signature(host, wordx)?,
                                        byte(host, wordx.advance(-1)?)?,
                                        host.value(Field::Verb, 0),
                                        end_flags,
                                    );
                                    // A zero-character suffix may update verb/ending
                                    // context without shortening the stem. Repairs
                                    // may also change only its predecessor or flags.
                                    // Reject a failed positive removal; zero-length
                                    // retry chains retain the finite budget above.
                                    if end_type & 0x3f != 0 && after == before {
                                        return Err(Error::State);
                                    }
                                    more = true;
                                }
                            } else {
                                rules(host, wordx, &mut phon, None, word_flags, &mut flags)?;
                                end_type = 0;
                            }
                            if phon[0] == SWITCH {
                                host.publish(&phon, false)?;
                                restore(host, wordx, &saved)?;
                                host.write(wordx.advance(-1)?, preceding)?;
                                return Ok(0);
                            }
                        }
                    }
                }
                if end_type1 & AFTER_STRESS == 0 {
                    host.append(&mut phon, &ending)?;
                    length(&phon)?;
                    ending[0] = 0;
                }
                restore(host, wordx, &saved)?;
            }
            host.write(wordx.advance(-1)?, preceding)?;
        }
    }
    host.plural(word_flags, last)?;
    word_flags |= emphasized;
    let mut prefix_stress = 0;
    for b in &prefix[..length(&prefix)?] {
        if matches!(*b, 6 | 7) {
            prefix_stress = *b;
        }
    }
    if prefix_flags || prefix_stress != 0 {
        if host.value(Field::Prefixes, 0) != 0 || prefix_type & AFTER_STRESS != 0 {
            host.stress(Some(&mut phon), &mut flags, 3, 0)?;
            length(&phon)?;
            let mut seen = false;
            let size = length(&prefix)?;
            for b in &mut prefix[..size] {
                if *b == 6 {
                    if seen {
                        *b = 5;
                    } else {
                        seen = true;
                    }
                }
            }
        }
        host.publish(&joined([&unpron, &prefix, &phon])?, true)?;
        host.stress(None, &mut flags, -1, 0)?;
    } else {
        host.stress(
            Some(&mut phon),
            &mut flags,
            -1,
            if ending[0] != 0 { 2 } else { 0 },
        )?;
        length(&phon)?;
        host.publish(&joined([&unpron, &prefix, &phon])?, true)?;
    }
    if ending[0] != 0 {
        let mut current = [0; PHONEMES];
        host.snapshot(&mut current)?;
        append_truncated(&mut current, &ending)?;
        host.publish(&current, false)?;
    }
    if word_flags & 0x10 != 0 {
        flags[0] &= !0x10000000;
    }
    if word_flags & 0x80 != 0 && host.value(Field::StressFlags, 0) & 0x100000 != 0 {
        host.change_stress(3)?;
    } else if word_flags & 0xc00 != 0 {
        host.change_stress(6)?;
        if word_flags & 0x800 != 0 {
            flags[0] |= 0x10000000;
        }
    } else {
        let skip = host.value(Field::Skip, 0);
        if skip < remaining {
            if skip < 0 {
                return Err(Error::State);
            }
            if host.value(Field::WordFlags, skip as u32) as u32 & 0x10 != 0 {
                if flags[0] & 0x600 != 0 {
                    host.change_stress(4)?;
                } else if flags[0] & 0x800 != 0 && host.value(Field::AnyPresent, 0) != 0 {
                    host.change_stress(3)?;
                }
            }
        }
    }
    if end_type1 & 0x2000 != 0 {
        host.store(Store::Verb, 2);
        host.store(Store::VerbS, 2);
    }
    if flags[1] & 8 != 0 {
        host.store(Store::Past, 3);
        host.store(Store::Verb, 0);
        host.store(Store::Noun, 0);
    } else if flags[1] & 1 != 0 {
        host.store(Store::Verb, 2);
        host.store(Store::VerbS, 0);
        host.store(Store::Noun, 0);
    } else if flags[1] & 2 != 0 {
        host.store(Store::Verb, 0);
        host.store(Store::VerbS, 2);
        host.store(Store::Past, 0);
        host.store(Store::Noun, 0);
    } else if flags[1] & 4 != 0 {
        host.store(Store::Noun, 2);
        host.store(Store::Verb, 0);
        host.store(Store::VerbS, 0);
        host.store(Store::Past, 0);
    }
    if byte(host, wordx)? != 0 && flags[1] & 0x100 == 0 {
        for (field, store) in [
            (Field::Verb, Store::Verb),
            (Field::VerbS, Store::VerbS),
            (Field::Noun, Store::Noun),
            (Field::Past, Store::Past),
        ] {
            let value = host.value(field, 0);
            if value > 0 {
                host.store(store, value - 1);
            }
        }
    }
    if count == 1
        && host.value(Field::Language, 0) == 0x656e
        && host.locale(first, false)
        && first != u32::from(b'i')
    {
        flags[0] |= 0x01000000;
    }
    if host.value(Field::Alt, 0) & 2 != 0 && flags[0] & 0x18000 != 0 {
        host.special(flags[0])?;
    }
    flags[0] |= was_unpron;
    for (i, b) in original[..original_length].iter().enumerate() {
        host.write(start.advance(i as isize)?, *b)?;
    }
    Ok(flags[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Reply {
        phon: Vec<u8>,
        ending: Vec<u8>,
        flags: [u32; 2],
        result: i32,
    }
    impl Reply {
        fn phon(bytes: &[u8], result: i32) -> Self {
            Self {
                phon: bytes.to_vec(),
                result,
                ..Self::default()
            }
        }
    }
    struct Fixture {
        sources: [Vec<u8>; 3],
        values: [i32; 20],
        rows: [u32; 3],
        output: [u8; PHONEMES],
        lists: VecDeque<Reply>,
        rules: VecDeque<Reply>,
        calls: Vec<&'static str>,
        stress_calls: Vec<(bool, i32, i32, Vec<u8>)>,
        changes: Vec<i32>,
        list_skip: i32,
        text_source: bool,
        number_switch: bool,
        fresh_replaced: bool,
        spell_switch: bool,
        bad_pron: bool,
        unpron: bool,
        consumed: usize,
        remove_progress: bool,
        publish_failure: bool,
    }
    fn fill(out: &mut [u8; PHONEMES], text: &[u8]) {
        assert!(text.len() < PHONEMES);
        out[..text.len()].copy_from_slice(text);
        out[text.len()] = 0;
    }
    impl Fixture {
        fn new(text: &[u8]) -> Self {
            let mut source = b"  !".to_vec();
            source.extend_from_slice(text);
            source.extend_from_slice(b"\0\0");
            let mut f = Self {
                sources: [source, b"  replaced \0".to_vec(), vec![0; 65]],
                values: [0; 20],
                rows: [0; 3],
                output: [0x5a; PHONEMES],
                lists: VecDeque::new(),
                rules: VecDeque::new(),
                calls: vec![],
                stress_calls: vec![],
                changes: vec![],
                list_skip: 0,
                text_source: false,
                number_switch: false,
                fresh_replaced: false,
                spell_switch: false,
                bad_pron: false,
                unpron: false,
                consumed: 1,
                remove_progress: true,
                publish_failure: false,
            };
            f.values[Field::Ready as usize] = 1;
            f.values[Field::Remaining as usize] = 3;
            fill(&mut f.output, b"owner");
            f
        }
        fn at(&self, source: Source) -> Option<usize> {
            let origin: isize = if source.slot == 2 { 1 } else { 3 };
            usize::try_from(origin.checked_add(source.offset)?).ok()
        }
        fn pronunciation(&self) -> &[u8] {
            &self.output[..length(&self.output).unwrap()]
        }
    }
    impl Host for Fixture {
        fn byte(&self, s: Source) -> Option<u8> {
            self.sources.get(s.slot as usize)?.get(self.at(s)?).copied()
        }
        fn write(&mut self, s: Source, b: u8) -> Result<(), Error> {
            let i = self.at(s).ok_or(Error::Source)?;
            *self
                .sources
                .get_mut(s.slot as usize)
                .and_then(|v| v.get_mut(i))
                .ok_or(Error::Source)? = b;
            Ok(())
        }
        fn value(&self, field: Field, index: u32) -> i32 {
            if matches!(field, Field::WordFlags) {
                self.rows.get(index as usize).copied().unwrap_or(0) as i32
            } else {
                self.values[field as usize]
            }
        }
        fn store(&mut self, field: Store, value: i32) {
            let index = match field {
                Store::Skip => Field::Skip,
                Store::Verb => Field::Verb,
                Store::VerbS => Field::VerbS,
                Store::Noun => Field::Noun,
                Store::Past => Field::Past,
            };
            self.values[index as usize] = value;
        }
        fn locale(&self, code: u32, digit: bool) -> bool {
            char::from_u32(code).is_some_and(|c| {
                if digit {
                    c.is_ascii_digit()
                } else {
                    c.is_alphabetic()
                }
            })
        }
        fn list(
            &mut self,
            source: &mut Source,
            out: &mut [u8; PHONEMES],
            flags: &mut [u32; 2],
            _: i32,
        ) -> Result<bool, Error> {
            self.calls.push("list");
            let r = self.lists.pop_front().unwrap_or_default();
            fill(out, &r.phon);
            *flags = r.flags;
            self.values[Field::Skip as usize] = self.list_skip;
            if self.text_source {
                *source = Source {
                    slot: 1,
                    offset: -1,
                };
            }
            if self.bad_pron {
                out.fill(1);
            }
            Ok(r.result != 0)
        }
        fn emoji(&mut self, _: &mut Source, _: &mut [u32; 2]) -> Result<(), Error> {
            self.calls.push("emoji");
            Ok(())
        }
        fn text(&mut self, _: Source) -> Result<(), Error> {
            self.calls.push("text");
            Ok(())
        }
        fn dotted(&mut self, _: Source) -> Result<i32, Error> {
            Ok(0)
        }
        fn number_language(&mut self) -> Result<(), Error> {
            self.calls.push("number-language");
            fill(
                &mut self.output,
                if self.number_switch {
                    b"\x15fr"
                } else {
                    b"probe"
                },
            );
            if self.fresh_replaced {
                self.rows[0] |= 0x200000;
            }
            Ok(())
        }
        fn number(
            &mut self,
            roman: bool,
            _: Source,
            out: &mut [u8; PHONEMES],
            _: &mut [u32; 2],
        ) -> Result<bool, Error> {
            self.calls.push(if roman { "roman" } else { "number" });
            fill(out, b"digits");
            Ok(true)
        }
        fn spell(
            &mut self,
            _: &mut Source,
            out: &mut [u8; PHONEMES],
            _: i32,
        ) -> Result<bool, Error> {
            self.calls.push("spell");
            fill(&mut self.output, b"\x15fr");
            fill(out, b"letters");
            Ok(self.spell_switch)
        }
        fn letter(&mut self, _: Source, out: &mut [u8; PHONEMES], _: bool) -> Result<usize, Error> {
            self.calls.push("letter");
            fill(out, b"letter");
            self.unpron = false;
            Ok(self.consumed)
        }
        fn unpronounceable(&mut self, _: Source, _: i32) -> Result<bool, Error> {
            Ok(self.unpron)
        }
        fn spelling_stress(&mut self, _: &mut [u8; PHONEMES], _: i32) -> Result<(), Error> {
            Ok(())
        }
        fn rules(
            &mut self,
            _: Source,
            out: &mut [u8; PHONEMES],
            ending: Option<&mut [u8; PHONEMES]>,
            _: u32,
            flags: &mut [u32; 2],
        ) -> Result<i32, Error> {
            self.calls.push("rules");
            let r = self
                .rules
                .pop_front()
                .unwrap_or_else(|| Reply::phon(b"stem", 0));
            fill(out, &r.phon);
            if let Some(end) = ending {
                fill(end, &r.ending);
            }
            *flags = r.flags;
            Ok(r.result)
        }
        fn remove(
            &mut self,
            source: Source,
            ending: i32,
            copy: Option<&mut [u8; WORD]>,
        ) -> Result<i32, Error> {
            self.calls.push("remove");
            let count = signature(self, source)?.0;
            if let Some(copy) = copy {
                for (i, b) in copy[..count].iter_mut().enumerate() {
                    *b = byte(self, source.advance(i as isize)?)?;
                }
                copy[count] = 0;
            }
            if ending & 0x800 != 0 && self.values[Field::Verb as usize] == 0 {
                self.values[Field::Verb as usize] = 1;
            }
            if self.remove_progress && count > 0 && ending & 0x3f != 0 {
                self.write(source.advance(count as isize - 1)?, b' ')?;
            }
            Ok(4)
        }
        fn prefix(&mut self, source: &[u8; 65]) -> Result<Source, Error> {
            self.sources[2].copy_from_slice(source);
            Ok(Source { slot: 2, offset: 0 })
        }
        fn trace_suffix(&mut self, _: &[u8; PHONEMES]) {
            self.calls.push("trace");
        }
        fn append(&mut self, out: &mut [u8; PHONEMES], end: &[u8; PHONEMES]) -> Result<(), Error> {
            self.calls.push("append");
            append_truncated(out, end)
        }
        fn plural(&mut self, _: u32, _: u32) -> Result<(), Error> {
            self.calls.push("plural");
            fill(&mut self.output, b"plural-effect");
            Ok(())
        }
        fn stress(
            &mut self,
            out: Option<&mut [u8; PHONEMES]>,
            _: &mut [u32; 2],
            position: i32,
            control: i32,
        ) -> Result<(), Error> {
            self.calls.push("stress");
            let shared = out.is_none();
            let out = out.unwrap_or(&mut self.output);
            self.stress_calls
                .push((shared, position, control, out[..length(out)?].to_vec()));
            Ok(())
        }
        fn snapshot(&self, out: &mut [u8; PHONEMES]) -> Result<(), Error> {
            copy(out, &self.output)
        }
        fn publish(&mut self, out: &[u8; PHONEMES], joined: bool) -> Result<(), Error> {
            self.calls.push("publish");
            if self.publish_failure {
                return Err(Error::State);
            }
            copy(&mut self.output, out)?;
            if joined {
                self.output[199] = 0;
            }
            Ok(())
        }
        fn change_stress(&mut self, level: i32) -> Result<(), Error> {
            self.changes.push(level);
            Ok(())
        }
        fn special(&mut self, _: u32) -> Result<(), Error> {
            self.calls.push("special");
            Ok(())
        }
    }

    #[test]
    fn unloaded_dictionary_resets_skip_without_source_access() {
        let mut f = Fixture::new(b"cat ");
        f.values[Field::Ready as usize] = 0;
        f.values[Field::Skip as usize] = 9;
        f.sources[0].clear();
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(f.calls, ["publish"]);
        assert_eq!(f.values[Field::Skip as usize], 0);
        assert!(f.pronunciation().is_empty());
    }
    #[test]
    fn key_lookup_retains_predecessor_and_normal_restore_preserves_word() {
        let mut f = Fixture::new(b"enter ");
        f.values[Field::SayAs as usize] = 0x24;
        f.lists.push_back(Reply::phon(b"key", 1));
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(&f.sources[0][2..9], b"_enter ");
        assert_eq!(f.pronunciation(), b"key");
        assert_eq!(f.output[199], 0);
    }
    #[test]
    fn text_replacement_returns_before_normal_restore_and_output_join() {
        let mut f = Fixture::new(b"cat . ");
        f.text_source = true;
        f.lists.push_back(Reply {
            flags: [TEXT | 0x01000000, 0],
            ..Reply::default()
        });
        assert_eq!(translate(&mut f), Ok(TEXT | 0x01000000));
        assert_eq!(&f.sources[0][3..], b"cat   \0\0");
        assert_eq!(f.calls, ["list", "text"]);
        assert_eq!(f.pronunciation(), b"owner");
        assert_eq!(f.output[199], 0x5a);
    }
    #[test]
    fn number_probe_preserves_shared_switch_and_reads_fresh_replaced_flag() {
        let mut f = Fixture::new(b"123 ");
        f.number_switch = true;
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(f.pronunciation(), b"\x15fr");
        assert!(!f.calls.contains(&"number"));
        let mut f = Fixture::new(b"123 ");
        f.values[Field::Numbers2 as usize] = 0x8000;
        f.fresh_replaced = true;
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(f.pronunciation(), b"digits");
        assert!(f.calls.contains(&"number"));
    }
    #[test]
    fn spelling_switch_retains_primitive_output_without_parent_publication() {
        let mut f = Fixture::new(b"abc ");
        f.values[Field::SayAs as usize] = 0x12;
        f.spell_switch = true;
        assert_eq!(translate(&mut f), Ok(SPELL));
        assert_eq!(f.pronunciation(), b"\x15fr");
        assert_eq!(f.calls, ["spell"]);
    }
    #[test]
    fn stacked_suffixes_restore_source_and_keep_after_stress_order() {
        let mut f = Fixture::new(b"cats ");
        let source = f.sources[0].clone();
        f.rules.push_back(Reply {
            phon: b"stem".to_vec(),
            ending: b"s".to_vec(),
            result: MORE | AFTER_STRESS | 1,
            ..Reply::default()
        });
        f.rules.push_back(Reply {
            phon: b"stem2".to_vec(),
            ending: b"t".to_vec(),
            result: MORE | 1,
            ..Reply::default()
        });
        f.rules.push_back(Reply::phon(b"final", 0));
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(f.sources[0], source);
        assert_eq!(f.pronunciation(), b"finalts");
        assert_eq!(f.stress_calls, [(false, -1, 2, b"final".to_vec())]);
        assert!(!f.calls.contains(&"append"));
    }
    #[test]
    fn prefix_primary_stress_is_reduced_before_shared_owner_stress() {
        let mut f = Fixture::new(b"reword ");
        f.values[Field::Prefixes as usize] = 1;
        f.rules.push_back(Reply {
            phon: b"whole".to_vec(),
            ending: vec![6, b'p', 6, b'q'],
            result: PREFIX | 2,
            ..Reply::default()
        });
        f.rules.push_back(Reply::phon(b"probe", 0));
        f.rules.push_back(Reply::phon(b"stem", 0));
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(
            f.pronunciation(),
            &[6, b'p', 5, b'q', b's', b't', b'e', b'm']
        );
        assert_eq!(f.stress_calls[0], (false, 3, 0, b"stem".to_vec()));
        assert_eq!(f.stress_calls[1], (true, -1, 0, f.pronunciation().to_vec()));
        assert_eq!(&f.sources[0][3..], b"reword \0\0");
    }
    #[test]
    fn end_unstress_uses_pointer_presence_and_grammar_state_is_fresh() {
        let mut f = Fixture::new(b"cat next ");
        f.rows[1] = 0x10;
        f.list_skip = 1;
        f.values[Field::AnyPresent as usize] = 1;
        f.lists.push_back(Reply {
            phon: b"cat".to_vec(),
            flags: [0x800, 8],
            result: 1,
            ..Reply::default()
        });
        assert_eq!(translate(&mut f), Ok(0x800));
        assert_eq!(f.changes, [3]);
        assert_eq!(f.values[Field::Past as usize], 2);
        assert_eq!(f.values[Field::Verb as usize], 0);
    }
    #[test]
    fn malformed_pronunciation_and_failed_publication_never_replay() {
        let mut f = Fixture::new(b"cat ");
        f.bad_pron = true;
        assert_eq!(translate(&mut f), Err(Error::Phonemes));
        assert_eq!(f.calls, ["list"]);
        let mut f = Fixture::new(b"cat ");
        f.publish_failure = true;
        f.lists.push_back(Reply::phon(b"cat", 1));
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.pronunciation(), b"plural-effect");
        assert_eq!(f.calls.iter().filter(|c| **c == "list").count(), 1);
    }
    #[test]
    fn nonprogressing_letter_and_suffix_reject_after_executed_effects() {
        let mut f = Fixture::new(b"abcdefgh ");
        f.unpron = true;
        f.consumed = 0;
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.calls, ["list", "letter"]);
        let mut f = Fixture::new(b"cats ");
        f.remove_progress = false;
        for _ in 0..2 {
            f.rules.push_back(Reply {
                phon: b"stem".to_vec(),
                ending: b"s".to_vec(),
                result: MORE | 1,
                ..Reply::default()
            });
        }
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.calls.iter().filter(|c| **c == "remove").count(), 2);
    }
    #[test]
    fn zero_character_suffix_can_advance_grammatical_context() {
        let mut f = Fixture::new(b"cats ");
        f.rules.push_back(Reply {
            phon: b"first".to_vec(),
            ending: b"s".to_vec(),
            result: MORE | 1,
            ..Reply::default()
        });
        f.rules.push_back(Reply {
            phon: b"second".to_vec(),
            result: MORE | 0x800,
            ..Reply::default()
        });
        f.rules.push_back(Reply::phon(b"final", 0));
        assert_eq!(translate(&mut f), Ok(0));
        assert_eq!(f.calls.iter().filter(|c| **c == "remove").count(), 2);
        assert_eq!(f.pronunciation(), b"finals");
        assert_eq!(&f.sources[0][3..], b"cats \0\0");
    }
    #[test]
    fn checked_source_offsets_and_final_join_are_bounded() {
        assert_eq!(
            Source {
                slot: 0,
                offset: isize::MAX
            }
            .advance(1),
            Err(Error::Source)
        );
        assert_eq!(
            Source {
                slot: 0,
                offset: isize::MIN
            }
            .advance(-1),
            Err(Error::Source)
        );
        let mut a = [b'a'; PHONEMES];
        a[199] = 0;
        let mut b = [0; PHONEMES];
        fill(&mut b, b"suffix");
        let out = joined([&a, &b, &b]).unwrap();
        assert_eq!(length(&out), Ok(199));
        assert_eq!(out, a);
    }
}
