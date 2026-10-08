//! MBROLA phoneme generation with owned, bounded resume state.
//!
//! Prepare each phoneme once, then retain its command until whole-command
//! admission. Backpressure never replays embedded commands, markers, mapping
//! prefix changes or acoustic programs. The host supplies resident programs,
//! pitch contours and output admission; this driver creates no scheduler.
// Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::generate::{Entry, FmtParams, MAX_ENTRIES};
use crate::mbrola::Selection;
use crate::phoneme_program::PhonemeData;
use std::fmt::Write;

pub const TEXT_CAPACITY: usize = 384;
const MIN_QUEUE: i32 = 25;

#[derive(Clone)]
pub struct Text {
    bytes: [u8; TEXT_CAPACITY],
    length: usize,
}
impl Default for Text {
    fn default() -> Self {
        Self {
            bytes: [0; TEXT_CAPACITY],
            length: 0,
        }
    }
}
impl Text {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub fn terminated(&self) -> &[u8] {
        &self.bytes[..self.length + 1]
    }
    pub fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::Capacity)?;
        if end >= TEXT_CAPACITY || bytes.contains(&0) {
            return Err(Error::Capacity);
        }
        self.bytes[self.length..end].copy_from_slice(bytes);
        self.length = end;
        self.bytes[end] = 0;
        Ok(())
    }
    fn name(&mut self, name: i32) -> Result<(), Error> {
        let bytes = name.to_le_bytes();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(4);
        self.append(&bytes[..end])
    }
}
impl Write for Text {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.append(text.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Settings {
    pub pause_factor: i32,
    pub wav_factor: i32,
    pub sample_rate: i32,
    pub lengthen: i32,
    pub clause_char: i32,
    pub clause_word: i32,
    pub sentences: i32,
    pub phoneme_events: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    Accepted,
    /// Consumed this many bytes from the submitted suffix.
    Progress(usize),
    Pending,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bounds,
    Phoneme,
    Arithmetic,
    Capacity,
    Host,
    Resume,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Generating,
    Flush,
    Done,
    Failed,
}

pub trait Host {
    fn free(&mut self) -> i32;
    /// Refresh after embedded commands, which can change speed/settings.
    fn settings(&mut self) -> Settings;
    fn embedded(&mut self, cursor: &mut i32, source: i32);
    fn marker(&mut self, kind: i32, position: i32, length: i32, value: i32);
    fn phoneme_marker(&mut self, index: usize, ipa: bool, position: i32);
    fn select(&mut self, index: usize) -> Selection;
    fn pitch(&mut self, index: usize, split: i32, final_only: bool) -> Result<Text, Error>;
    fn set_flags(&mut self, index: usize, flags: u16);
    fn interpret(&mut self, index: usize) -> PhonemeData;
    fn sample(&mut self, data: &mut PhonemeData, length: i32) -> i32;
    fn spect(&mut self, index: usize, fmt: &mut FmtParams) -> i32;
    fn pause_length(&mut self, length: i32, control: i32) -> i32;
    /// Admit the suffix at offset. Accepted consumes the entire suffix;
    /// Progress consumes a positive prefix; Pending/Failed consume nothing.
    fn submit(&mut self, text: &Text, offset: usize, file_output: bool) -> Admission;
    fn queue_audio(&mut self, milliseconds: i32);
    /// Request tail servicing after flush admission. This neither predicts
    /// 500 ms of audio nor acknowledges a clause end on the process stream.
    fn drain_audio(&mut self);
    /// Admit the ordinary flush command; this is not an audio acknowledgement.
    fn flush(&mut self) -> Admission;
}

struct Pending {
    text: Text,
    written: usize,
    milliseconds: i32,
    next: usize,
}
pub struct Generator {
    ix: usize,
    embedded: i32,
    words: i32,
    pending: Option<Pending>,
    phase: Phase,
    count: usize,
    file_output: bool,
}
impl Default for Generator {
    fn default() -> Self {
        Self {
            ix: 1,
            embedded: 0,
            words: 0,
            pending: None,
            phase: Phase::Done,
            count: 0,
            file_output: false,
        }
    }
}

fn mul_div(a: i32, b: i32, divisor: i32) -> Result<i32, Error> {
    if divisor <= 0 {
        return Err(Error::Arithmetic);
    }
    a.checked_mul(b)
        .map(|n| n / divisor)
        .ok_or(Error::Arithmetic)
}
fn add(a: i32, b: i32) -> Result<i32, Error> {
    a.checked_add(b).ok_or(Error::Arithmetic)
}

impl Generator {
    /// Return true only when queue/admission backpressure requires a resume.
    /// Restarting clears a previous pending command. A runtime error makes
    /// this run terminal until restarted; earlier effects remain issued.
    /// Invalid bounds/resume parameters are rejected without changing state.
    pub fn translate(
        &mut self,
        entries: &mut [Entry],
        count: usize,
        resume: bool,
        file_output: bool,
        host: &mut impl Host,
    ) -> Result<bool, Error> {
        if entries.len() > MAX_ENTRIES
            || count > entries.len()
            || (count > 1 && count == entries.len())
        {
            return Err(Error::Bounds);
        }
        if !resume {
            self.ix = 1;
            self.embedded = 0;
            self.words = 0;
            self.pending = None;
            self.phase = Phase::Generating;
            self.count = count;
            self.file_output = file_output;
        } else if count != self.count
            || file_output != self.file_output
            || self.phase == Phase::Failed
        {
            return Err(Error::Resume);
        }
        let result = self.run(entries, host);
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }

    fn run(&mut self, entries: &mut [Entry], host: &mut impl Host) -> Result<bool, Error> {
        loop {
            if let Some(pending) = &mut self.pending {
                // Its earlier markers were already queued. Reserve only the
                // one audio entry still to be committed, without replaying them.
                if !self.file_output && host.free() < 1 {
                    return Ok(true);
                }
                match host.submit(&pending.text, pending.written, self.file_output) {
                    Admission::Pending => return Ok(true),
                    Admission::Failed => return Err(Error::Host),
                    Admission::Accepted => {}
                    Admission::Progress(n) => {
                        let remaining = pending.text.bytes().len() - pending.written;
                        if n == 0 || n > remaining {
                            return Err(Error::Host);
                        }
                        pending.written += n;
                        if pending.written != pending.text.bytes().len() {
                            return Ok(true);
                        }
                    }
                }
                if !self.file_output {
                    host.queue_audio(pending.milliseconds);
                }
                self.ix = pending.next;
                self.pending = None;
            }
            match self.phase {
                Phase::Done => return Ok(false),
                Phase::Failed => return Err(Error::Resume),
                Phase::Flush => {
                    if !self.file_output {
                        if host.free() < 1 {
                            return Ok(true);
                        }
                        match host.flush() {
                            Admission::Pending => return Ok(true),
                            Admission::Failed | Admission::Progress(_) => return Err(Error::Host),
                            Admission::Accepted => host.drain_audio(),
                        }
                    }
                    self.phase = Phase::Done;
                    return Ok(false);
                }
                Phase::Generating => {}
            }
            if self.ix >= self.count {
                self.phase = Phase::Flush;
                continue;
            }
            if host.free() < MIN_QUEUE {
                return Ok(true);
            }
            let ix = self.ix;
            let p = entries[ix];
            let next = entries.get(ix + 1).copied().ok_or(Error::Bounds)?;
            if p.present == 0 {
                return Err(Error::Phoneme);
            }
            if p.synthflags & 2 != 0 {
                host.embedded(&mut self.embedded, i32::from(p.source));
            }
            let s = host.settings();
            let position = add(i32::from(p.source & 0x7ff), s.clause_char)?;
            if p.new_word & 4 != 0 {
                host.marker(2, position, 0, s.sentences);
                host.marker(
                    1,
                    position,
                    i32::from(p.source >> 11),
                    add(s.clause_word, self.words)?,
                );
                self.words = add(self.words, 1)?;
            }
            let selection = host.select(ix);
            let following = ix + 1 + usize::from(selection.control & 1 != 0);
            if selection.name == 0 {
                self.ix = following;
                continue;
            }
            let ph = p.phoneme;
            let mut name = selection.name;
            let length = i32::try_from(p.length).map_err(|_| Error::Arithmetic)?;
            let mut len = if ph.kind == 0 && name as u32 == ph.mnemonic {
                name = i32::from(b'_');
                let n = mul_div(length, s.pause_factor, 256)?;
                if n == 0 {
                    1
                } else {
                    n
                }
            } else {
                mul_div(80, s.wav_factor, 256)?
            };
            if ph.code != 15 {
                host.phoneme_marker(ix, s.phoneme_events & 2 != 0, position);
            }
            let mut text = Text::default();
            text.name(name)?;
            text.append(b"\t")?;
            let mut second = selection.second;
            let pause = if second == i32::from(b'_') {
                second = 0;
                selection.percent
            } else {
                0
            };
            let mut final_pitch = Text::default();
            let mut vowel = false;
            match ph.kind {
                2 => {
                    // Only vowels dereference the next phoneme record here.
                    // Name selection may legitimately ignore absent neighbours.
                    if next.present == 0 {
                        return Err(Error::Phoneme);
                    }
                    len = i32::from(ph.standard_length);
                    if p.synthflags & 8 != 0 {
                        len = add(len, s.lengthen)?;
                    }
                    if next.phoneme.kind == 0 {
                        len = add(len, 50)?;
                    }
                    len = mul_div(len, length, 256)?;
                    if second == 0 {
                        let pitch = host.pitch(ix, 0, false)?;
                        write!(text, "{len}\t").map_err(|_| Error::Capacity)?;
                        text.append(pitch.bytes())?;
                    } else {
                        let pitch = host.pitch(ix, selection.percent, false)?;
                        let first = mul_div(len, selection.percent, 100)?;
                        write!(text, "{first}\t").map_err(|_| Error::Capacity)?;
                        text.append(pitch.bytes())?;
                        let split = selection.percent.checked_neg().ok_or(Error::Arithmetic)?;
                        let pitch = host.pitch(ix, split, false)?;
                        text.name(second)?;
                        write!(
                            text,
                            "\t{}\t",
                            len.checked_sub(first).ok_or(Error::Arithmetic)?
                        )
                        .map_err(|_| Error::Capacity)?;
                        text.append(pitch.bytes())?;
                    }
                    vowel = true;
                }
                4 => {
                    if next.kind != 2 && !(next.kind == 3 && next.new_word == 0) {
                        entries[ix].synthflags |= 0x2000;
                        host.set_flags(ix, entries[ix].synthflags);
                    }
                    let mut data = host.interpret(ix);
                    len = mul_div(host.sample(&mut data, 0), 1000, s.sample_rate)?;
                    len = add(len, host.pause_length(i32::from(p.prepause), 1))?;
                }
                5 => len = mul_div(80, s.wav_factor, 256)?,
                6 => {
                    let mut data = host.interpret(ix);
                    let mut samples = if p.synthflags & 8 != 0 {
                        host.sample(&mut data, length)
                    } else {
                        0
                    };
                    samples = add(samples, host.sample(&mut data, length))?;
                    len = mul_div(samples, 1000, s.sample_rate)?;
                }
                8 if next.kind != 2 => {
                    let data = host.interpret(ix);
                    let mut fmt = FmtParams {
                        fmt_addr: data.sound_addresses[0],
                        ..FmtParams::default()
                    };
                    len = mul_div(host.spect(ix, &mut fmt), 1000, s.sample_rate)?;
                    if next.kind == 0 {
                        len = add(len, 50)?;
                    }
                    final_pitch = host.pitch(ix, 0, true)?;
                }
                3 if next.kind == 0 => {
                    len = add(len, 50)?;
                    final_pitch = host.pitch(ix, 0, true)?;
                }
                _ => {}
            }
            if !vowel {
                if second != 0 {
                    let first = mul_div(len, selection.percent, 100)?;
                    writeln!(text, "{first}").map_err(|_| Error::Capacity)?;
                    text.name(second)?;
                    text.append(b"\t")?;
                    len = len.checked_sub(first).ok_or(Error::Arithmetic)?;
                }
                write!(text, "{len}").map_err(|_| Error::Capacity)?;
                text.append(final_pitch.bytes())?;
                text.append(b"\n")?;
            }
            if pause != 0 {
                len = add(len, host.pause_length(pause, 0))?;
                writeln!(text, "_ \t{}", host.pause_length(pause, 0))
                    .map_err(|_| Error::Capacity)?;
            }
            self.pending = Some(Pending {
                text,
                written: 0,
                milliseconds: len,
                next: following,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Mock {
        free: i32,
        blocked: usize,
        flush_blocked: usize,
        failed: bool,
        selected: Vec<usize>,
        embedded: Vec<i32>,
        words: Vec<i32>,
        markers: Vec<usize>,
        audio: Vec<i32>,
        attempts: Vec<Vec<u8>>,
        addresses: Vec<usize>,
        flush_calls: usize,
        partial: usize,
        accepted: Vec<u8>,
        invalid_progress: Option<usize>,
    }
    impl Host for Mock {
        fn free(&mut self) -> i32 {
            self.free
        }
        fn settings(&mut self) -> Settings {
            Settings {
                pause_factor: 256,
                wav_factor: 256,
                sample_rate: 22050,
                lengthen: 70,
                clause_char: 100,
                clause_word: 60,
                sentences: 12,
                phoneme_events: 2,
            }
        }
        fn embedded(&mut self, cursor: &mut i32, _source: i32) {
            self.embedded.push(*cursor);
            *cursor += 1;
        }
        fn marker(&mut self, kind: i32, _position: i32, _length: i32, value: i32) {
            if kind == 1 {
                self.words.push(value);
            }
        }
        fn phoneme_marker(&mut self, index: usize, ipa: bool, _position: i32) {
            assert!(ipa);
            self.markers.push(index);
        }
        fn select(&mut self, index: usize) -> Selection {
            self.selected.push(index);
            Selection {
                name: if index == 1 {
                    i32::from(b'a')
                } else {
                    i32::from(b'b')
                },
                control: i32::from(index == 1),
                ..Selection::default()
            }
        }
        fn pitch(&mut self, _index: usize, _split: i32, _final_only: bool) -> Result<Text, Error> {
            panic!("consonant fixture does not use pitch")
        }
        fn set_flags(&mut self, _index: usize, _flags: u16) {
            panic!("not a stop")
        }
        fn interpret(&mut self, _index: usize) -> PhonemeData {
            panic!("not sampled")
        }
        fn sample(&mut self, _data: &mut PhonemeData, _length: i32) -> i32 {
            panic!("not sampled")
        }
        fn spect(&mut self, _index: usize, _fmt: &mut FmtParams) -> i32 {
            panic!("not spectral")
        }
        fn pause_length(&mut self, _length: i32, _control: i32) -> i32 {
            panic!("no appended pause")
        }
        fn submit(&mut self, text: &Text, offset: usize, _file: bool) -> Admission {
            let suffix = &text.bytes()[offset..];
            self.attempts.push(suffix.to_vec());
            self.addresses.push(text.bytes().as_ptr() as usize);
            if let Some(n) = self.invalid_progress {
                return Admission::Progress(n);
            }
            if self.failed {
                return Admission::Failed;
            }
            if self.blocked != 0 {
                self.blocked -= 1;
                Admission::Pending
            } else if self.partial != 0 && self.partial < suffix.len() {
                self.accepted.extend_from_slice(&suffix[..self.partial]);
                Admission::Progress(self.partial)
            } else {
                self.accepted.extend_from_slice(suffix);
                Admission::Accepted
            }
        }
        fn queue_audio(&mut self, milliseconds: i32) {
            self.audio.push(milliseconds);
        }
        fn drain_audio(&mut self) {
            self.audio.push(500);
        }
        fn flush(&mut self) -> Admission {
            self.flush_calls += 1;
            if self.flush_blocked != 0 {
                self.flush_blocked -= 1;
                Admission::Pending
            } else {
                Admission::Accepted
            }
        }
    }
    fn entries() -> [Entry; 5] {
        let mut result = [Entry::default(); 5];
        for p in &mut result {
            p.present = 1;
            p.phoneme.kind = 5;
            p.phoneme.code = 40;
            p.length = 256;
            p.synthflags = 2;
            p.new_word = 4;
        }
        result
    }
    #[test]
    fn partial_admission_retries_only_suffix_and_commits_effects_once() {
        let mut entries = entries();
        let mut generator = Generator::default();
        let mut host = Mock {
            free: 100,
            partial: 2,
            ..Mock::default()
        };
        assert_eq!(
            generator.translate(&mut entries, 4, false, false, &mut host),
            Ok(true)
        );
        assert_eq!(host.accepted, b"a\t");
        assert!(host.audio.is_empty());
        host.blocked = 1;
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(true)
        );
        assert_eq!(host.accepted, b"a\t");
        for _ in 0..3 {
            assert_eq!(
                generator.translate(&mut entries, 4, true, false, &mut host),
                Ok(true)
            );
        }
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(false)
        );
        assert_eq!(host.accepted, b"a\t80\nb\t80\n");
        assert_eq!(
            host.attempts,
            [
                b"a\t80\n".to_vec(),
                b"80\n".to_vec(),
                b"80\n".to_vec(),
                b"\n".to_vec(),
                b"b\t80\n".to_vec(),
                b"80\n".to_vec(),
                b"\n".to_vec()
            ]
        );
        assert_eq!(host.selected, [1, 3]);
        assert_eq!(host.embedded, [0, 1]);
        assert_eq!(host.markers, [1, 3]);
        assert_eq!(host.words, [60, 61]);
        assert_eq!(host.audio, [80, 80, 500]);
        assert_eq!(host.flush_calls, 1);
        for n in [0, 6, usize::MAX] {
            let mut generator = Generator::default();
            let mut host = Mock {
                free: 100,
                invalid_progress: Some(n),
                ..Mock::default()
            };
            assert_eq!(
                generator.translate(&mut entries, 4, false, false, &mut host),
                Err(Error::Host)
            );
            assert!(host.audio.is_empty());
            assert_eq!(
                generator.translate(&mut entries, 4, true, false, &mut host),
                Err(Error::Resume)
            );
        }
    }
    #[test]
    fn command_backpressure_preserves_prepared_text_and_skipped_index_once() {
        let mut entries = entries();
        let mut generator = Generator::default();
        let mut host = Mock {
            free: 100,
            blocked: 3,
            flush_blocked: 2,
            ..Mock::default()
        };
        assert_eq!(
            generator.translate(&mut entries, 4, false, false, &mut host),
            Ok(true)
        );
        for _ in 0..2 {
            assert_eq!(
                generator.translate(&mut entries, 4, true, false, &mut host),
                Ok(true)
            );
        }
        assert_eq!(host.selected, [1]);
        assert_eq!(host.embedded, [0]);
        assert_eq!(host.words, [60]);
        assert_eq!(host.markers, [1]);
        assert!(host.audio.is_empty());
        assert_eq!(host.attempts, vec![b"a\t80\n".to_vec(); 3]);
        assert!(host.addresses.iter().all(|&p| p == host.addresses[0]));
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(true)
        );
        assert_eq!(host.selected, [1, 3]);
        assert_eq!(host.embedded, [0, 1]);
        assert_eq!(host.words, [60, 61]);
        assert_eq!(host.markers, [1, 3]);
        assert_eq!(host.audio, [80, 80]);
        assert_eq!(host.attempts[3], b"a\t80\n");
        assert_eq!(host.attempts[4], b"b\t80\n");
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(true)
        );
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(false)
        );
        assert_eq!(host.flush_calls, 3);
        assert_eq!(host.audio, [80, 80, 500]);
        let attempts = host.attempts.len();
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(false)
        );
        assert_eq!(host.flush_calls, 3);
        assert_eq!(host.attempts.len(), attempts);
    }
    #[test]
    fn pending_submission_waits_for_audio_queue_space_and_restart_discards_it() {
        let mut entries = entries();
        let mut generator = Generator::default();
        let mut host = Mock {
            free: 24,
            blocked: 1,
            ..Mock::default()
        };
        assert_eq!(
            generator.translate(&mut entries, 4, false, false, &mut host),
            Ok(true)
        );
        assert!(host.selected.is_empty());
        host.free = 100;
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(true)
        );
        host.free = 0;
        assert_eq!(
            generator.translate(&mut entries, 4, true, false, &mut host),
            Ok(true)
        );
        assert_eq!(host.attempts.len(), 1);
        // A different restarted clause may use the owner without carrying its
        // old pending bytes/cursors into that clause.
        host.free = 100;
        assert_eq!(
            generator.translate(&mut entries, 0, false, true, &mut host),
            Ok(false)
        );
        assert_eq!(host.attempts.len(), 1);
        assert_eq!(host.flush_calls, 0);
    }
    #[test]
    fn errors_terminate_the_run_and_owners_do_not_share_state() {
        let mut entries = entries();
        let mut failed = Generator::default();
        let mut host = Mock {
            free: 100,
            failed: true,
            ..Mock::default()
        };
        assert_eq!(
            failed.translate(&mut entries, 4, false, false, &mut host),
            Err(Error::Host)
        );
        assert_eq!(
            failed.translate(&mut entries, 4, true, false, &mut host),
            Err(Error::Resume)
        );
        assert_eq!(host.attempts.len(), 1);
        let mut second = Generator::default();
        let mut other = Mock {
            free: 100,
            ..Mock::default()
        };
        assert_eq!(
            second.translate(&mut entries, 4, false, true, &mut other),
            Ok(false)
        );
        assert_eq!(other.words, [60, 61]);
        assert!(other.audio.is_empty());
        assert_eq!(other.flush_calls, 0);
        assert_eq!(
            second.translate(&mut entries, 3, true, true, &mut other),
            Err(Error::Resume)
        );
        assert_eq!(
            second.translate(&mut entries, 4, true, false, &mut other),
            Err(Error::Resume)
        );
    }
    #[test]
    fn malformed_bounds_and_mnemonics_are_bounded() {
        let mut generator = Generator::default();
        let mut host = Mock {
            free: 100,
            ..Mock::default()
        };
        assert_eq!(
            generator.translate(&mut [], 2, false, false, &mut host),
            Err(Error::Bounds)
        );
        let mut entries = entries();
        entries[1].present = 0;
        assert_eq!(
            generator.translate(&mut entries, 4, false, false, &mut host),
            Err(Error::Phoneme)
        );
        assert!(host.selected.is_empty());
        let mut text = Text::default();
        text.name(i32::from_le_bytes([0xff, 0x80, 0, b'z']))
            .unwrap();
        assert_eq!(text.terminated(), [0xff, 0x80, 0]);
        assert_eq!(text.append(&[1; TEXT_CAPACITY]), Err(Error::Capacity));
        assert_eq!(text.terminated(), [0xff, 0x80, 0]);
        assert_eq!(mul_div(i32::MAX, 1000, 22050), Err(Error::Arithmetic));
        assert_eq!(mul_div(1, 1000, 0), Err(Error::Arithmetic));
    }
}
