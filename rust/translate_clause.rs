//! Complete clause tokenization, word dispatch and final publication policy.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{language, phoneme_list::Source as Entry, suffix, translate_word::Error, utf8};
pub const SOURCE: usize = 800;
pub const ROWS: usize = 300;
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Field {
    EmbeddedIndex,
    EmbeddedRead,
    PrePause,
    AnyStressed,
    Characters,
    StartChar,
    Words,
    StartWord,
    MaxPause,
    NewSentence,
    Sentences,
    SkipSentences,
    Skipping,
    Count,
    LastStress,
    Timeout,
    Verb,
    Noun,
    Past,
    VerbS,
    Repeat,
    EndVowel,
    Dictionary0,
    Dictionary1,
    Terminator,
    ToneNumbers,
    SayAs,
    SayAsDigits,
    PhonemeInput,
    Offset,
    CapsInWord,
    Language,
    Apostrophe,
    WordEmphasis,
    BreakNumbers,
    Thousands,
    Decimal,
    MaxDigits,
    SkipWords,
    SkipDictionary,
    EndPause,
    OutputCount,
    Ideographs,
    Alternate,
    Alphabet,
}
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Class {
    Alpha,
    Space,
    Upper,
    Lower,
    Digit,
    Alnum,
    Bracket,
    Emoji,
    Modifier,
    Regional,
    Tag,
    Punctuation,
    Apostrophe,
    Break,
    ByteSpace,
    AnyDigit,
}
/// Explicit padding is initialized; all bytes may be copied by C primitives.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub flags: u32,
    pub start: u16,
    pub pause: u8,
    pub reserved0: u8,
    pub source: u16,
    pub length: u8,
    pub reserved1: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Read {
    pub terminator: i32,
    pub tone: i32,
    pub top: usize,
}
pub struct WordCall<'a> {
    pub clause: &'a mut [u8; SOURCE],
    pub end: usize,
    pub number: Option<(&'a mut [u8; 150], usize)>,
    pub offset: usize,
    pub rows: &'a mut [Row; ROWS + 1],
    pub row: usize,
    pub remaining: usize,
    pub pause: i32,
}
pub trait Host {
    fn value(&self, field: Field) -> i32;
    fn store(&mut self, field: Field, value: i32) -> Result<(), Error>;
    fn byte(&self, index: usize) -> Option<u8>;
    fn write(&mut self, index: usize, value: u8) -> Result<(), Error>;
    fn classify(&self, code: u32, class: Class) -> bool;
    fn lower(&self, code: u32) -> u32;
    fn read(
        &mut self,
        positions: &mut [i16; SOURCE + 4],
        voice: &mut [u8; 40],
    ) -> Result<Read, Error>;
    fn embedded_command(&mut self, index: &mut usize) -> Result<i32, Error>;
    fn translate_char(
        &mut self,
        index: usize,
        previous: u32,
        code: u32,
        next: u32,
        inserted: &mut u32,
        flags: &mut u32,
    ) -> Result<u32, Error>;
    fn upper_word(
        &mut self,
        source: &mut [u8; SOURCE],
        index: usize,
        code: u32,
    ) -> Result<bool, Error>;
    fn entry(&self, index: i32) -> Result<Entry, Error>;
    fn put(&mut self, index: i32, value: Entry) -> Result<(), Error>;
    fn command(&self, index: i32) -> Result<u32, Error>;
    fn put_command(&mut self, index: i32, value: u32) -> Result<(), Error>;
    /// Source and optional number scratch, initialized extents, numeric word
    /// and row offsets. Clause/rule contexts are scoped and restored on return.
    /// Rows include an initialized predecessor at slot zero; logical row i is
    /// slot i+1. Every mutable buffer is disjoint and no pointer is retained.
    fn translate(&mut self, call: WordCall<'_>) -> Result<u32, Error>;
    fn word_embedded(&mut self) -> Result<(), Error>;
    fn eof(&self) -> bool;
    fn make_list(&mut self, pause: i32, sentence: bool) -> Result<(), Error>;
    fn output(&mut self, index: usize, field: u32, value: u32) -> Result<(), Error>;
    /// kind 0 terminator, 1 tone, 2 voice-name or absent; output slots retain
    /// each completed publication even if a later primitive refuses.
    fn result(&mut self, kind: u32, value: i32, voice: &[u8; 40]) -> Result<(), Error>;
}
const SPACE: u32 = 32;
fn source_byte(h: &impl Host, index: usize) -> Result<u8, Error> {
    if index >= SOURCE + 40 {
        return Err(Error::Source);
    }
    h.byte(index).ok_or(Error::Source)
}
fn decode(h: &impl Host, mut index: usize, reverse: bool) -> Result<utf8::Character, Error> {
    loop {
        if source_byte(h, index)? & 0xc0 != 0x80 {
            break;
        }
        index = if reverse {
            index.checked_sub(1)
        } else {
            index.checked_add(1)
        }
        .ok_or(Error::Source)?;
    }
    utf8::head(|i| index.checked_add(i).and_then(|j| source_byte(h, j).ok()))
        .map_err(|_| Error::Source)
}
fn local(text: &[u8], index: usize, reverse: bool) -> Result<u32, Error> {
    utf8::decode(text, index, reverse)
        .map(|c| c.code)
        .map_err(|_| Error::Source)
}
fn length(
    source: usize,
    top: usize,
    positions: &[i16; SOURCE + 4],
    row: &mut Row,
) -> Result<u8, Error> {
    let mut i = source;
    while i < top && *positions.get(i).ok_or(Error::Source)? <= 0 {
        i += 1;
    }
    row.source = *positions.get(i).ok_or(Error::Source)? as u16;
    let mut count = 0u8;
    while *positions.get(i).ok_or(Error::Source)? != 0 {
        if positions[i] != -1 {
            count = count.wrapping_add(1);
        }
        i += 1;
    }
    Ok(count)
}
fn terminate_commands(h: &mut impl Host) -> Result<(), Error> {
    let i = h
        .value(Field::EmbeddedIndex)
        .checked_sub(1)
        .ok_or(Error::State)?;
    h.put_command(i, h.command(i)? | 0x80)
}
fn add_space(source: &mut [u8; SOURCE], index: &mut usize, row: &mut Row) -> Result<(), Error> {
    *source.get_mut(*index).ok_or(Error::Capacity)? = b' ';
    *index += 1;
    row.start = row.start.checked_add(1).ok_or(Error::Capacity)?;
    Ok(())
}
fn shift(mask: i32, position: i32) -> bool {
    // The old signed shifts can use a negative count for two/one-digit groups.
    // Such bits do not designate an admissible number boundary.
    (0..32).contains(&position) && (mask as u32) & (1u32 << position) != 0
}
pub fn translate(h: &mut impl Host) -> Result<(), Error> {
    let mut positions = [0; SOURCE + 4];
    let mut voice = [0; 40];
    let mut sbuf = [0; SOURCE];
    let mut words = [Row::default(); ROWS + 1];
    for f in [
        Field::EmbeddedIndex,
        Field::EmbeddedRead,
        Field::PrePause,
        Field::AnyStressed,
    ] {
        h.store(f, 0)?;
    }
    h.store(Field::StartChar, h.value(Field::Characters).max(0))?;
    h.store(Field::StartWord, h.value(Field::Words).wrapping_add(1))?;
    let read = h.read(&mut positions, &mut voice)?;
    if read.top >= SOURCE {
        return Err(Error::Source);
    }
    let mut terminator = read.terminator;
    h.result(0, terminator, &voice)?;
    h.result(
        1,
        if read.tone == 0 {
            (terminator & 0x7000) >> 12
        } else {
            read.tone
        },
        &voice,
    )?;
    positions[read.top + 1] = 0;
    positions[read.top + 2] = 0x7fff;
    positions[read.top + 3] = 0;
    let mut pause = (terminator & 0xfff) * 10;
    if terminator & 0x800000 != 0 {
        pause *= 32;
    }
    let mut p = 0;
    while source_byte(h, p)? != 0 && h.classify(source_byte(h, p)? as u32, Class::ByteSpace) {
        p += 1;
    }
    let sentence = if source_byte(h, p)? == 0 {
        pause = pause
            .checked_sub(h.value(Field::MaxPause))
            .ok_or(Error::State)?
            .max(0);
        if h.value(Field::NewSentence) != 0 {
            terminator |= 0x80000;
        }
        h.store(
            Field::MaxPause,
            h.value(Field::MaxPause).wrapping_add(pause),
        )?;
        false
    } else {
        h.store(Field::MaxPause, pause)?;
        h.value(Field::NewSentence) != 0
    };
    h.store(Field::Terminator, terminator)?;
    if sentence {
        h.store(Field::Sentences, h.value(Field::Sentences).wrapping_add(1))?;
        if h.value(Field::SkipSentences) > 0 {
            h.store(Field::SkipSentences, h.value(Field::SkipSentences) - 1)?;
            if h.value(Field::SkipSentences) == 0 {
                h.store(Field::Skipping, 0)?;
            }
        }
    }
    h.put(
        0,
        Entry {
            code: 10,
            ..Entry::default()
        },
    )?;
    h.store(Field::Count, 1)?;
    for f in [
        Field::LastStress,
        Field::Timeout,
        Field::Verb,
        Field::Noun,
        Field::Past,
        Field::VerbS,
        Field::Repeat,
        Field::EndVowel,
        Field::Dictionary0,
        Field::Dictionary1,
    ] {
        h.store(f, 0)?;
    }
    sbuf[1] = b' ';
    sbuf[2] = b' ';
    let mut ix = 3;
    let mut count = 0usize;
    let mut src = 0usize;
    let mut word_source = 0usize;
    let mut prev_in = SPACE;
    let mut prev_out = SPACE;
    let mut prev_save = 0;
    let mut inserted = 0;
    let mut pause_add = 0;
    let mut upper = 1u32;
    let mut alpha_count = 0;
    let mut finished = false;
    let mut quoted = false;
    let mut phonemes = false;
    let mut flags = 0u32;
    let mut next_flags = 0u32;
    let mut embedded = 0i32;
    let mut letters = 0;
    let mut space_inserted = false;
    let mut syllable = false;
    let mut decimal = false;
    let mut cc = 0;
    words[1].start = 3;
    words[1].length = length(src, read.top, &positions, &mut words[1])?;
    // Source rewinds/insertion can reprocess a character. Bound invalid
    // primitive cycles without imposing a polling or scheduling cadence.
    let mut steps = 0;
    while !finished && ix < SOURCE - 1 {
        steps += 1;
        if steps > SOURCE * 8 {
            return Err(Error::State);
        }
        let prev_out2 = prev_out;
        prev_out = local(&sbuf, ix - 1, true)?;
        if h.value(Field::ToneNumbers) != 0
            && (48..=57).contains(&prev_out)
            && h.classify(prev_out2, Class::Alpha)
        {
            prev_out = 97;
        }
        if prev_save != 0 {
            prev_in = prev_save;
            prev_save = 0;
        } else if src > 0 {
            prev_in = decode(h, src - 1, true)?.code;
        }
        let previous_src = src;
        let mut c = if inserted != 0 {
            let c = inserted;
            inserted = 0;
            c
        } else {
            let ch = decode(h, src, false)?;
            src = src.checked_add(ch.width).ok_or(Error::Source)?;
            cc = ch.code;
            cc
        };
        let (mut next, next_width) = if c == 0 {
            finished = true;
            c = SPACE;
            (SPACE, 0)
        } else {
            let ch = decode(h, src, false)?;
            (ch.code, ch.width)
        };
        if c == 1 {
            let begin = src.checked_sub(1).ok_or(Error::Source)?;
            if prev_in != SPACE {
                c = SPACE;
                prev_save = c;
                src -= 1;
            } else {
                embedded = embedded
                    .checked_add(h.embedded_command(&mut src)?)
                    .ok_or(Error::State)?;
                prev_save = prev_in;
                if src < begin || src > SOURCE {
                    return Err(Error::Source);
                }
                for i in begin..src {
                    h.write(i, b' ')?;
                }
                src = begin;
                continue;
            }
        }
        if h.value(Field::SayAs) == 0x24 && c != SPACE {
            if prev_in == SPACE && next == SPACE {
                h.store(Field::SayAs, 0x14)?;
            }
            c = h.lower(c);
        }
        if phonemes {
            upper = 8;
            if c == 93 && next == 93 {
                phonemes = false;
                src += 1;
                c = SPACE;
            }
        } else if h.value(Field::SayAs) & 0xf0 == 0x40 {
            if h.classify(c, Class::Digit) {
                h.store(
                    Field::SayAsDigits,
                    h.value(Field::SayAsDigits).wrapping_add(1),
                )?;
                if h.value(Field::SayAsDigits) > h.value(Field::SayAs) & 0xf {
                    c = SPACE;
                    space_inserted = true;
                    h.store(Field::SayAsDigits, 0)?;
                }
            } else {
                h.store(Field::SayAsDigits, 0)?;
                if h.classify(prev_out, Class::Digit) {
                    c = SPACE;
                    space_inserted = true;
                }
            }
        } else if h.value(Field::SayAs) & 0x10 == 0 {
            if matches!(c, 0x92 | 0xb4 | 0x2019 | 0x2032) {
                c = 39;
            }
            if matches!(c, 0x2018 | 63)
                && h.classify(prev_out, Class::Alpha)
                && h.classify(next, Class::Alpha)
            {
                c = 39;
            }
            if c == 0x530 {
                c = SPACE;
                flags |= 0x400;
            }
            if c == 0x557 {
                c = SPACE;
                flags |= 0x20000;
            }
            c = h.translate_char(src, prev_in, c, next, &mut inserted, &mut flags)?;
            if matches!(c, 8 | 0xfe0e | 0xfe0f) {
                continue;
            }
            if inserted != 0 {
                next = inserted;
            }
            if !h.classify(c, Class::Alpha)
                && !h.classify(c, Class::Space)
                && !h.classify(c, Class::Punctuation)
                && h.classify(prev_out, Class::Alpha)
            {
                let tone = h.value(Field::ToneNumbers) != 0
                    && (48..=57).contains(&c)
                    && !(48..=57).contains(&next);
                let join = c == 0x200d
                    && h.classify(prev_out, Class::Emoji)
                    && h.classify(next, Class::Emoji);
                let tag = h.classify(c, Class::Tag) && h.classify(prev_out, Class::Emoji);
                if !tone && !join && !tag {
                    c = SPACE;
                    space_inserted = true;
                }
            }
            if h.classify(prev_out, Class::Digit) {
                if !h.classify(c, Class::Digit) && !matches!(c, 46 | 44 | 32) {
                    c = SPACE;
                    space_inserted = true;
                }
            } else if prev_in == 44 {
                if c == 44 {
                    c = SPACE;
                }
            } else {
                decimal = false;
            }
            if c == 91 && (next == 2 || (next == 91 && h.value(Field::PhonemeInput) != 0)) {
                phonemes = true;
                src += 1;
                continue;
            }
            if h.classify(c, Class::Alpha) {
                let mut emoji_join = false;
                let mut emoji_break = false;
                alpha_count += 1;
                if prev_out == 0x200d && h.classify(c, Class::Emoji) {
                    emoji_join = h.classify(
                        local(&sbuf, words[count + 1].start as usize, false)?,
                        Class::Emoji,
                    );
                } else if h.classify(c, Class::Emoji) && h.classify(prev_out, Class::Emoji) {
                    if h.classify(c, Class::Modifier) {
                    } else if h.classify(c, Class::Regional)
                        && h.classify(prev_out, Class::Regional)
                    {
                        let mut ri = 0;
                        let mut j = ix;
                        while j > words[count + 1].start as usize {
                            if !h.classify(local(&sbuf, j - 1, true)?, Class::Regional) {
                                break;
                            }
                            ri += 1;
                            j = j.checked_sub(4).ok_or(Error::Source)?;
                        }
                        emoji_break = ri & 1 == 0;
                    } else {
                        emoji_break = true;
                    }
                }
                let a = language::alphabet_from_char(c as i32);
                let b = language::alphabet_from_char(prev_out as i32);
                let foreign = a.is_some_and(|a| a.offset != h.value(Field::Offset));
                let previous_foreign = b.is_some_and(|a| a.offset != h.value(Field::Offset));
                if !emoji_join
                    && (emoji_break
                        || !h.classify(prev_out, Class::Alpha)
                        || should_split(h, c, prev_out)
                        || h.classify(c, Class::Emoji) != h.classify(prev_out, Class::Emoji)
                        || foreign != previous_foreign)
                {
                    if !h.classify(prev_out, Class::Punctuation) {
                        letters = 0;
                    }
                    if prev_out != SPACE && !h.classify(prev_out, Class::Punctuation) {
                        c = SPACE;
                        space_inserted = true;
                        if !h.classify(prev_out, Class::Bracket) {
                            next_flags |= 0x100;
                        }
                    } else {
                        if h.classify(c, Class::Upper) {
                            flags |= 2;
                        }
                        if prev_out == SPACE
                            && h.classify(sbuf[ix - 2] as u32, Class::Digit)
                            && !h.classify(prev_in, Class::Digit)
                        {
                            add_space(&mut sbuf, &mut ix, &mut words[count + 1])?;
                        }
                    }
                }
                if c != SPACE {
                    letters += 1;
                    let offset = h.value(Field::Offset) as u32;
                    if offset > 0
                        && ((c < 0x250 && prev_out >= offset)
                            || (c >= offset && letters > 1 && prev_out < 0x250))
                        && h.classify(prev_out, Class::Alpha)
                    {
                        c = SPACE;
                        space_inserted = true;
                        flags |= 0x4000;
                        next_flags |= 0x80;
                    }
                }
                if h.classify(c, Class::Upper) {
                    c = h.lower(c);
                    if h.value(Field::CapsInWord) != 0 {
                        if !syllable {
                            inserted = c;
                            c = 0x2c8;
                            syllable = true;
                        }
                    } else if h.classify(prev_in, Class::Lower) {
                        if prev_out != SPACE && !h.upper_word(&mut sbuf, ix, c)? {
                            c = SPACE;
                            space_inserted = true;
                            prev_save = c;
                        }
                    } else if c != SPACE
                        && h.classify(prev_in, Class::Upper)
                        && h.classify(next, Class::Lower)
                    {
                        let next2 = decode(h, src + next_width, false)?.code;
                        if h.value(Field::Language) == 0x6e6c
                            && letters == 2
                            && c == 106
                            && prev_in == 73
                        {
                        } else if prev_out != SPACE && h.classify(next2, Class::Alpha) {
                            c = SPACE;
                            space_inserted = true;
                            prev_save = c;
                            next_flags |= 0x100;
                        }
                    }
                } else if upper != 0 && letters > 2 {
                    if h.value(Field::Language) == 0x656e && c == 115 && next == SPACE {
                        c = SPACE;
                        upper |= 4;
                        if sbuf[ix - 1] == b'\'' {
                            sbuf[ix - 1] = b' ';
                        }
                    } else {
                        upper = 0;
                    }
                } else {
                    upper = 0;
                }
            } else if c == 45 {
                if !h.classify(prev_in, Class::Space) && h.classify(next, Class::Alpha) {
                    if prev_out != SPACE {
                        c = SPACE;
                        space_inserted = true;
                    } else {
                        flags |= 0x80;
                        if count > 0 {
                            words[count].flags |= 0x4000;
                        }
                        c = SPACE;
                    }
                } else if prev_in == SPACE && next == SPACE {
                    c = SPACE;
                    pause_add = 4;
                } else if next == 45 {
                    src += 1;
                    c = SPACE;
                    pause_add = 4;
                } else if prev_out == SPACE
                    && h.classify(prev_out2, Class::Alpha)
                    && !h.classify(prev_in, Class::Alpha)
                {
                    add_space(&mut sbuf, &mut ix, &mut words[count + 1])?;
                }
            } else if c == 46 {
                if prev_out == 46 {
                    c = SPACE;
                    space_inserted = true;
                } else if count > 0
                    && words[count].flags & 0x100 == 0
                    && h.classify(prev_in, Class::Alpha)
                {
                    words[count].flags |= 0x10000;
                    if h.classify(next, Class::Space) || next == 45 {
                        c = SPACE;
                    }
                }
            } else if c == 39 {
                if ((prev_in == 46 && next == 115) || h.classify(prev_in, Class::Alnum))
                    && h.classify(next, Class::Alpha)
                    || h.value(Field::Apostrophe) & 1 != 0 && h.classify(next, Class::Alpha)
                    || h.value(Field::Apostrophe) & 2 != 0 && h.classify(prev_in, Class::Alpha)
                {
                    quoted = false;
                } else if h.classify(prev_in, Class::Apostrophe) && prev_out2 == SPACE {
                    quoted = false;
                    if next == SPACE {
                        src += 1;
                    }
                } else if prev_out == 115 && !quoted {
                    c = SPACE;
                } else {
                    quoted = h.classify(prev_out, Class::Space);
                    pause_add = 4;
                    c = SPACE;
                }
            } else if h.classify(c, Class::Break) {
                c = SPACE;
            } else if h.classify(c, Class::Digit) {
                if h.value(Field::ToneNumbers) != 0
                    && h.classify(prev_out, Class::Alpha)
                    && !h.classify(next, Class::AnyDigit)
                {
                } else if prev_out != SPACE && !h.classify(prev_out, Class::Digit) {
                    if prev_out != h.value(Field::Decimal) as u32
                        || (decimal && h.value(Field::Decimal) == 44)
                    {
                        c = SPACE;
                        space_inserted = true;
                    } else {
                        decimal = true;
                    }
                } else if prev_out == SPACE
                    && h.classify(prev_out2, Class::Alpha)
                    && !h.classify(prev_in, Class::Alpha)
                {
                    add_space(&mut sbuf, &mut ix, &mut words[count + 1])?;
                }
            }
        }
        if h.classify(c, Class::Space) {
            if prev_out == SPACE {
                flags |= 0x40000;
                continue;
            }
            if matches!(cc, 9 | 10) {
                next_flags |= 0x40000;
            }
            if space_inserted {
                let mut j = 0u8;
                let mut k = src.checked_sub(1).ok_or(Error::Source)?;
                while k >= word_source && *positions.get(k).ok_or(Error::Source)? != 0 {
                    if positions[k] > 0 {
                        j = j.wrapping_add(1);
                    }
                    if k == 0 {
                        break;
                    }
                    k -= 1;
                }
                words[count + 1].length = j;
            }
            word_source = src;
            *sbuf.get_mut(ix).ok_or(Error::Capacity)? = b' ';
            ix += 1;
            if count < ROWS - 1 && ix > words[count + 1].start as usize {
                if embedded > 0 {
                    terminate_commands(h)?;
                    words[count + 1].flags |= 0x40;
                    embedded = 0;
                }
                if alpha_count == 0 {
                    upper &= !1;
                }
                words[count + 1].pause = h.value(Field::PrePause) as u8;
                words[count + 1].flags |= upper | flags | h.value(Field::WordEmphasis) as u32;
                if h.value(Field::PrePause) > 0 {
                    if ix >= SOURCE {
                        return Err(Error::Capacity);
                    }
                    let start = words[count + 1].start as usize;
                    sbuf.copy_within(start..ix, start + 1);
                    sbuf[start] = b' ';
                    words[count + 1].start += 1;
                    ix += 1;
                }
                count += 1;
                words[count + 1].start = ix as u16;
                words[count + 1].flags = 0;
                words[count + 1].length = length(src, read.top, &positions, &mut words[count + 1])?;
                flags = next_flags;
                next_flags = 0;
                h.store(Field::PrePause, 0)?;
                upper = 1;
                alpha_count = 0;
                syllable = false;
            }
            if space_inserted {
                src = previous_src;
                inserted = 0;
                space_inserted = false;
            }
        } else if ix < SOURCE - 4 {
            let (bytes, width) = suffix::encode(c);
            sbuf[ix..ix + width].copy_from_slice(&bytes[..width]);
            ix += width;
        }
        if pause_add > h.value(Field::PrePause) {
            h.store(Field::PrePause, pause_add)?;
        }
        pause_add = 0;
    }
    if count == 0 && embedded > 0 {
        terminate_commands(h)?;
        words[1].flags |= 0x40;
        count = 1;
    }
    let end = ix.checked_sub(1).ok_or(Error::Source)?;
    *sbuf.get_mut(ix).ok_or(Error::Capacity)? = 0;
    words[1].pause = 0;
    words[count + 1].pause = 8;
    if count > 0 {
        let mut i = count - 1;
        while i > 0 && h.classify(sbuf[words[i + 1].start as usize] as u32, Class::Bracket) {
            i -= 1;
        }
        words[i + 1].flags |= 0x10;
        if terminator & 0x400000 != 0 && words[count].flags & 0x100 == 0 {
            words[count].flags |= 0x10000;
        }
    }
    words[1].flags |= 0x200;
    dispatch(
        h,
        &mut sbuf,
        Boundary {
            end,
            count,
            terminator,
        },
        &mut words,
        &mut pause,
        &mut voice,
    )?;
    if h.value(Field::EmbeddedRead) < h.value(Field::EmbeddedIndex) {
        h.word_embedded()?;
    }
    let n = h.value(Field::Count);
    if !(0..=998).contains(&n) {
        return Err(Error::Capacity);
    }
    for i in n..n + 2 {
        let mut e = h.entry(i)?;
        e.code = 9;
        e.stress = 0;
        e.source = src as u16;
        e.synthflags = 0;
        h.put(i, e)?;
    }
    h.store(Field::Count, n + 2)?;
    if h.eof() && (count == 0 || h.value(Field::EndPause) == 0) {
        pause = 10;
    }
    h.make_list(pause, sentence)?;
    h.output(1000, 0, 0)?;
    h.output(1000, 1, 1)?;
    if embedded != 0 {
        let n = usize::try_from(h.value(Field::OutputCount)).map_err(|_| Error::Capacity)?;
        h.output(n.checked_sub(2).ok_or(Error::Capacity)?, 2, 2)?;
        terminate_commands(h)?;
        h.put_command(h.value(Field::EmbeddedIndex), 0x80)?;
    }
    h.store(Field::NewSentence, i32::from(terminator & 0x80000 != 0))?;
    h.result(2, i32::from(terminator & 0x20000 != 0), &voice)
}
// Native alphabet resource plus fresh ideograph options, projected separately
// from the other scalar fields to avoid duplicating the replacement policy.
fn should_split(h: &impl Host, c: u32, p: u32) -> bool {
    if h.value(Field::Ideographs) != 0 && (c > 0x3040 || p > 0x3040) {
        return true;
    }
    let alternate = h.value(Field::Alternate);
    if alternate == 0 {
        return false;
    }
    let a = language::alphabet_from_char(c as i32);
    let b = language::alphabet_from_char(p as i32);
    [a, b]
        .into_iter()
        .flatten()
        .any(|a| a.offset == alternate && a.flags & 0x20 != 0)
}
struct Boundary {
    end: usize,
    count: usize,
    terminator: i32,
}
fn dispatch(
    h: &mut impl Host,
    sbuf: &mut [u8; SOURCE],
    boundary: Boundary,
    words: &mut [Row; ROWS + 1],
    pause: &mut i32,
    voice: &mut [u8; 40],
) -> Result<(), Error> {
    let Boundary {
        end,
        count,
        terminator,
    } = boundary;
    let mut i = 0usize;
    let mut number = [0u8; 150];
    let mut number_rows = [Row::default(); ROWS + 1];
    while i < count && h.value(Field::Count) < 991 {
        h.store(Field::Words, h.value(Field::Words).wrapping_add(1))?;
        if h.value(Field::SkipWords) > 0 {
            h.store(Field::SkipWords, h.value(Field::SkipWords) - 1)?;
            if h.value(Field::SkipWords) == 0 {
                h.store(Field::Skipping, 0)?;
            }
        }
        if h.value(Field::Skipping) != 0 {
            i += 1;
            continue;
        }
        // Clearing the alphabet is a scalar host effect.
        h.store(Field::Alphabet, 0)?;
        let word = words[i + 1].start as usize;
        let mut cursor = word;
        if h.classify(*sbuf.get(word).ok_or(Error::Source)? as u32, Class::Digit)
            && h.value(Field::BreakNumbers) != 0x49249248
        {
            let mut n = 0;
            while n < 130 {
                let c = *sbuf.get(cursor).ok_or(Error::Source)?;
                if h.classify(c as u32, Class::Digit) {
                    number[n] = c;
                    n += 1;
                    cursor += 1;
                } else if c as i32 == h.value(Field::Thousands)
                    && sbuf.get(cursor + 1) == Some(&b' ')
                    && h.classify(
                        *sbuf.get(cursor + 2).ok_or(Error::Source)? as u32,
                        Class::Digit,
                    )
                    && sbuf.get(cursor + 3) != Some(&b' ')
                    && sbuf.get(cursor + 4) != Some(&b' ')
                {
                    cursor += 2;
                    i += 1;
                    if i >= count {
                        return Err(Error::Source);
                    }
                } else {
                    sbuf.get_mut(word..cursor).ok_or(Error::Source)?.fill(b' ');
                    sbuf.get_mut(word..word + n)
                        .ok_or(Error::Source)?
                        .copy_from_slice(&number[..n]);
                    break;
                }
            }
            cursor = word;
        }
        let mut digits = 0;
        while h.classify(
            *sbuf.get(word + digits).ok_or(Error::Source)? as u32,
            Class::Digit,
        ) {
            digits += 1;
        }
        let mut dictionary;
        if digits > 4 && digits <= 32 {
            number[..3].fill(b' ');
            let mut n = 3;
            let mut remaining = digits as i32;
            let mut groups = 0;
            if digits as i32 > h.value(Field::MaxDigits) || sbuf[word] == b'0' {
                words[i + 1].flags |= 0x80000;
            }
            while n < 130 && groups < ROWS - 2 {
                let c = *sbuf.get(cursor).ok_or(Error::Source)?;
                cursor += 1;
                if !c.is_ascii_digit() && c as i32 != h.value(Field::Decimal) {
                    break;
                }
                number[n] = c;
                n += 1;
                remaining -= 1;
                if remaining > 0 && shift(h.value(Field::BreakNumbers), remaining) {
                    number_rows[groups + 1] = words[i + 1];
                    groups += 1;
                    if h.value(Field::Thousands) != 32 {
                        number[n] = h.value(Field::Thousands) as u8;
                        n += 1;
                    }
                    number[n] = b' ';
                    n += 1;
                    if words[i + 1].flags & 0x80000 == 0 {
                        if shift(h.value(Field::BreakNumbers), remaining - 1) {
                            number[n] = b'0';
                            number[n + 1] = b'0';
                            n += 2;
                        }
                        if shift(h.value(Field::BreakNumbers), remaining - 2) {
                            number[n] = b'0';
                            n += 1;
                        }
                    }
                }
            }
            cursor = cursor.checked_sub(1).ok_or(Error::Source)?;
            number_rows[groups + 1] = words[i + 1];
            number_rows[groups + 2] = *words.get(i + 2).ok_or(Error::Source)?;
            let num_count = groups + 2;
            for row in &mut number_rows[2..groups + 2] {
                row.flags &= !(0x40000 | 0x40);
            }
            let initialized = n
                .checked_add(17)
                .filter(|&end| end <= number.len())
                .ok_or(Error::Capacity)?;
            for j in 0..16 {
                let c = *sbuf.get(cursor + j).ok_or(Error::Source)?;
                number[n + j] = c;
                if c == 0 {
                    number[n + j..n + 16].fill(0);
                    break;
                }
            }
            number[n + 16] = 0;
            let mut at = 3;
            let mut row = 0;
            dictionary = 0;
            while at < n && row < ROWS {
                dictionary = h.translate(WordCall {
                    clause: sbuf,
                    end,
                    number: Some((&mut number, initialized)),
                    offset: at,
                    rows: &mut number_rows,
                    row,
                    remaining: num_count.checked_sub(row).ok_or(Error::Source)?,
                    pause: words[i + 1].pause as i32,
                })?;
                row += 1;
                while at < n {
                    let c = number[at];
                    at += 1;
                    if c == b' ' {
                        break;
                    }
                }
                words[i + 1].pause = 0;
            }
        } else {
            h.store(Field::PrePause, 0)?;
            let pre = words[i + 1].pause as i32;
            dictionary = h.translate(WordCall {
                clause: sbuf,
                end,
                number: None,
                offset: word,
                rows: words,
                row: i,
                remaining: count - i,
                pause: pre,
            })?;
            if h.value(Field::PrePause) > words[i + 2].pause as i32 {
                words[i + 2].pause = h.value(Field::PrePause) as u8;
                h.store(Field::PrePause, 0)?;
            }
            if dictionary & 0x1000 != 0 {
                let mut at = word;
                while *sbuf.get(at).ok_or(Error::Source)? != b' ' {
                    if sbuf[at] == 0 {
                        return Err(Error::Source);
                    }
                    number.fill(0);
                    number[1..10].fill(b' ');
                    let ch = utf8::decode(sbuf, at, false).map_err(|_| Error::Source)?;
                    number[3..3 + ch.width]
                        .copy_from_slice(sbuf.get(at..at + ch.width).ok_or(Error::Source)?);
                    h.translate(WordCall {
                        clause: sbuf,
                        end,
                        number: Some((&mut number, 150)),
                        offset: 3,
                        rows: words,
                        row: i,
                        remaining: count - i,
                        pause: 0,
                    })?;
                    at += ch.width;
                }
            }
            if dictionary & 0x3000000 != 0
                && terminator & 0x400000 != 0
                && i as i32
                    == (count as i32 - 1)
                        .checked_sub(h.value(Field::SkipDictionary))
                        .ok_or(Error::State)?
            {
                *pause = 10;
                h.result(1, 4, voice)?;
            }
        }
        if dictionary & 0x80 != 0 {
            while h.value(Field::SkipDictionary) > 0 {
                let skip = h.value(Field::SkipDictionary) as usize;
                if skip >= count - i {
                    h.store(Field::SkipDictionary, 0)?;
                    break;
                }
                words[i + skip + 1].flags |= 0x100000;
                h.store(Field::SkipDictionary, h.value(Field::SkipDictionary) - 1)?;
            }
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct Call {
        bytes: Vec<u8>,
        suffix: Vec<u8>,
        row: Row,
        remaining: usize,
        pause: i32,
        number: bool,
        initialized: usize,
    }
    struct Fixture {
        fields: [i32; 45],
        input: Vec<u8>,
        source: Vec<u8>,
        entries: Vec<Entry>,
        commands: [u32; 250],
        calls: Vec<Call>,
        results: Vec<(u32, i32)>,
        output: Vec<(usize, u32, u32)>,
        made: Option<(i32, bool)>,
        term: i32,
        tone: i32,
        returned: u32,
        skip: i32,
        fail: bool,
        insert_loop: bool,
        chars: usize,
        bad_top: bool,
        bad_voice: bool,
        count_overflow: bool,
        remove_delimiter: bool,
        read_say_as: Option<i32>,
        foreign_byte: Option<u8>,
    }
    impl Fixture {
        fn new(text: &str) -> Self {
            let mut f = Self {
                fields: [0; 45],
                input: text.as_bytes().to_vec(),
                source: vec![],
                entries: vec![
                    Entry {
                        word_stress: 0x55,
                        tone: 0x56,
                        ..Entry::default()
                    };
                    1000
                ],
                commands: [0; 250],
                calls: vec![],
                results: vec![],
                output: vec![],
                made: None,
                term: 0x480028,
                tone: 0,
                returned: 0x80000000,
                skip: 0,
                fail: false,
                insert_loop: false,
                chars: 0,
                bad_top: false,
                bad_voice: false,
                count_overflow: false,
                remove_delimiter: false,
                read_say_as: None,
                foreign_byte: None,
            };
            f.fields[Field::BreakNumbers as usize] = 0x49249248;
            f.fields[Field::Thousands as usize] = 44;
            f.fields[Field::Decimal as usize] = 46;
            f.fields[Field::MaxDigits as usize] = 15;
            f.fields[Field::Language as usize] = 0x656e;
            f.fields[Field::NewSentence as usize] = 1;
            f
        }
        fn field(&mut self, f: Field, v: i32) {
            self.fields[f as usize] = v;
        }
        fn words(&self) -> Vec<&str> {
            self.calls
                .iter()
                .map(|c| std::str::from_utf8(&c.bytes).expect("fixture emits UTF-8"))
                .collect()
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
        fn byte(&self, i: usize) -> Option<u8> {
            self.foreign_byte.or_else(|| self.source.get(i).copied())
        }
        fn write(&mut self, i: usize, v: u8) -> Result<(), Error> {
            *self.source.get_mut(i).ok_or(Error::Source)? = v;
            Ok(())
        }
        fn classify(&self, c: u32, k: Class) -> bool {
            let ch = char::from_u32(c);
            match k {
                Class::Alpha => ch.is_some_and(char::is_alphabetic) || c >= 0x1f000,
                Class::Space => ch.is_some_and(char::is_whitespace),
                Class::Upper => ch.is_some_and(char::is_uppercase),
                Class::Lower => ch.is_some_and(char::is_lowercase),
                Class::Digit | Class::AnyDigit => (48..=57).contains(&c),
                Class::Alnum => ch.is_some_and(char::is_alphanumeric),
                Class::Bracket => matches!(c, 40 | 41 | 91 | 93 | 123 | 125),
                Class::Emoji => (0x1f000..=0x1faff).contains(&c),
                Class::Modifier => (0x1f3fb..=0x1f3ff).contains(&c),
                Class::Regional => (0x1f1e6..=0x1f1ff).contains(&c),
                Class::Tag => (0xe0020..=0xe007f).contains(&c),
                Class::Punctuation => c == 39,
                Class::Apostrophe => false,
                Class::Break => c == 95,
                Class::ByteSpace => c > 0 && c <= 32,
            }
        }
        fn lower(&self, c: u32) -> u32 {
            char::from_u32(c)
                .and_then(|c| c.to_lowercase().next())
                .map_or(c, |c| c as u32)
        }
        fn read(&mut self, p: &mut [i16; SOURCE + 4], v: &mut [u8; 40]) -> Result<Read, Error> {
            self.source = self.input.clone();
            self.source.push(0);
            for (i, &b) in self.input.iter().enumerate() {
                p[i] = if b.is_ascii_whitespace() {
                    0
                } else if b & 0xc0 == 0x80 {
                    -1
                } else {
                    i as i16 + 1
                };
            }
            if self.bad_voice {
                v.fill(1);
            } else {
                v[..6].copy_from_slice(b"voice\0");
            }
            if let Some(v) = self.read_say_as {
                self.field(Field::SayAs, v);
            }
            Ok(Read {
                terminator: self.term,
                tone: self.tone,
                top: if self.bad_top {
                    SOURCE + 1
                } else {
                    self.input.len()
                },
            })
        }
        fn embedded_command(&mut self, i: &mut usize) -> Result<i32, Error> {
            while self.source.get(*i).is_some_and(u8::is_ascii_digit) {
                *i += 1;
            }
            if self.source.get(*i).copied().unwrap_or(0) == 0 {
                return Err(Error::Source);
            }
            *i += 1;
            let n = self.value(Field::EmbeddedIndex) as usize;
            self.commands[n] = 1;
            self.field(Field::EmbeddedIndex, n as i32 + 1);
            Ok(1)
        }
        fn translate_char(
            &mut self,
            _: usize,
            _: u32,
            c: u32,
            _: u32,
            insert: &mut u32,
            _: &mut u32,
        ) -> Result<u32, Error> {
            self.chars += 1;
            if self.insert_loop {
                *insert = c;
                Ok(0xfe0f)
            } else {
                Ok(c)
            }
        }
        fn upper_word(&mut self, _: &mut [u8; SOURCE], _: usize, _: u32) -> Result<bool, Error> {
            Ok(false)
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
        fn translate(&mut self, call: WordCall<'_>) -> Result<u32, Error> {
            let WordCall {
                clause,
                end,
                number,
                offset,
                rows,
                row,
                remaining,
                pause,
            } = call;
            let is_number = number.is_some();
            let (bytes, initialized) = if let Some((b, n)) = number {
                (&mut b[..], n)
            } else {
                (&mut clause[..], end + 2)
            };
            assert!(offset < initialized && initialized <= bytes.len() && row + remaining <= ROWS);
            let length = bytes[offset..initialized]
                .iter()
                .position(|b| matches!(b, 0 | b' '))
                .expect("terminated fixture word");
            self.calls.push(Call {
                bytes: bytes[offset..offset + length].to_vec(),
                suffix: bytes[offset + length..initialized].to_vec(),
                row: rows[row + 1],
                remaining,
                pause,
                number: is_number,
                initialized,
            });
            self.field(Field::Noun, self.value(Field::Noun) + 1);
            self.field(Field::SkipDictionary, self.skip);
            if self.remove_delimiter {
                bytes[offset + length] = 0;
            }
            if self.count_overflow {
                self.field(Field::Count, 1000);
            } else if rows[row + 1].flags & 0x100000 == 0 {
                self.field(Field::Count, self.value(Field::Count) + 1);
            }
            if self.fail {
                Err(Error::State)
            } else {
                Ok(self.returned)
            }
        }
        fn word_embedded(&mut self) -> Result<(), Error> {
            self.field(Field::EmbeddedRead, self.value(Field::EmbeddedIndex));
            Ok(())
        }
        fn eof(&self) -> bool {
            false
        }
        fn make_list(&mut self, p: i32, s: bool) -> Result<(), Error> {
            self.made = Some((p, s));
            self.field(Field::OutputCount, self.value(Field::Count));
            Ok(())
        }
        fn output(&mut self, i: usize, f: u32, v: u32) -> Result<(), Error> {
            if i > 1000 {
                return Err(Error::Capacity);
            }
            self.output.push((i, f, v));
            Ok(())
        }
        fn result(&mut self, k: u32, v: i32, name: &[u8; 40]) -> Result<(), Error> {
            if k == 2 && v != 0 && !name.contains(&0) {
                return Err(Error::Source);
            }
            self.results.push((k, v));
            Ok(())
        }
    }
    #[test]
    fn emoji_runs_joiners_and_flags_keep_distinct_tokens() {
        let mut f = Fixture::new("🙂🙂 👩🏽‍💻 🇦🇧🇨🇩");
        assert_eq!(translate(&mut f), Ok(()));
        assert_eq!(f.words(), vec!["🙂", "🙂", "👩🏽‍💻", "🇦🇧", "🇨🇩"]);
        assert_ne!(f.calls[0].row.flags & 0x200, 0);
        assert_ne!(f.calls[4].row.flags & 0x10, 0);
        assert_eq!(f.calls[0].remaining, 5);
    }
    #[test]
    fn decimal_grouping_preserves_suffix_context_and_clears_only_following_flags() {
        let mut f = Fixture::new(" 123456789th next");
        assert_eq!(translate(&mut f), Ok(()));
        assert_eq!(f.words(), vec!["123,", "456,", "789", "th", "next"]);
        assert!(f.calls[2].suffix.starts_with(b" th next"));
        assert_ne!(f.calls[0].row.flags & 0x40000, 0);
        assert!(f.calls[1..3].iter().all(|c| c.row.flags & 0x40000 == 0));
        assert!(f.calls[..3].iter().all(|c| c.number && c.initialized < 150));
        assert_eq!(f.calls[0].remaining, 4);
        assert_eq!(f.calls[2].remaining, 2);
        assert_eq!(f.calls[0].row.source, f.calls[2].row.source);
    }
    #[test]
    fn failed_word_retains_completed_publications_and_effects_without_replay() {
        let mut f = Fixture::new("alpha beta");
        f.fail = true;
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.calls.len(), 1);
        assert_eq!(f.value(Field::Noun), 1);
        assert_eq!(f.results.len(), 2);
        assert!(f.made.is_none() && f.output.is_empty());
    }
    #[test]
    fn unterminated_spelling_source_refuses_at_nul_instead_of_scanning_tail() {
        let mut f = Fixture::new("xyz");
        f.returned = 0x1000;
        f.remove_delimiter = true;
        assert_eq!(translate(&mut f), Err(Error::Source));
        assert_eq!(f.calls.len(), 4);
        assert!(f.calls[1..]
            .iter()
            .all(|c| c.number && c.initialized == 150 && c.pause == 0));
    }
    #[test]
    fn cyclic_insert_primitive_has_bounded_work_and_preserves_effects() {
        let mut f = Fixture::new("a");
        f.insert_loop = true;
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.chars, SOURCE * 8);
        assert!(f.calls.is_empty() && f.made.is_none());
    }
    #[test]
    fn oversized_phoneme_count_refuses_before_terminal_entry_writes() {
        let mut f = Fixture::new("word");
        f.count_overflow = true;
        assert_eq!(translate(&mut f), Err(Error::Capacity));
        assert_eq!(f.calls.len(), 1);
        assert_eq!(f.entries[999].word_stress, 0x55);
        assert!(f.output.is_empty());
    }
    #[test]
    fn dictionary_skips_delete_following_rows_without_losing_word_counts() {
        let mut f = Fixture::new("one two three");
        f.returned |= 0x80;
        f.skip = 2;
        assert_eq!(translate(&mut f), Ok(()));
        assert_eq!(f.value(Field::Words), 3);
        assert!(f.calls[1..].iter().all(|c| c.row.flags & 0x100000 != 0));
        assert_eq!(f.value(Field::SkipDictionary), 0);
    }
    #[test]
    fn empty_clause_carries_sentence_without_decrementing_skip_and_preserves_entry_tails() {
        let mut f = Fixture::new(" ");
        f.term = 20;
        f.field(Field::MaxPause, 100);
        f.field(Field::SkipSentences, 2);
        assert_eq!(translate(&mut f), Ok(()));
        assert_eq!(f.value(Field::SkipSentences), 2);
        assert_eq!(f.value(Field::NewSentence), 1);
        assert_eq!(f.value(Field::MaxPause), 200);
        assert_eq!(f.made, Some((100, false)));
        assert_eq!(f.results[0], (0, 20));
        assert_eq!(f.entries[1].code, 9);
        assert_eq!(f.entries[1].word_stress, 0x55);
        assert_eq!(f.entries[1].tone, 0x56);
    }
    #[test]
    fn abbreviation_changes_tone_after_initial_publication() {
        let mut f = Fixture::new("mr");
        f.returned |= 0x1000000;
        assert_eq!(translate(&mut f), Ok(()));
        assert_eq!(f.results, vec![(0, f.term), (1, 0), (1, 4), (2, 0)]);
        assert_eq!(f.made, Some((10, true)));
    }
    #[test]
    fn bad_read_extent_refuses_before_outputs_but_keeps_initial_reset() {
        let mut f = Fixture::new("word");
        f.bad_top = true;
        f.field(Field::EmbeddedIndex, 12);
        assert_eq!(translate(&mut f), Err(Error::Source));
        assert_eq!(f.value(Field::EmbeddedIndex), 0);
        assert!(f.results.is_empty() && f.calls.is_empty());
    }
    #[test]
    fn voice_termination_refusal_preserves_prior_list_publication() {
        let mut f = Fixture::new("word");
        f.bad_voice = true;
        f.term = 0x20000;
        assert_eq!(translate(&mut f), Err(Error::Source));
        assert!(f.made.is_some());
        assert_eq!(f.output, vec![(1000, 0, 0), (1000, 1, 1)]);
        assert_eq!(f.results.len(), 2);
    }
    #[test]
    fn fresh_read_say_as_digit_effects_are_applied_during_tokenization() {
        let mut f = Fixture::new("12345");
        f.read_say_as = Some(0x42);
        assert_eq!(translate(&mut f), Ok(()));
        assert_eq!(f.words(), vec!["12", "34", "5"]);
    }
    #[test]
    fn infinite_foreign_space_or_continuation_sources_stop_at_physical_capacity() {
        for byte in [b' ', 0x80] {
            let mut f = Fixture::new("word");
            f.foreign_byte = Some(byte);
            assert_eq!(translate(&mut f), Err(Error::Source));
            assert_eq!(f.results.len(), 2);
            assert!(f.calls.is_empty());
        }
    }
    #[test]
    fn extreme_foreign_pause_command_and_skip_scalars_refuse_without_overflow() {
        let mut f = Fixture::new(" ");
        f.field(Field::MaxPause, i32::MIN);
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.results.len(), 2);
        f.field(Field::EmbeddedIndex, i32::MIN);
        assert_eq!(terminate_commands(&mut f), Err(Error::State));
        let mut f = Fixture::new("word");
        f.returned |= 0x1000000;
        f.skip = i32::MIN;
        assert_eq!(translate(&mut f), Err(Error::State));
        assert_eq!(f.calls.len(), 1);
    }
    #[test]
    fn dense_number_group_padding_reserves_the_complete_suffix_before_copy() {
        let mut f = Fixture::new("12345678901234567890123456789012");
        f.field(Field::MaxDigits, 64);
        f.field(Field::BreakNumbers, -1);
        assert_eq!(translate(&mut f), Err(Error::Capacity));
        assert!(f.calls.is_empty() && f.made.is_none());
        assert_eq!(f.results.len(), 2);
    }
}
