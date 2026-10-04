//! Bounded execution of the fork's compiled letter-to-phoneme templates.
// Copyright (C) 2005-2015 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{dictionary::InvalidDictionary, unicode, word_key};

const PRE: u8 = 1;
const POST: u8 = 2;
const PHONEMES: u8 = 3;
const GROUP_END: u8 = 7;
const NO_TRACE: u32 = 0x10000000;
const UNPRON_TEST: u32 = 0x80000000;

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct Context {
    pub conditions: u32,
    pub word_flags: u32,
    pub dictionary_flags: u32,
    pub vowel_count: i32,
    pub stressed_count: i32,
    pub expect_verb: i32,
    pub tone_numbers: i32,
    pub suffix_options: i32,
    pub trace: u32,
    /// Start of this word within text; zero selects the usual offset one.
    pub word_start: u32,
    /// Preserve the C adapter's plain-char signedness for the high ending byte.
    pub signed_bytes: u32,
}

/// Language configuration and prefix lookup are supplied by the engine owner.
/// Implementations must keep predicate work bounded; no I/O belongs here.
pub trait Environment {
    fn is_letter(&mut self, code: u32, group: u8) -> bool;
    fn letter_group(
        &mut self,
        text: &[u8],
        position: usize,
        group: u8,
        backwards: bool,
    ) -> Option<usize>;
    fn prefix_flags(&mut self, prefix: &[u8]) -> [u32; 2];
    fn trace(&mut self, _template: usize, _phonemes: Option<usize>, _points: i32) {}
}

/// All offsets borrow the supplied buffers; no allocation or pointer mutation.
#[derive(Default, Clone, Copy, Debug)]
pub struct Match {
    pub points: i32,
    pub phonemes: Option<usize>,
    pub ending: i32,
    pub delete: Option<usize>,
    pub advance: usize,
}

fn byte(text: &[u8], position: isize) -> u8 {
    usize::try_from(position)
        .ok()
        .and_then(|p| text.get(p))
        .copied()
        .unwrap_or(0)
}
fn is_digit(code: u32) -> bool {
    unicode::is_digit(code) || (0x966..=0x96f).contains(&code)
}

/// Matches a compiled `.Lxx` string list without reading before/after text.
pub fn letter_group(
    patterns: &[u8],
    text: &[u8],
    position: usize,
    backwards: bool,
) -> Result<Option<usize>, InvalidDictionary> {
    let mut cursor = 0;
    loop {
        let first = *patterns
            .get(cursor)
            .ok_or(InvalidDictionary("missing letter-group end"))?;
        if first == GROUP_END {
            return Ok(None);
        }
        if first == b'~' {
            return Ok(Some(0));
        }
        let end = terminator(patterns, cursor)?;
        let pattern = &patterns[cursor..end];
        let start = if backwards {
            if text.get(position).copied().unwrap_or(0) == 0 {
                cursor = end + 1;
                continue;
            }
            position
                .checked_add(1)
                .and_then(|p| p.checked_sub(pattern.len()))
        } else {
            Some(position)
        };
        if let Some(start) = start {
            if let Some(actual) = text.get(start..start.saturating_add(pattern.len())) {
                if !actual.contains(&0) && actual == pattern {
                    return Ok(Some(pattern.len()));
                }
            }
        }
        cursor = end + 1;
    }
}
fn decode(text: &[u8], position: isize, backwards: bool) -> (u32, isize) {
    let mut start = position;
    if backwards {
        while byte(text, start) & 0xc0 == 0x80 {
            start -= 1;
        }
    }
    if let Ok(start) = usize::try_from(start) {
        let (code, width) = word_key::decode(text, start);
        (code, width as isize)
    } else {
        (0, 1)
    }
}
fn take(rules: &[u8], cursor: &mut usize) -> Result<u8, InvalidDictionary> {
    let value = rules
        .get(*cursor)
        .copied()
        .ok_or(InvalidDictionary("truncated rule instruction"))?;
    *cursor += 1;
    Ok(value)
}
fn terminator(rules: &[u8], start: usize) -> Result<usize, InvalidDictionary> {
    rules
        .get(start..)
        .and_then(|r| r.iter().position(|b| *b == 0))
        .map(|n| start + n)
        .ok_or(InvalidDictionary("unterminated rule phonemes"))
}
fn group<E: Environment>(
    env: &mut E,
    text: &[u8],
    p: isize,
    number: u8,
    backwards: bool,
) -> Option<usize> {
    usize::try_from(p)
        .ok()
        .filter(|p| *p < text.len())
        .and_then(|p| {
            env.letter_group(text, p, number, backwards)
                .filter(|n| *n <= if backwards { p + 1 } else { text.len() - p })
        })
}
fn dollar<E: Environment>(
    env: &mut E,
    text: &[u8],
    position: usize,
    word_start: usize,
    consumed: usize,
    group_length: usize,
    command: u8,
) -> Option<i32> {
    // Prefix includes the current group and consumed letters, but no context.
    let length = position
        .checked_sub(word_start)?
        .checked_add(consumed)?
        .checked_add(group_length)?;
    if length + 3 > 160 {
        return None;
    }
    let mut prefix = [0_u8; 160];
    prefix[..length].copy_from_slice(text.get(word_start..word_start + length)?);
    prefix[length] = b' ';
    let flags = env.prefix_flags(&prefix[..length + 2]);
    let mask = 1_u32.checked_shl(14 + u32::from(command & 15)).unwrap_or(0);
    if (command == 3 && flags[0] & 0x80000000 != 0 && flags[1] & 0x4000 == 0)
        || flags[0] & mask != 0
    {
        Some(23)
    } else {
        None
    }
}

/// `text` includes accessible context before the input word and ends with NUL.
/// Context's `word_start`, `position` and deletion offsets use this same slice.
/// `rules` starts at an indexed group and must include its GROUP_END byte.
/// Missing context outside the text slice acts as a NUL boundary.
pub fn match_group<E: Environment>(
    rules: &[u8],
    text: &[u8],
    position: usize,
    group_length: usize,
    context: &Context,
    env: &mut E,
) -> Result<Match, InvalidDictionary> {
    if position == 0
        || position >= text.len()
        || text.last() != Some(&0)
        || group_length > 4
        || position
            .checked_add(group_length)
            .is_none_or(|end| end > text.len())
    {
        return Err(InvalidDictionary("invalid rule text window"));
    }
    let word_start = context.word_start.max(1) as usize;
    if word_start > position {
        return Err(InvalidDictionary("rule position precedes word start"));
    }
    let mut best = Match::default();
    let mut cursor = 0;
    let mut common = None;
    loop {
        if rules.get(cursor) == Some(&GROUP_END) {
            break;
        }
        let template = cursor;
        let mut current = Match {
            points: 1,
            ..Match::default()
        };
        let mut consumed = 0;
        let mut pre = position as isize;
        let mut post = (position + group_length) as isize;
        let mut left = -2;
        let mut right = -6;
        let mut mode = 0;
        let mut letter_w = 0;
        let mut at_start = false;
        let mut ignore = context.word_flags & UNPRON_TEST != 0;
        let mut success = false;
        loop {
            let instruction = take(rules, &mut cursor)?;
            let mut points = 0;
            let mut failed = false;
            if instruction <= 9 {
                match instruction {
                    0 => {
                        current.phonemes = common;
                        cursor -= 1;
                        success = true;
                        break;
                    }
                    8 => {
                        at_start = true;
                        ignore = false;
                        mode = PRE;
                    }
                    PRE => {
                        mode = PRE;
                        if context.word_flags & UNPRON_TEST != 0 {
                            break;
                        }
                    }
                    POST => mode = POST,
                    PHONEMES => {
                        current.phonemes = Some(cursor);
                        success = true;
                        break;
                    }
                    4 => {
                        // Legacy common records retain their whole template.
                        let mut p = cursor;
                        loop {
                            let opcode = take(rules, &mut p)?;
                            if opcode == 0 || opcode == PHONEMES {
                                break;
                            }
                            if opcode == 5 {
                                take(rules, &mut p)?;
                            }
                            if opcode == 9 {
                                take(rules, &mut p)?;
                                take(rules, &mut p)?;
                            }
                        }
                        common = Some(p);
                    }
                    5 => {
                        let condition = take(rules, &mut cursor)?;
                        let mask = 1_u32
                            .checked_shl(u32::from(condition & 31))
                            .filter(|_| condition < 64)
                            .ok_or(InvalidDictionary("invalid rule condition"))?;
                        if (context.conditions & mask != 0) == (condition >= 32) {
                            break;
                        }
                        current.points += 1;
                    }
                    9 => {
                        take(rules, &mut cursor)?;
                        take(rules, &mut cursor)?;
                    }
                    _ => {}
                }
                continue;
            }
            if mode == 0 {
                let letter = byte(text, post);
                post += 1;
                if letter == instruction || (letter == b'E' && instruction == b'e') {
                    if letter & 0xc0 != 0x80 {
                        points = 21;
                    }
                    consumed += 1;
                } else {
                    failed = true;
                }
            } else if mode == POST {
                right = if right + 6 > 18 { 19 } else { right + 6 };
                let last = letter_w;
                if byte(text, post - 1) == 0 {
                    break;
                }
                let (code, width) = decode(text, post, false);
                letter_w = code;
                let extra = width - 1;
                let letter = byte(text, post);
                post += 1;
                match instruction {
                    17 => {
                        let number = take(rules, &mut cursor)?.wrapping_sub(b'A');
                        if env.is_letter(code, number) {
                            points = (if number == 2 { 19 } else { 20 }) - right;
                            post += extra;
                        } else {
                            failed = true;
                        }
                    }
                    18 => {
                        let number = take(rules, &mut cursor)?.wrapping_sub(b'A');
                        if let Some(n) = group(env, text, post - 1, number, false) {
                            points = 20 - right;
                            post += n as isize - 1;
                        } else {
                            failed = true;
                        }
                    }
                    25 => {
                        if env.is_letter(code, 0)
                            || (code == 32 && context.word_flags & 0x08000000 != 0)
                        {
                            failed = true;
                        } else {
                            points = 20 - right;
                            post += extra;
                        }
                    }
                    15 => {
                        if is_digit(code) {
                            points = 20 - right;
                            post += extra;
                        } else if context.tone_numbers != 0 {
                            points = 20 - right;
                            post -= 1;
                        } else {
                            failed = true;
                        }
                    }
                    16 | 11 => {
                        let matched = if instruction == 16 {
                            !unicode::is_alpha(code)
                        } else {
                            code == last
                        };
                        if matched {
                            points = 21 - right;
                            post += extra;
                        } else {
                            failed = true;
                        }
                    }
                    28 => {
                        post -= 1;
                        let command = take(rules, &mut cursor)?;
                        match command {
                            1 => current.ending = 0x8000,
                            2 => {
                                if context.word_flags & 0x800000 != 0 {
                                    failed = true;
                                } else {
                                    points = 1;
                                }
                            }
                            c if c & 0xf0 == 0x10 => {
                                if context.dictionary_flags & (1_u32 << (14 + (c & 15))) != 0 {
                                    points = 23;
                                } else {
                                    failed = true;
                                }
                            }
                            c if c & 0xf0 == 0x20 || c == 3 => {
                                if let Some(n) = dollar(
                                    env,
                                    text,
                                    position,
                                    word_start,
                                    consumed,
                                    group_length,
                                    c,
                                ) {
                                    points = n;
                                } else {
                                    failed = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    b'-' => {
                        if letter == b'-' || (letter == b' ' && context.word_flags & 0x4000 != 0) {
                            points = 22 - right;
                        } else {
                            failed = true;
                        }
                    }
                    21 | 29 => {
                        let mut syllables = 1;
                        if instruction == 21 {
                            while rules.get(cursor) == Some(&21) {
                                cursor += 1;
                                syllables += 1;
                            }
                        }
                        let mut p = post + extra;
                        let mut count = 0;
                        let mut vowel = false;
                        while letter_w != 32 && letter_w != 0 {
                            let next_vowel = env.is_letter(letter_w, 7);
                            if instruction == 29 && next_vowel {
                                failed = true;
                                break;
                            }
                            if !vowel && next_vowel {
                                count += 1;
                            }
                            vowel = next_vowel;
                            let (next, width) = decode(text, p, false);
                            letter_w = next;
                            p += width;
                        }
                        if instruction == 21 {
                            if syllables <= count {
                                points = 18 + syllables - right;
                            } else {
                                failed = true;
                            }
                        } else if !failed {
                            points = 19 - right;
                        }
                    }
                    23 => {
                        let mut p = post - 1;
                        let mut previous = p;
                        let target = word_key::decode(rules, cursor).0;
                        let mut matched_group = None;
                        while letter_w != target
                            && letter_w != 32
                            && letter_w != 0
                            && matched_group.is_none()
                        {
                            if target == 18 {
                                let number = *rules
                                    .get(cursor + 1)
                                    .ok_or(InvalidDictionary("truncated skip group"))?;
                                matched_group =
                                    group(env, text, p, number.wrapping_sub(b'A'), false);
                            }
                            previous = p;
                            let (next, width) = decode(text, p, false);
                            letter_w = next;
                            p += width;
                        }
                        if letter_w == target || matched_group.is_some() {
                            post = previous;
                        }
                    }
                    12 | 60 => {
                        post -= 1;
                        points = if instruction == 12 { 20 } else { -20 };
                    }
                    13 => {
                        current.delete = ((position + group_length) as isize..post)
                            .find(|p| byte(text, *p) == b'e')
                            .map(|p| p as usize);
                    }
                    14 => {
                        let a = take(rules, &mut cursor)?;
                        let b = take(rules, &mut cursor)?;
                        let c = take(rules, &mut cursor)?;
                        let ending = ((if context.signed_bytes != 0 {
                            i32::from(a as i8)
                        } else {
                            i32::from(a)
                        }) << 16)
                            + (i32::from(b & 127) << 8)
                            + i32::from(c & 127);
                        if context.vowel_count == 0
                            && ending & 0x400 == 0
                            && context.suffix_options & 1 != 0
                        {
                            failed = true;
                        } else {
                            current.ending = ending;
                        }
                    }
                    24 => {
                        if context.word_flags & 0x2000 != 0 {
                            failed = true;
                        } else {
                            post -= 1;
                            points = 1;
                        }
                    }
                    _ => {
                        if letter == instruction {
                            if letter & 0xc0 != 0x80 {
                                points = 21 - right;
                            }
                        } else {
                            failed = true;
                        }
                    }
                }
            } else {
                left = if left + 2 > 18 { 19 } else { left + 2 };
                if byte(text, pre) == 0 {
                    break;
                }
                let last = decode(text, pre, false).0;
                pre -= 1;
                let (code, width) = decode(text, pre, true);
                letter_w = code;
                let extra = width - 1;
                let letter = byte(text, pre);
                match instruction {
                    17 => {
                        let number = take(rules, &mut cursor)?.wrapping_sub(b'A');
                        if env.is_letter(code, number) {
                            points = (if number == 2 { 19 } else { 20 }) - left;
                            pre -= extra;
                        } else {
                            failed = true;
                        }
                    }
                    18 => {
                        let number = take(rules, &mut cursor)?.wrapping_sub(b'A');
                        if let Some(n) = group(env, text, pre, number, true) {
                            points = 20 - right;
                            pre -= n as isize - 1;
                        } else {
                            failed = true;
                        }
                    }
                    25 => {
                        if !env.is_letter(code, 0) {
                            points = 20 - left;
                            pre -= extra;
                        } else {
                            failed = true;
                        }
                    }
                    11 | 15 | 16 => {
                        let matched = match instruction {
                            11 => code == last,
                            15 => is_digit(code),
                            _ => !unicode::is_alpha(code),
                        };
                        if matched {
                            points = 21 - if instruction == 16 { right } else { left };
                            pre -= extra;
                        } else {
                            failed = true;
                        }
                    }
                    28 => {
                        pre += 1;
                        let command = take(rules, &mut cursor)?;
                        if command == 3 || command & 0xf0 == 0x20 {
                            if let Some(n) = dollar(
                                env,
                                text,
                                position,
                                word_start,
                                consumed,
                                group_length,
                                command,
                            ) {
                                points = n;
                            } else {
                                failed = true;
                            }
                        }
                    }
                    21 => {
                        let mut syllables = 1;
                        while rules.get(cursor) == Some(&21) {
                            cursor += 1;
                            syllables += 1;
                        }
                        if syllables <= context.vowel_count {
                            points = 18 + syllables - left;
                        } else {
                            failed = true;
                        }
                    }
                    10 | 26 | 19 => {
                        pre += 1;
                        let matched = match instruction {
                            10 => context.stressed_count > 0,
                            26 => context.expect_verb != 0,
                            _ => context.word_flags & 2 != 0,
                        };
                        if matched {
                            points = if instruction == 10 { 19 } else { 1 };
                        } else {
                            failed = true;
                        }
                    }
                    29 => {
                        let mut p = pre - extra;
                        while letter_w != 32 && letter_w != 0 {
                            if env.is_letter(letter_w, 7) {
                                failed = true;
                                break;
                            }
                            let (next, width) = decode(text, p - 1, true);
                            letter_w = next;
                            p -= width;
                        }
                        // The legacy PRE scan treats a wide-group NUL as a
                        // vowel; without one it can run before storage. Reject
                        // either NUL boundary instead of accepting that scan.
                        if letter_w == 0 {
                            failed = true;
                        }
                        if !failed {
                            points = 3;
                        }
                    }
                    b'.' => {
                        let mut p = pre;
                        while byte(text, p) != 0 && byte(text, p) != b' ' && byte(text, p) != b'.' {
                            p -= 1;
                        }
                        if byte(text, p) == b'.' {
                            points = 50;
                        } else {
                            failed = true;
                        }
                    }
                    b'-' => {
                        if letter == b'-' || (letter == b' ' && context.word_flags & 0x80 != 0) {
                            points = 22 - right;
                        } else {
                            failed = true;
                        }
                    }
                    23 => {
                        let mut p = pre + 1;
                        let mut previous = p;
                        let target = *rules
                            .get(cursor)
                            .ok_or(InvalidDictionary("truncated backwards skip"))?;
                        let mut matched_group = None;
                        while byte(text, p) != target
                            && byte(text, p) != 32
                            && byte(text, p) != 0
                            && matched_group.is_none()
                        {
                            previous = p;
                            p -= 1;
                            if target == 18 {
                                let number = *rules
                                    .get(cursor + 1)
                                    .ok_or(InvalidDictionary("truncated backwards group"))?;
                                matched_group =
                                    group(env, text, previous, number.wrapping_sub(b'A'), true);
                            }
                        }
                        if byte(text, p) == target {
                            pre = previous;
                        }
                        if matched_group.is_some() {
                            pre = previous + 1;
                        }
                    }
                    _ => {
                        if letter == instruction {
                            if letter == 32 {
                                points = 4;
                            } else if letter & 0xc0 != 0x80 {
                                points = 21 - left;
                            }
                        } else {
                            failed = true;
                        }
                    }
                }
            }
            if failed {
                break;
            }
            current.points += points;
        }
        if success && !ignore && (!at_start || byte(text, pre - 1) == b' ') {
            if at_start {
                current.points += 4;
            }
            if let Some(p) = current.phonemes {
                terminator(rules, p)?;
            }
            if current.points >= best.points {
                current.advance = consumed;
                best = current;
            }
            if context.trace != 0 && current.points > 0 && context.word_flags & NO_TRACE == 0 {
                env.trace(
                    template,
                    current.phonemes,
                    current.points + if group_length > 1 { 35 } else { 0 },
                );
            }
        }
        cursor = terminator(rules, cursor)? + 1;
    }
    best.advance = (best.advance + group_length).max(1);
    if best.points == 0 {
        best.phonemes = None;
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Language {
        prefix: Vec<u8>,
    }
    impl Environment for Language {
        fn is_letter(&mut self, code: u32, group: u8) -> bool {
            let vowel = "aeiouö".chars().any(|c| c as u32 == code);
            match group {
                0 | 7 => vowel,
                2 => unicode::is_alpha(code) && !vowel,
                _ => false,
            }
        }
        fn letter_group(
            &mut self,
            text: &[u8],
            position: usize,
            _group: u8,
            backwards: bool,
        ) -> Option<usize> {
            letter_group(b"ch\0\x07", text, position, backwards).unwrap()
        }
        fn prefix_flags(&mut self, prefix: &[u8]) -> [u32; 2] {
            self.prefix = prefix.to_vec();
            [0x80000000, 0]
        }
    }
    #[test]
    fn later_ties_reuse_common_phonemes_and_preserve_start_bonus() {
        let mut language = Language::default();
        let result = match_group(
            &[4, 3, 42, 0, 3, 43, 0, 0, 7],
            b" a\0",
            1,
            1,
            &Context::default(),
            &mut language,
        )
        .unwrap();
        assert_eq!(
            (result.points, result.phonemes, result.advance),
            (1, Some(2), 1)
        );
        let result = match_group(
            &[8, 3, 42, 0, 3, 43, 0, 7],
            b" a\0",
            1,
            1,
            &Context {
                word_flags: UNPRON_TEST,
                ..Context::default()
            },
            &mut language,
        )
        .unwrap();
        assert_eq!((result.points, result.phonemes), (5, Some(2)));
    }
    #[test]
    fn deletion_endings_and_devanagari_digits_keep_legacy_semantics() {
        let mut language = Language::default();
        let rules = [2, b'e', 13, 14, 0x80, 0x81, 0x82, 3, 42, 0, 7];
        let signed = match_group(
            &rules,
            b" ae \0",
            1,
            1,
            &Context {
                signed_bytes: 1,
                ..Context::default()
            },
            &mut language,
        )
        .unwrap();
        let unsigned =
            match_group(&rules, b" ae \0", 1, 1, &Context::default(), &mut language).unwrap();
        assert_eq!(signed.delete, Some(2));
        assert_eq!(signed.ending, 0xff800102_u32 as i32);
        assert_eq!(unsigned.ending, 0x800102);
        let result = match_group(
            &[2, 15, 3, 42, 0, 3, 43, 0, 7],
            " a१\0".as_bytes(),
            1,
            1,
            &Context::default(),
            &mut language,
        )
        .unwrap();
        assert_eq!((result.points, result.phonemes), (21, Some(3)));
    }
    #[test]
    fn prefix_lookup_excludes_borrowed_left_context_and_consumes_only_the_word() {
        let mut language = Language::default();
        let result = match_group(
            &[b'a', b't', 2, 28, 3, 3, 42, 0, 7],
            b" dog cat \0",
            5,
            1,
            &Context {
                word_start: 5,
                ..Context::default()
            },
            &mut language,
        )
        .unwrap();
        assert_eq!(language.prefix, b"cat \0");
        assert_eq!((result.points, result.advance), (66, 3));
        let result = match_group(
            &[1, b' ', b'g', 3, 42, 0, 7],
            b" dog cat \0",
            5,
            1,
            &Context {
                word_start: 5,
                ..Context::default()
            },
            &mut language,
        )
        .unwrap();
        assert_eq!((result.points, result.phonemes), (24, Some(4)));
    }
    #[test]
    fn string_groups_bound_backward_reads_and_allow_empty_groups() {
        let patterns = "ö\0ch\0\u{7}".as_bytes();
        assert_eq!(
            letter_group(patterns, " ö\0".as_bytes(), 2, true).unwrap(),
            Some(2)
        );
        assert_eq!(letter_group(patterns, b"c\0", 0, true).unwrap(), None);
        assert_eq!(letter_group(b"~\0\x07", b"\0", 0, true).unwrap(), Some(0));
        assert!(letter_group(b"ch", b"ch\0", 0, false).is_err());
    }
    #[test]
    fn malformed_instructions_and_text_windows_return_errors() {
        let mut language = Language::default();
        for rules in [
            &[][..],
            &[5, 64, 3, 42, 0, 7],
            &[9, 1],
            &[2, 14, 1],
            &[3, 42],
            &[4, 2],
            &[0],
        ] {
            assert!(match_group(rules, b" a\0", 1, 1, &Context::default(), &mut language).is_err());
        }
        assert!(match_group(
            &[7],
            b" a\0",
            usize::MAX,
            1,
            &Context::default(),
            &mut language
        )
        .is_err());
        assert!(match_group(&[7], b" a", 1, 1, &Context::default(), &mut language).is_err());
        let result = match_group(
            &[1, 29, 3, 42, 0, 3, 43, 0, 7],
            " १a२ \0".as_bytes(),
            3,
            1,
            &Context::default(),
            &mut language,
        )
        .unwrap();
        assert_eq!((result.points, result.phonemes), (1, Some(6)));
    }
}
