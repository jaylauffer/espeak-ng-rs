//! Owned main clause parser with separate source and backend effects.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{clause_input as input, suffix, utf8};

pub const NONE: i32 = 0x4000;
pub const EOF: i32 = 0x90028;
const PERIOD: i32 = 0x80028;
const PARAGRAPH: i32 = 0x80046;
const VOICE: i32 = 0x24000;
const COLON: i32 = 0x4001e;
const QUESTION: i32 = 0x82028;
const EXCLAMATION: i32 = 0x8302d;
const DOT_AFTER: i32 = 0x400000;
const EMPHASIS: i32 = 0x530;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Arithmetic,
    Backend,
    State,
    Uninitialized,
    Progress,
}

/// Only initialized scalar fields and terminated string prefixes participate
/// in a compatibility snapshot. Replay belongs to the instance and survives
/// clause boundaries (including the legacy text-reset operation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct State {
    pub cursor: input::Cursor,
    pub pending_second: i32,
    pub replay_index: i32,
    pub replay: [u8; 24],
    pub ignore: i32,
    pub audio: i32,
    pub clear_skipping: i32,
    pub skipping: i32,
    pub ssml: i32,
    pub phoneme_input: i32,
    pub line_length: i32,
    pub capitals: i32,
    pub punctuation: i32,
    pub punctuation_list: [u32; 60],
    pub sayas_mode: i32,
    pub sayas_start: i32,
    pub parameters: [i32; 15],
    pub skip_characters: i32,
    pub end_position: i32,
    pub clause_start: i32,
    pub repeat_count: i32,
    pub upper_count: i32,
    pub lower_count: i32,
    pub language: i32,
    pub numbers: i32,
    pub lowercase_sentence: i32,
    pub tone: i32,
    pub index_top: i32,
    pub current_voice: [u8; 40],
    pub voice_change: [u8; 40],
    pub base_identifier: [u8; 40],
    pub has_base_identifier: i32,
    pub signed_bytes: i32,
    pub wide16: i32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            cursor: input::Cursor {
                pending: 0,
                count: 0,
            },
            pending_second: 0,
            replay_index: -1,
            replay: [0; 24],
            ignore: 0,
            audio: 0,
            clear_skipping: 0,
            skipping: 0,
            ssml: 0,
            phoneme_input: 0,
            line_length: 0,
            capitals: 0,
            punctuation: 0,
            punctuation_list: [0; 60],
            sayas_mode: 0,
            sayas_start: 0,
            parameters: [0; 15],
            skip_characters: 0,
            end_position: 0,
            clause_start: 0,
            repeat_count: 0,
            upper_count: 0,
            lower_count: 0,
            language: 0,
            numbers: 0,
            lowercase_sentence: 0,
            tone: 0,
            index_top: 0,
            current_voice: [0; 40],
            voice_change: [0; 40],
            base_identifier: [0; 40],
            has_base_identifier: 0,
            signed_bytes: 1,
            wide16: 0,
        }
    }
}
impl State {
    pub fn valid(&self) -> bool {
        (-1..24).contains(&self.replay_index)
            && (self.replay_index < 0 || self.replay[self.replay_index as usize..].contains(&0))
            && self.punctuation_list.contains(&0)
            && self.current_voice.contains(&0)
            && self.voice_change.contains(&0)
            && self.base_identifier.contains(&0)
            && matches!(self.signed_bytes, 0 | 1)
            && matches!(self.wide16, 0 | 1)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Class {
    Space,
    Alnum,
    Alpha,
    Upper,
    Lower,
    Digit,
    Punctuation,
    WordAlpha,
    Bracket,
    ByteSpace,
}
/// Copied effect commands: tag1, punctuation2, character-name3, special-capital4.
/// Backend text is a terminated initialized scratch buffer. Tag units are
/// copied and narrowed according to the host's wchar_t width before dispatch.
#[repr(C)]
pub struct Command {
    pub kind: u32,
    pub code: i32,
    pub next: i32,
    pub end: i32,
    pub index: i32,
    pub clause: i32,
    pub found: i32,
    pub xml: [u32; 501],
    pub text: [u8; 74],
}
impl Command {
    fn new(kind: u32, index: usize) -> Result<Self, Error> {
        Ok(Self {
            kind,
            code: 0,
            next: 0,
            end: 0,
            index: i32::try_from(index).map_err(|_| Error::Capacity)?,
            clause: 0,
            found: 0,
            xml: [0; 501],
            text: [0; 74],
        })
    }
}
/// Sparse output writes must reject gaps and bounds before touching storage.
/// `initialized` includes all bytes written, even after a shorter tag result.
/// Index storage is independently writable; its unused tail is never read.
pub trait Output {
    fn capacity(&self) -> usize;
    fn initialized(&self) -> &[u8];
    fn write(&mut self, position: usize, bytes: &[u8]) -> Result<(), Error>;
    fn character_index(&mut self, position: usize, value: i16) -> Result<(), Error>;
}
pub struct Buffer<'a> {
    bytes: &'a mut [u8],
    indexes: &'a mut [i16],
    initialized: usize,
}
impl<'a> Buffer<'a> {
    pub fn new(bytes: &'a mut [u8], indexes: &'a mut [i16]) -> Self {
        Self {
            bytes,
            indexes,
            initialized: 0,
        }
    }
}
impl Output for Buffer<'_> {
    fn capacity(&self) -> usize {
        self.bytes.len()
    }
    fn initialized(&self) -> &[u8] {
        &self.bytes[..self.initialized]
    }
    fn write(&mut self, position: usize, bytes: &[u8]) -> Result<(), Error> {
        if position > self.initialized {
            return Err(Error::Uninitialized);
        }
        let end = position.checked_add(bytes.len()).ok_or(Error::Capacity)?;
        self.bytes
            .get_mut(position..end)
            .ok_or(Error::Capacity)?
            .copy_from_slice(bytes);
        self.initialized = self.initialized.max(end);
        Ok(())
    }
    fn character_index(&mut self, position: usize, value: i16) -> Result<(), Error> {
        *self.indexes.get_mut(position).ok_or(Error::Capacity)? = value;
        Ok(())
    }
}
pub trait Host {
    fn source_eof(&self) -> bool;
    fn source_read(&mut self) -> u32;
    fn source_peek(&mut self) -> u32;
    /// Pure, stable host/locale classification. Wide classification may narrow
    /// its input on platforms whose wint_t is16 bits; word predicates do not.
    fn classify(&self, code: i32, class: Class) -> bool;
    fn replace(&self, code: i32) -> Result<input::Replacement, Error>;
    /// Owns separate backend resources, publishes copied state before callbacks
    /// and refreshes afterwards. Output effects write only admitted storage.
    fn effect(
        &mut self,
        state: &mut State,
        output: &mut dyn Output,
        command: &mut Command,
    ) -> Result<(), Error>;
}
fn eof(state: &State, host: &impl Host) -> bool {
    state.cursor.eof(host.source_eof())
}
fn read(state: &mut State, host: &mut impl Host) -> Result<i32, Error> {
    state
        .cursor
        .read(|| host.source_read())
        .map_err(|_| Error::Arithmetic)
}
fn add(value: i32, amount: i32) -> Result<i32, Error> {
    value.checked_add(amount).ok_or(Error::Arithmetic)
}
fn encode(output: &mut dyn Output, index: &mut usize, code: i32) -> Result<(), Error> {
    let (bytes, length) = suffix::encode(code as u32);
    output.write(*index, &bytes[..length])?;
    *index += length;
    Ok(())
}
fn terminate(
    state: &mut State,
    output: &mut dyn Output,
    index: usize,
    pending: Option<i32>,
) -> Result<(), Error> {
    output.write(index, b" \0")?;
    if let Some(code) = pending {
        state.cursor.unread(code);
    }
    Ok(())
}
fn remove(output: &mut dyn Output, index: usize) -> Result<(), Error> {
    let character =
        utf8::decode(output.initialized(), index, false).map_err(|_| Error::Uninitialized)?;
    output.write(index, &b"    "[..character.width])
}
fn effect(
    state: &mut State,
    output: &mut dyn Output,
    host: &mut impl Host,
    command: &mut Command,
) -> Result<(), Error> {
    host.effect(state, output, command)?;
    if !state.valid() {
        return Err(Error::State);
    }
    Ok(())
}
fn position(state: &State) -> Result<i16, Error> {
    Ok(state
        .cursor
        .count
        .checked_sub(state.clause_start)
        .ok_or(Error::Arithmetic)? as i16)
}
fn replay_byte(state: &mut State) -> Result<i32, Error> {
    let index = usize::try_from(state.replay_index).map_err(|_| Error::State)?;
    let byte = *state.replay.get(index).ok_or(Error::State)?;
    state.replay_index += 1;
    Ok(if state.signed_bytes != 0 {
        i32::from(byte as i8)
    } else {
        i32::from(byte)
    })
}
fn text_length(text: &[u8]) -> Result<usize, Error> {
    text.iter().position(|c| *c == 0).ok_or(Error::Backend)
}

/// Reads one clause. Earlier source/backend/output effects remain visible if
/// a later bounds/count check fails. No whole-operation rollback is promised.
/// The parser neither schedules work nor performs I/O outside its supplied host.
pub fn read_clause(
    state: &mut State,
    output: &mut dyn Output,
    host: &mut impl Host,
) -> Result<i32, Error> {
    if !state.valid() || output.capacity() > i32::MAX as usize {
        return Err(Error::State);
    }
    let capacity = output.capacity() as i32;
    if state.clear_skipping != 0 {
        state.skipping = 0;
        state.clear_skipping = 0;
    }
    state.repeat_count = 0;
    state.upper_count = 0;
    state.lower_count = 0;
    state.tone = 0;
    state.voice_change[0] = 0;
    let mut current = 32;
    let mut following = if state.pending_second != 0 {
        state.pending_second
    } else if eof(state, host) {
        0
    } else {
        read(state, host)?
    };
    let mut previous = 32;
    let mut index = 0usize;
    let mut line_length = 0i32;
    let mut mode = 0;
    let mut any_alnum = false;
    let mut stressed = false;
    let mut delayed_clause = 0;
    let mut delayed_index = 0usize;
    let mut next = 0;
    let mut last_count = state.cursor.count;
    let mut replay_steps = 0usize;
    while !eof(state, host)
        || state.cursor.pending != 0
        || state.pending_second != 0
        || state.replay_index >= 0
    {
        // Internal replay contains at most24 bytes plus two character slots.
        // Replaying those sources across several boundaries stays below this
        // budget. Repeated backend deferral without new source admission can
        // otherwise loop forever after EOF (the retained C reader does so).
        if state.cursor.count == last_count {
            replay_steps += 1;
            if replay_steps > state.replay.len() * 4 {
                return Err(Error::Progress);
            }
        } else {
            last_count = state.cursor.count;
            replay_steps = 0;
        }
        if !host.classify(current, Class::Alnum) {
            if state.end_position > 0 && state.cursor.count > state.end_position {
                return Ok(EOF);
            }
            if state.skip_characters > 0 && state.cursor.count >= state.skip_characters {
                state.clear_skipping = 1;
                state.skip_characters = 0;
                state.cursor.unread(following);
                return Ok(NONE);
            }
        }
        let previous2 = previous;
        previous = current;
        current = following;
        if state.replay_index >= 0 && state.replay.get(state.replay_index as usize) == Some(&0) {
            state.replay_index = -1;
        }
        if state.replay_index == 0 && state.pending_second == 0 {
            current = replay_byte(state)?;
        }
        following = if state.replay_index >= 0 {
            replay_byte(state)?
        } else if eof(state, host) {
            32
        } else {
            read(state, host)?
        };
        state.pending_second = 0;
        if state.ssml != 0 && mode == 0 {
            if current == 38 && (following == 35 || (97..=122).contains(&following)) {
                let mut entity = [0u8; 22];
                let mut length = 0;
                current = following;
                while !eof(state, host)
                    && (host.classify(current, Class::Alnum) || current == 35)
                    && length < 20
                {
                    entity[length] = current as u8;
                    length += 1;
                    current = read(state, host)?;
                }
                following = if eof(state, host) {
                    0
                } else {
                    read(state, host)?
                };
                // sprintf's %s ends at the first narrowed NUL, independently of
                // the entity's logical codepoint count; %c inserts low bytes.
                let prefix = text_length(&entity)?;
                state.replay[..prefix].copy_from_slice(&entity[..prefix]);
                state.replay[prefix] = current as u8;
                state.replay[prefix + 1] = following as u8;
                state.replay[prefix + 2] = 0;
                let found = if current == 59 {
                    let parsed =
                        crate::ssml::reference(&entity[..prefix], current, following, |code| {
                            host.classify(code as i32, Class::ByteSpace)
                        })
                        .map_err(|_| Error::Backend)?;
                    current = parsed.first;
                    following = parsed.second;
                    parsed.status
                } else {
                    -1
                };
                if found <= 0 {
                    state.replay_index = 0;
                    current = 38;
                    following = 32;
                }
                if current <= 32 && matches!(state.sayas_mode, 0x14 | 0x24) {
                    current = add(current, 0xe000)?;
                }
            } else if current == 60
                && (following == 47
                    || host.classify(following, Class::Alpha)
                    || matches!(following, 33 | 63))
            {
                if index as i32 > capacity - 20 {
                    state.pending_second = current;
                    terminate(state, output, index, Some(following))?;
                    return Ok(NONE);
                }
                let mut command = Command::new(1, index)?;
                let mut length = 0;
                current = following;
                while !eof(state, host) && current != 62 && length < 500 {
                    command.xml[length] = if state.wide16 != 0 {
                        current as u16 as u32
                    } else {
                        current as u32
                    };
                    length += 1;
                    current = read(state, host)?;
                }
                following = 32;
                if state.has_base_identifier != 0 {
                    state.current_voice = state.base_identifier;
                }
                effect(state, output, host, &mut command)?;
                index = usize::try_from(command.index).map_err(|_| Error::Backend)?;
                if index > output.initialized().len() {
                    return Err(Error::Uninitialized);
                }
                if command.clause != 0 {
                    terminate(state, output, index, None)?;
                    if command.clause & 0x20000 != 0 {
                        state.voice_change = state.current_voice;
                    }
                    return Ok(command.clause);
                }
                current = 32;
                if !eof(state, host) {
                    following = read(state, host)?;
                }
                continue;
            }
        }
        if state.ignore != 0 {
            continue;
        }
        if following == 10 && state.line_length == -1 {
            let mut clause = input::clause_type(current as u32);
            if clause == NONE {
                output.character_index(index, position(state)?)?;
                state.index_top = index as i32;
                encode(output, &mut index, current)?;
                clause = PERIOD;
            }
            terminate(state, output, index, None)?;
            return Ok(clause);
        }
        if current == 1 {
            if following == 86 {
                output.write(index, b"\0")?;
                index += 1;
                while !eof(state, host) {
                    current = read(state, host)?;
                    if host.classify(current, Class::Space) || index as i32 >= capacity - 1 {
                        break;
                    }
                    output.write(index, &[current as u8])?;
                    index += 1;
                }
                output.write(index, b"\0")?;
                return Ok(VOICE);
            } else if following == 66 {
                output.write(index, b"   \0")?;
                index += 3;
                let can_read = !eof(state, host);
                if can_read {
                    following = read(state, host)?;
                }
                if can_read && following == 48 {
                    // The C condition tests EOF before read, not afterwards.
                    state.punctuation = 0;
                } else {
                    state.punctuation = 1;
                    state.punctuation_list[0] = 0;
                    if following != 49 {
                        let mut count = 0;
                        while !eof(state, host)
                            && !host.classify(following, Class::Space)
                            && count < 59
                        {
                            state.punctuation_list[count] = if state.wide16 != 0 {
                                following as u16 as u32
                            } else {
                                following as u32
                            };
                            count += 1;
                            following = read(state, host)?;
                            output.write(index, b" ")?;
                            index += 1;
                        }
                        state.punctuation_list[count] = 0;
                        state.punctuation = 2;
                    }
                }
                if !eof(state, host) {
                    following = read(state, host)?;
                }
                continue;
            }
        }
        line_length = add(line_length, 1)?;
        let replacement = host.replace(current)?;
        if replacement.ignore {
            continue;
        }
        current = replacement.code;
        if host.classify(current, Class::Alnum) {
            any_alnum = true;
        } else {
            if stressed {
                stressed = false;
                current = EMPHASIS;
                state.cursor.unread(following);
                following = 32;
            }
            if current == 0xf0b {
                current = 32;
            }
            if current == 0xd4d && following == 0x200d {
                current = 0xd4e;
            }
            if current == 0xdca && following == 0x200d && !eof(state, host) {
                following = read(state, host)?;
            }
        }
        if host.classify(current, Class::Upper) {
            state.upper_count = add(state.upper_count, 1)?;
            if state.capitals == 2
                && state.sayas_mode == 0
                && !host.classify(previous, Class::Upper)
            {
                let mut command = Command::new(4, index)?;
                effect(state, output, host, &mut command)?;
                if command.found != 0 {
                    let length = text_length(&command.text[..30])?;
                    if index.checked_add(length).ok_or(Error::Capacity)? < output.capacity() {
                        output.write(index, &command.text[..length + 1])?;
                        index += length;
                    }
                }
            }
        } else if host.classify(current, Class::Alpha) {
            state.lower_count = add(state.lower_count, 1)?;
        }
        mode = input::phoneme_mode(state.phoneme_input, mode, current, following);
        if current == 10 {
            let mut paragraphs = 0i32;
            while !eof(state, host) && host.classify(following, Class::Space) {
                if following == 10 {
                    paragraphs = add(paragraphs, 1)?;
                }
                following = read(state, host)?;
            }
            if paragraphs > 0 {
                if delayed_clause != 0 {
                    remove(output, delayed_index)?;
                }
                terminate(state, output, index, Some(following))?;
                paragraphs = paragraphs.min(3);
                if state.ssml != 0 {
                    paragraphs = 1;
                }
                return Ok(PARAGRAPH - 30 + 30 * paragraphs);
            }
            if line_length <= state.line_length {
                terminate(state, output, index, Some(following))?;
                return Ok(COLON);
            }
            line_length = 0;
        }
        let mut announced = 0;
        if mode == 0 && state.sayas_mode == 0 {
            let mut end_clause = false;
            if delayed_clause != 0 && !host.classify(current, Class::Space) {
                if !host.classify(current, Class::WordAlpha)
                    || !host.classify(current, Class::Lower)
                {
                    state.pending_second = current;
                    terminate(state, output, delayed_index, Some(following))?;
                    return Ok(delayed_clause);
                }
                delayed_clause = 0;
            }
            if current == 46 && following == 46 {
                while !eof(state, host) {
                    next = read(state, host)?;
                    if next != 46 {
                        break;
                    }
                    current = 0x2026;
                    following = 32;
                }
                if current == 0x2026 {
                    following = next;
                } else {
                    state.cursor.unread(next);
                }
            }
            let mut punctuation = input::clause_type(current as u32);
            if punctuation != NONE {
                if punctuation & (QUESTION | EXCLAMATION) != 0 {
                    while !eof(state, host)
                        && input::clause_type(following as u32) & (QUESTION | EXCLAMATION) != 0
                    {
                        next = read(state, host)?;
                        following = next;
                    }
                }
                if punctuation & 0x100000 != 0 {
                    stressed = true;
                    state.tone = (punctuation >> 12) & 15;
                    continue;
                }
                if host.classify(following, Class::Space)
                    || punctuation & 0x8000 != 0
                    || host.classify(following, Class::Bracket)
                    || following == 63
                    || eof(state, host)
                    || following == 1
                {
                    end_clause = true;
                }
            }
            if current == 0xe03c {
                current = 60;
            }
            if state.punctuation != 0
                && host.classify(current, Class::Punctuation)
                && state.audio == 0
            {
                // wcschr includes the terminating NUL, as does this prefix scan.
                let listed = state
                    .punctuation_list
                    .iter()
                    .take_while(|c| **c != 0)
                    .any(|c| *c == current as u32)
                    || current == 0;
                if state.punctuation == 1 || listed {
                    state.repeat_count = 0;
                    let mut command = Command::new(2, index)?;
                    command.code = current;
                    command.next = following;
                    command.end = i32::from(end_clause);
                    effect(state, output, host, &mut command)?;
                    index = usize::try_from(command.index).map_err(|_| Error::Backend)?;
                    following = command.next;
                    if index > output.initialized().len() {
                        return Err(Error::Uninitialized);
                    }
                    if command.clause >= 0 {
                        return Ok(command.clause);
                    }
                    announced = current;
                }
            }
            if punctuation & 0x200000 != 0 && announced == 0 {
                let mut command = Command::new(3, index)?;
                command.code = current;
                command.end = 1;
                effect(state, output, host, &mut command)?;
                let length = text_length(&command.text)?;
                output.write(index, &command.text[..length + 1])?;
                if length != 0 {
                    index += length;
                    announced = current;
                    punctuation &= !0x7000;
                }
            }
            if end_clause {
                let mut newlines = 0i32;
                next = following;
                if host.classify(next, Class::Space) {
                    while !eof(state, host) && host.classify(next, Class::Space) {
                        if next == 10 {
                            newlines = add(newlines, 1)?;
                        }
                        next = read(state, host)?;
                    }
                }
                if current == 46 && newlines < 2 {
                    punctuation |= DOT_AFTER;
                }
                if newlines == 0 {
                    if current == 44
                        && previous == 46
                        && state.language == 0x6875
                        && host.classify(previous2, Class::Digit)
                        && (host.classify(next, Class::Digit) || host.classify(next, Class::Lower))
                    {
                        current = 0x557;
                        end_clause = false;
                    }
                    if current == 46 && next == 39 && host.source_peek() == 115 {
                        end_clause = false;
                    }
                    if current == 46 {
                        if state.numbers & 0x10000 != 0
                            && (host.classify(previous, Class::Digit)
                                || (input::roman_upper(previous as u32)
                                    && (input::roman_upper(previous2 as u32)
                                        || host.classify(previous2, Class::Space))))
                            && (!host.classify(previous, Class::Digit)
                                || host.classify(next, Class::Lower)
                                || next == 45)
                        {
                            end_clause = false;
                        }
                        if host.classify(next, Class::Lower) && state.lowercase_sentence == 0 {
                            end_clause = false;
                        }
                        if !any_alnum {
                            current = 32;
                            end_clause = false;
                        }
                    } else if !any_alnum {
                        end_clause = false;
                    }
                    if end_clause && current == 46 && next == 60 && state.ssml != 0 {
                        end_clause = false;
                        delayed_index = index;
                        delayed_clause = punctuation;
                    }
                }
                if end_clause {
                    terminate(state, output, index, Some(next))?;
                    if host.classify(previous, Class::Digit)
                        && !host.classify(next, Class::WordAlpha)
                    {
                        punctuation &= !DOT_AFTER;
                    }
                    if newlines > 1 {
                        return Ok(if matches!(punctuation, QUESTION | EXCLAMATION) {
                            punctuation + 35
                        } else {
                            PARAGRAPH
                        });
                    }
                    return Ok(punctuation);
                } else if !eof(state, host) && host.classify(following, Class::Space) {
                    state.cursor.unread(next);
                }
            }
        }
        if state.parameters[0] == 1 {
            continue;
        }
        if current == announced {
            if host.classify(current, Class::Bracket) {
                current = 0xe028;
            } else if current != 45 {
                current = 32;
            }
        }
        let mut tail = index + 1;
        if current == 0xe03c {
            current = 60;
        }
        encode(output, &mut index, current)?;
        if !host.classify(current, Class::Space) && !host.classify(current, Class::Bracket) {
            output.character_index(index, position(state)?)?;
            while tail < index {
                output.character_index(tail, -1)?;
                tail += 1;
            }
        }
        state.index_top = index as i32;
        if (index as i32 > capacity - 75
            && !host.classify(current, Class::WordAlpha)
            && !host.classify(current, Class::Digit))
            || index as i32 >= capacity - 4
        {
            terminate(state, output, index, Some(following))?;
            return Ok(NONE);
        }
    }
    if stressed {
        encode(output, &mut index, EMPHASIS)?;
    }
    if delayed_clause != 0 {
        remove(output, delayed_index)?;
    }
    terminate(state, output, index, None)?;
    Ok(EOF)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestHost {
        source: Vec<u32>,
        index: usize,
        effects: Vec<u32>,
        defer_punctuation: bool,
    }
    impl TestHost {
        fn new(text: &str) -> Self {
            Self {
                source: text.chars().map(u32::from).chain([0]).collect(),
                index: 0,
                effects: Vec::new(),
                defer_punctuation: false,
            }
        }
    }
    impl Host for TestHost {
        fn source_eof(&self) -> bool {
            self.index >= self.source.len()
        }
        fn source_read(&mut self) -> u32 {
            let code = self.source.get(self.index).copied().unwrap_or(0);
            self.index += 1;
            code
        }
        fn source_peek(&mut self) -> u32 {
            self.source.get(self.index).copied().unwrap_or(0)
        }
        fn classify(&self, code: i32, class: Class) -> bool {
            let Some(code) = char::from_u32(code as u32) else {
                return false;
            };
            match class {
                Class::Space | Class::ByteSpace => code.is_ascii_whitespace(),
                Class::Alnum => code.is_ascii_alphanumeric(),
                Class::Alpha | Class::WordAlpha => code.is_ascii_alphabetic(),
                Class::Upper => code.is_ascii_uppercase(),
                Class::Lower => code.is_ascii_lowercase(),
                Class::Digit => code.is_ascii_digit(),
                Class::Punctuation => code.is_ascii_punctuation(),
                Class::Bracket => matches!(code, '(' | ')' | '[' | ']'),
            }
        }
        fn replace(&self, code: i32) -> Result<input::Replacement, Error> {
            Ok(input::Replacement {
                code,
                ignore: false,
            })
        }
        fn effect(
            &mut self,
            state: &mut State,
            output: &mut dyn Output,
            command: &mut Command,
        ) -> Result<(), Error> {
            self.effects.push(command.kind);
            match command.kind {
                1 => {
                    output.write(command.index as usize, b" ")?;
                    command.index += 1;
                    if command.xml[0] == u32::from(b'v') {
                        state.current_voice[..3].copy_from_slice(b"fr\0");
                        command.clause = VOICE;
                    }
                }
                2 => {
                    if self.defer_punctuation {
                        if command.end != 0 && command.index & 1 != 0 {
                            state.pending_second = command.code;
                            terminate(state, output, 0, Some(command.next))?;
                            command.index = 0;
                            command.clause = -1;
                        } else {
                            let text = b" [\x02punct]]\0";
                            output.write(command.index as usize, text)?;
                            command.index += (text.len() - 1) as i32;
                            command.clause = if command.end != 0 { 0x80004 } else { -1 };
                        }
                    } else {
                        command.clause = -1;
                    }
                }
                3 | 4 => {}
                _ => return Err(Error::Backend),
            }
            Ok(())
        }
    }
    #[test]
    fn clause_boundaries_retain_source_count_replay_and_sparse_indexes() {
        let mut state = State::default();
        let mut host = TestHost::new("Ab. Next");
        let mut bytes = [0xa5; 512];
        let mut indexes = [-7; 512];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(
            read_clause(&mut state, &mut output, &mut host),
            Ok(PERIOD | DOT_AFTER)
        );
        assert_eq!(output.initialized(), b"Ab \0");
        assert_eq!(
            (
                state.cursor.count,
                state.cursor.pending,
                state.index_top,
                state.upper_count,
                state.lower_count
            ),
            (5, 78, 2, 1, 1)
        );
        assert_eq!(&indexes[..4], &[-7, 2, 3, -7]);
        assert_eq!(bytes[4], 0xa5);
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(read_clause(&mut state, &mut output, &mut host), Ok(EOF));
        assert_eq!(output.initialized(), b"Next \0");
        let other = State::default();
        assert_eq!(other.cursor.count, 0);
        assert_eq!(other.replay_index, -1);
    }
    #[test]
    fn ssml_replay_voice_and_private_control_entities_keep_instance_state() {
        let mut state = State {
            ssml: 1,
            ..State::default()
        };
        let mut host = TestHost::new("a &bad;B<v>");
        let mut bytes = [0xa5; 512];
        let mut indexes = [0; 512];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(read_clause(&mut state, &mut output, &mut host), Ok(VOICE));
        assert_eq!(output.initialized(), b"a &bad;B  \0");
        assert_eq!(&state.voice_change[..3], b"fr\0");
        assert_eq!(state.replay_index, -1);
        assert_eq!(host.effects, vec![1]);
        let mut state = State {
            ssml: 1,
            sayas_mode: 0x14,
            ..State::default()
        };
        let mut host = TestHost::new("&#9;");
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(read_clause(&mut state, &mut output, &mut host), Ok(EOF));
        assert_eq!(output.initialized(), b"\xee\x80\x89 \0");
    }
    #[test]
    fn bounds_count_and_suppressed_dot_use_only_initialized_output() {
        let mut state = State::default();
        let mut host = TestHost::new("a");
        let mut bytes = [];
        let mut indexes = [0; 8];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(
            read_clause(&mut state, &mut output, &mut host),
            Err(Error::Capacity)
        );
        let mut state = State::default();
        state.cursor.count = i32::MAX;
        let mut host = TestHost::new("a");
        assert_eq!(
            read_clause(&mut state, &mut output, &mut host),
            Err(Error::Arithmetic)
        );
        assert_eq!(host.index, 0);
        let mut state = State {
            ssml: 1,
            ..State::default()
        };
        state.parameters[0] = 1;
        let mut host = TestHost::new("a.<i>");
        let mut bytes = [0xa5; 512];
        let mut indexes = [0; 512];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        // A suppressed dot leaves a tag separator as the initialized byte to
        // remove. An unwritten byte is rejected by the same sparse-output path.
        assert_eq!(read_clause(&mut state, &mut output, &mut host), Ok(EOF));
        assert_eq!(output.initialized(), b"  \0");
        let mut host = TestHost::new("a.");
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(remove(&mut output, 0), Err(Error::Uninitialized));
        assert_eq!(
            read_clause(&mut state, &mut output, &mut host),
            Ok(PERIOD | DOT_AFTER)
        );
    }
    #[test]
    fn embedded_punctuation_at_source_end_uses_eof_before_read() {
        let mut host = TestHost {
            source: vec![1, 66, 48],
            index: 0,
            effects: vec![],
            defer_punctuation: false,
        };
        let mut state = State {
            punctuation: 1,
            ..State::default()
        };
        let mut bytes = [0xa5; 512];
        let mut indexes = [0; 512];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(read_clause(&mut state, &mut output, &mut host), Ok(EOF));
        assert_eq!(state.punctuation, 0);
        assert_eq!(output.initialized(), b"    \0");
        assert!(host.effects.is_empty());
    }
    #[test]
    fn repeated_backend_deferral_without_source_progress_is_bounded() {
        let mut state = State {
            punctuation: 1,
            ssml: 1,
            ..State::default()
        };
        let mut host = TestHost::new(">..");
        host.defer_punctuation = true;
        let mut bytes = [0xa5; 512];
        let mut indexes = [0; 512];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(
            read_clause(&mut state, &mut output, &mut host),
            Err(Error::Progress)
        );
        assert_eq!(host.index, 4);
        assert!(host.effects.len() < 100);
        // Rejection retains earlier admitted writes; the core does not promise
        // a terminated logical result on an error.
        assert_eq!(output.initialized().first(), Some(&b' '));
        assert!(output.initialized().len() < 32);
    }
    #[test]
    fn four_byte_character_near_capacity_rejects_legacy_terminator_overrun() {
        let mut state = State::default();
        let mut host = TestHost::new("aaaaaaaaaaa\u{1f600}");
        let mut bytes = [0xa5; 16];
        let mut indexes = [0; 16];
        let mut output = Buffer::new(&mut bytes, &mut indexes);
        assert_eq!(
            read_clause(&mut state, &mut output, &mut host),
            Err(Error::Capacity)
        );
        assert_eq!(output.initialized(), b"aaaaaaaaaaa\xf0\x9f\x98\x80");
        assert_eq!(bytes[15], 0xa5);
    }
}
