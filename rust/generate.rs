//! Clause synthesis driver (`Generate`).
//!
//! Walks a clause's phoneme list and decides, per phoneme type, which
//! synthesis commands to issue and with which pitch, amplitude, spectrum and
//! sample parameters: pauses, envelopes, vowel starts and ends from this or the
//! neighbouring phonemes' programs, word/sentence/phoneme markers and embedded
//! commands. Generation suspends when the host's command queue is short of
//! space and resumes from [`State`]. The command queue, frames, samples and
//! phoneme programs stay with the [`Host`], called in C's order.
// Copyright (C) 2005 to 2014 Jonathan Duddington, (C) 2015-2017 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{phoneme::Phoneme, phoneme_program::PhonemeData};

/// `N_PHONEME_LIST + 1`, the capacity of the engine's phoneme list.
pub const MAX_ENTRIES: usize = 1001;
/// Generation stops before `N_PHONEME_LIST - 2`.
const LIST_LIMIT: usize = 998;
/// Envelope tables (`N_ENVELOPE_DATA`).
const N_ENVELOPES: i32 = 20;

const SEQ_CONTINUE: u16 = 0x01;
const EMBEDDED: u16 = 0x02;
const LENGTHEN: u16 = 0x08;
const NEXT_PAUSE: u16 = 0x2000;

const PH_PAUSE: u8 = 0;
const PH_VOWEL: u8 = 2;
const PH_LIQUID: u8 = 3;
const PH_STOP: u8 = 4;
const PH_VSTOP: u8 = 5;
const PH_FRICATIVE: u8 = 6;
const PH_VFRICATIVE: u8 = 7;
const PH_NASAL: u8 = 8;

const TRILL: u32 = 1 << 7;
const NO_PAUSE: u32 = 1 << 24;
const PREVOICE: u32 = 1 << 25;
const PHON_END_WORD: u8 = 15;

const START_OF_WORD: u8 = 1;
const START_OF_SENTENCE: u8 = 4;

const EVENT_WORD: i32 = 1;
const EVENT_SENTENCE: i32 = 2;
const EVENT_END: i32 = 5;
const PHONEME_IPA: i32 = 2;

const PITCH_RISE: i32 = 2;
const MIN_WCMDQ: i32 = 25;

// PHONEME_DATA indices
const FMT: usize = 0;
const VOWEL_START: usize = 2;
const VOWEL_END: usize = 3;
const ADD_WAV: usize = 4;
const PAUSE_BEFORE: usize = 7;
const SET_LENGTH: usize = 10;
const FOR_NEXT_PHONEME: i32 = 0x2;
const DONT_LENGTHEN: i32 = 0x4;

/// One copied phoneme-list entry with its phoneme record. Only `synthflags`
/// changes, and the host is told when it does.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Entry {
    /// `*ph`, meaningful when `present` is nonzero.
    pub phoneme: Phoneme,
    pub length: u32,
    pub synthflags: u16,
    pub source: u16,
    pub kind: u8,
    pub new_word: u8,
    pub prepause: u8,
    pub amp: u8,
    pub env: u8,
    pub pitch1: u8,
    pub pitch2: u8,
    pub stress: u8,
    pub tone: u8,
    pub present: u8,
}

/// The layout of `FMT_PARAMS`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FmtParams {
    pub fmt_control: i32,
    pub use_vowelin: i32,
    pub fmt_addr: i32,
    pub fmt_length: i32,
    pub fmt_amp: i32,
    pub fmt2_addr: i32,
    pub fmt2_lenadj: i32,
    pub wav_addr: i32,
    pub wav_amp: i32,
    pub transition0: i32,
    pub transition1: i32,
    pub std_length: i32,
}

/// A pitch or amplitude envelope: an `envelope_data` table, or an address
/// in the phoneme data (`GetEnvelope`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Envelope {
    Table(i32),
    Data(i32),
}

/// Position within a clause, kept across suspensions (the legacy statics).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct State {
    pub ix: i32,
    pub embedded_ix: i32,
    pub word_count: i32,
    /// The current word's text position; carried across clauses, as in C.
    pub source: i32,
}

/// Engine options and clause counters for one call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Settings {
    /// `option_phoneme_events`.
    pub phoneme_events: i32,
    /// `param[LOPT_WORD_MERGE]`.
    pub word_merge: i32,
    pub clause_start_char: i32,
    pub clause_start_word: i32,
    pub count_sentences: i32,
    pub count_characters: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// More than [`MAX_ENTRIES`] entries, or a count past them.
    TooLong,
    /// A neighbour read past the supplied entries.
    Bounds,
    /// A phoneme C dereferences is missing.
    Phoneme,
    /// An envelope number past the envelope tables.
    Envelope,
    /// The host could not run a tone phoneme's program.
    Host,
}

/// The command queue, frames, samples and phoneme programs.
pub trait Host {
    /// Free command-queue entries (`WcmdqFree`).
    fn free(&mut self) -> i32;
    /// Starts a clause: resets envelope lengths, frame and syllable marks and
    /// the word's previous-vowel data.
    fn reset(&mut self);
    /// Whether a pitch command has been issued in this clause.
    fn pitch_started(&mut self) -> bool;
    /// Reports the entry's phoneme to an output hook, if one is installed.
    fn alignment(&mut self, index: usize);
    /// Applies embedded commands at `*embedded_ix`, advancing it.
    fn embedded(&mut self, embedded_ix: &mut i32, source: i32);
    /// Breaks spectrum continuity (`last_frame = NULL`).
    fn break_frame(&mut self);
    fn marker(&mut self, kind: i32, position: i32, length: i32, value: i32);
    /// Emits a phoneme event naming the entry's phoneme.
    fn phoneme_marker(&mut self, index: usize, ipa: bool, position: i32);
    fn end_amplitude(&mut self);
    fn end_pitch(&mut self, voice_break: bool);
    fn pause(&mut self, length: i32, control: i32);
    fn amplitude(&mut self, amp: i32, envelope: Option<Envelope>);
    fn pitch(&mut self, envelope: Envelope, pitch1: i32, pitch2: i32);
    fn start_syllable(&mut self);
    /// Runs the entry's phoneme program; `word` passes the word data.
    fn interpret(&mut self, index: usize, control: i32, word: bool) -> PhonemeData;
    /// Runs the entry's tone phoneme program.
    fn tone(&mut self, index: usize) -> Result<PhonemeData, Error>;
    /// Queues the entry's spectrum sequence (`DoSpect2`), which may clear
    /// `fmt.wav_addr`.
    fn spect(&mut self, index: usize, which: i32, fmt: &mut FmtParams, modulation: i32);
    /// Queues the program's sampled sound (`DoSample3`).
    fn sample(&mut self, data: &mut PhonemeData, length_mod: i32, amp: i32);
    fn set_synthflags(&mut self, index: usize, flags: u16);
    fn set_std_length(&mut self, index: usize, value: u8);
}

struct Clause<'a> {
    entries: &'a mut [Entry],
}

impl Clause<'_> {
    fn at(&self, index: usize) -> Result<Entry, Error> {
        self.entries.get(index).copied().ok_or(Error::Bounds)
    }
}

fn phoneme(entry: &Entry) -> Result<Phoneme, Error> {
    if entry.present != 0 {
        Ok(entry.phoneme)
    } else {
        Err(Error::Phoneme)
    }
}

fn table(env: u8) -> Result<Envelope, Error> {
    let env = i32::from(env);
    if env < N_ENVELOPES {
        Ok(Envelope::Table(env))
    } else {
        Err(Error::Envelope)
    }
}

fn add_flags(clause: &mut Clause<'_>, host: &mut impl Host, index: usize, flags: u16) {
    let entry = &mut clause.entries[index];
    entry.synthflags |= flags;
    host.set_synthflags(index, entry.synthflags);
}

/// Generates commands for a clause (`Generate`). Returns `Ok(true)` when the
/// queue is short of space: call again with `resume` set once it drains.
/// Otherwise the clause is finished and `count` is set to 0.
///
/// `entries` holds the clause's `count` entries and any following entries
/// the neighbour reads may reach. On error, generation stopped part way;
/// commands already issued stay issued.
pub fn generate(
    entries: &mut [Entry],
    count: &mut usize,
    resume: bool,
    state: &mut State,
    settings: &Settings,
    host: &mut impl Host,
) -> Result<bool, Error> {
    if entries.len() > MAX_ENTRIES || *count > entries.len() {
        return Err(Error::TooLong);
    }
    let mut clause = Clause { entries };
    let s = settings;
    let use_ipa = s.phoneme_events & PHONEME_IPA != 0;

    if !resume {
        state.ix = 1;
        state.embedded_ix = 0;
        state.word_count = 0;
        host.reset();
        host.pause(0, 0); // isolate from the previous clause
    }

    while (state.ix as usize) < *count && (state.ix as usize) < LIST_LIMIT {
        let ix = state.ix as usize;
        let p = clause.at(ix)?;
        host.alignment(ix);

        let free_min = match p.kind {
            PH_PAUSE => 10,
            // less space for non-vowels; a vowel must follow to fill the pitch length
            PH_VOWEL => MIN_WCMDQ,
            _ => 15,
        };
        if host.free() <= free_min {
            return Ok(true); // wait
        }

        let prev = clause.at(ix.checked_sub(1).ok_or(Error::Bounds)?)?;
        let next = || clause.at(ix + 1);

        if p.synthflags & EMBEDDED != 0 {
            host.embedded(&mut state.embedded_ix, i32::from(p.source));
        }

        if p.new_word != 0 {
            let merges = p.kind == PH_VOWEL && s.word_merge & 1 != 0;
            if !(merges || phoneme(&p)?.flags & NO_PAUSE != 0) {
                host.break_frame();
            }
            state.source = i32::from(p.source & 0x7ff) + s.clause_start_char;
            if p.new_word & START_OF_SENTENCE != 0 {
                host.marker(EVENT_SENTENCE, state.source, 0, s.count_sentences);
            }
            if p.new_word & START_OF_WORD != 0 {
                // this count doesn't include multiple-word pronunciations in *_list
                let word = s.clause_start_word + state.word_count;
                state.word_count += 1;
                host.marker(EVENT_WORD, state.source, i32::from(p.source >> 11), word);
            }
        }

        host.end_amplitude();

        if p.prepause > 0 && phoneme(&p)?.flags & PREVOICE == 0 {
            host.pause(i32::from(p.prepause), 1);
        }

        let mut done_phoneme_marker = false;
        if s.phoneme_events != 0 && phoneme(&p)?.code != PHON_END_WORD {
            // after a liquid or nasal, a vowel's event follows its start
            if !(p.kind == PH_VOWEL && (prev.kind == PH_LIQUID || prev.kind == PH_NASAL)) {
                host.phoneme_marker(ix, use_ipa, state.source);
                done_phoneme_marker = true;
            }
        }

        match p.kind {
            PH_PAUSE => {
                host.pause(p.length as i32, 0);
                host.set_std_length(ix, phoneme(&p)?.standard_length);
            }
            PH_STOP => {
                let ph = phoneme(&p)?;
                let n = next()?;
                let released = n.kind == PH_VOWEL || (n.new_word == 0 && n.kind == PH_LIQUID);
                if !released {
                    add_flags(&mut clause, host, ix, NEXT_PAUSE);
                }
                if ph.flags & PREVOICE != 0 {
                    // a period of voicing before the release
                    let mut fmt = FmtParams::default();
                    let data = host.interpret(ix, 0x01, true);
                    fmt.fmt_addr = data.sound_addresses[FMT];
                    fmt.fmt_amp = data.sound_parameters[FMT];
                    if !host.pitch_started() {
                        host.amplitude(i32::from(n.amp), None);
                        host.pitch(table(p.env)?, i32::from(n.pitch1), i32::from(n.pitch2));
                    }
                    host.spect(ix, 0, &mut fmt, 0);
                }
                let mut data = host.interpret(ix, 0, true);
                data.control |= DONT_LENGTHEN;
                host.sample(&mut data, 0, 0);
            }
            PH_FRICATIVE => {
                let mut data = host.interpret(ix, 0, true);
                if p.synthflags & LENGTHEN != 0 {
                    host.sample(&mut data, p.length as i32, 0); // twice for [s:] etc.
                }
                host.sample(&mut data, p.length as i32, 0);
            }
            PH_VSTOP => {
                let ph = phoneme(&p)?;
                let n = next()?;
                let mut fmt = FmtParams {
                    fmt_control: DONT_LENGTHEN,
                    ..FmtParams::default()
                };
                let mut pre_voiced = false;
                if n.kind == PH_VOWEL {
                    host.amplitude(i32::from(p.amp), None);
                    host.pitch(table(p.env)?, i32::from(p.pitch1), i32::from(p.pitch2));
                    pre_voiced = true;
                } else if n.kind == PH_LIQUID && n.new_word == 0 {
                    host.amplitude(i32::from(n.amp), None);
                    host.pitch(table(n.env)?, i32::from(n.pitch1), i32::from(n.pitch2));
                    pre_voiced = true;
                } else if !host.pitch_started() {
                    host.amplitude(i32::from(n.amp), None);
                    host.pitch(table(p.env)?, i32::from(p.pitch1), i32::from(p.pitch2));
                }

                if prev.kind == PH_VOWEL || ph.flags & PREVOICE != 0 {
                    // a period of voicing before the release
                    let data = host.interpret(ix, 0x01, true);
                    fmt.fmt_addr = data.sound_addresses[FMT];
                    fmt.fmt_amp = data.sound_parameters[FMT];
                    host.spect(ix, 0, &mut fmt, 0);
                    if p.synthflags & LENGTHEN != 0 {
                        host.pause(25, 1);
                        host.spect(ix, 0, &mut fmt, 0);
                    }
                } else if p.synthflags & LENGTHEN != 0 {
                    host.pause(50, 0);
                }

                if pre_voiced {
                    host.start_syllable(); // followed by a vowel, or liquid + vowel
                } else {
                    add_flags(&mut clause, host, ix, NEXT_PAUSE);
                }
                let data = host.interpret(ix, 0, true);
                fmt.fmt_addr = data.sound_addresses[FMT];
                fmt.fmt_amp = data.sound_parameters[FMT];
                fmt.wav_addr = data.sound_addresses[ADD_WAV];
                fmt.wav_amp = data.sound_parameters[ADD_WAV];
                host.spect(ix, 0, &mut fmt, 0);

                if p.new_word == 0 && clause.at(ix + 2)?.new_word == 0 {
                    if n.kind == PH_VFRICATIVE {
                        host.pause(20, 0);
                    }
                    if n.kind == PH_FRICATIVE {
                        host.pause(12, 0);
                    }
                }
            }
            PH_VFRICATIVE => {
                let n = next()?;
                if n.kind == PH_VOWEL {
                    host.amplitude(i32::from(p.amp), None);
                    host.pitch(table(p.env)?, i32::from(p.pitch1), i32::from(p.pitch2));
                } else if n.kind == PH_LIQUID {
                    host.amplitude(i32::from(n.amp), None);
                    host.pitch(table(n.env)?, i32::from(n.pitch1), i32::from(n.pitch2));
                } else if !host.pitch_started() {
                    host.amplitude(i32::from(p.amp), None);
                    host.pitch(table(p.env)?, i32::from(p.pitch1), i32::from(p.pitch2));
                }
                if n.kind == PH_VOWEL || (n.kind == PH_LIQUID && n.new_word == 0) {
                    host.start_syllable();
                } else {
                    add_flags(&mut clause, host, ix, NEXT_PAUSE);
                }
                let data = host.interpret(ix, 0, true);
                let mut fmt = FmtParams {
                    std_length: data.parameters[SET_LENGTH].wrapping_mul(2),
                    fmt_addr: data.sound_addresses[FMT],
                    fmt_amp: data.sound_parameters[FMT],
                    wav_addr: data.sound_addresses[ADD_WAV],
                    wav_amp: data.sound_parameters[ADD_WAV],
                    ..FmtParams::default()
                };
                if p.synthflags & LENGTHEN != 0 {
                    host.spect(ix, 0, &mut fmt, 0);
                }
                host.spect(ix, 0, &mut fmt, 0);
            }
            PH_NASAL => {
                if p.synthflags & SEQ_CONTINUE == 0 {
                    host.amplitude(i32::from(p.amp), None);
                    host.pitch(table(p.env)?, i32::from(p.pitch1), i32::from(p.pitch2));
                }
                if prev.kind == PH_NASAL {
                    host.break_frame();
                }
                let data = host.interpret(ix, 0, true);
                let mut fmt = FmtParams {
                    std_length: data.parameters[SET_LENGTH].wrapping_mul(2),
                    fmt_addr: data.sound_addresses[FMT],
                    fmt_amp: data.sound_parameters[FMT],
                    ..FmtParams::default()
                };
                if next()?.kind == PH_VOWEL {
                    host.start_syllable();
                    host.spect(ix, 0, &mut fmt, 0);
                } else if prev.kind == PH_VOWEL && p.synthflags & SEQ_CONTINUE != 0 {
                    host.spect(ix, 0, &mut fmt, 0);
                } else {
                    host.break_frame(); // only for nasal ?
                    host.spect(ix, 0, &mut fmt, 0);
                    host.break_frame();
                }
            }
            PH_LIQUID => {
                let modulation = if phoneme(&p)?.flags & TRILL != 0 {
                    5
                } else {
                    0
                };
                if p.synthflags & SEQ_CONTINUE == 0 {
                    host.amplitude(i32::from(p.amp), None);
                    host.pitch(table(p.env)?, i32::from(p.pitch1), i32::from(p.pitch2));
                }
                if prev.kind == PH_NASAL {
                    host.break_frame();
                }
                if next()?.kind == PH_VOWEL {
                    host.start_syllable();
                }
                let data = host.interpret(ix, 0, true);
                let value = data.parameters[PAUSE_BEFORE].wrapping_sub(i32::from(p.prepause));
                if value > 0 {
                    host.pause(value, 1);
                }
                let mut fmt = FmtParams {
                    std_length: data.parameters[SET_LENGTH].wrapping_mul(2),
                    fmt_addr: data.sound_addresses[FMT],
                    fmt_amp: data.sound_parameters[FMT],
                    wav_addr: data.sound_addresses[ADD_WAV],
                    wav_amp: data.sound_parameters[ADD_WAV],
                    ..FmtParams::default()
                };
                host.spect(ix, 0, &mut fmt, modulation);
            }
            PH_VOWEL => vowel(
                &mut clause,
                host,
                s,
                state,
                ix,
                p,
                prev,
                done_phoneme_marker,
                use_ipa,
            )?,
            _ => {}
        }
        state.ix += 1;
    }
    host.end_pitch(true);
    if *count > 0 {
        host.marker(EVENT_END, s.count_characters, 0, s.count_sentences); // end of clause
        *count = 0;
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn vowel(
    clause: &mut Clause<'_>,
    host: &mut impl Host,
    s: &Settings,
    state: &State,
    ix: usize,
    p: Entry,
    prev: Entry,
    done_phoneme_marker: bool,
    use_ipa: bool,
) -> Result<(), Error> {
    let stress = p.stress & 0xf;
    let data = host.interpret(ix, 0, true);
    let mut fmt = FmtParams {
        std_length: data.parameters[SET_LENGTH].wrapping_mul(2),
        ..FmtParams::default()
    };
    let mut vowelstart_prev = false;

    fmt.fmt_addr = data.sound_addresses[VOWEL_START];
    if fmt.fmt_addr != 0 && data.control & FOR_NEXT_PHONEME == 0 {
        // a vowel start specified by the vowel's program
        fmt.fmt_length = data.sound_parameters[VOWEL_START];
    } else if prev.kind != PH_PAUSE {
        // check the previous phoneme
        let before = host.interpret(ix - 1, 0, false);
        fmt.fmt_addr = before.sound_addresses[VOWEL_START];
        if fmt.fmt_addr != 0 && before.control & FOR_NEXT_PHONEME != 0 {
            // a vowel start specified by the previous phoneme
            vowelstart_prev = true;
            fmt.fmt2_lenadj = before.sound_parameters[VOWEL_START];
        }
        fmt.transition0 = before.vowel_transitions[0];
        fmt.transition1 = before.vowel_transitions[1];
    }
    if fmt.fmt_addr == 0 {
        // the vowel's default start
        fmt.use_vowelin = 1;
        fmt.fmt_control = 1;
        fmt.fmt_addr = data.sound_addresses[FMT];
    }
    fmt.fmt_amp = data.sound_parameters[FMT];

    let mut pitch_env = table(p.env)?;
    let mut amp_env = None;
    if p.tone != 0 {
        let tone = host.tone(ix)?;
        pitch_env = Envelope::Data(tone.pitch_envelope);
        if tone.amplitude_envelope > 0 {
            amp_env = Some(Envelope::Data(tone.amplitude_envelope));
        }
    }

    host.start_syllable();

    let modulation = match stress {
        0 | 1 => 1, // 16ths
        7.. => 3,
        _ => 2,
    };
    let (amp, pitch1, pitch2) = (i32::from(p.amp), i32::from(p.pitch1), i32::from(p.pitch2));
    if prev.kind == PH_VSTOP || prev.kind == PH_VFRICATIVE {
        host.amplitude(amp, amp_env);
        host.pitch(pitch_env, pitch1, pitch2); // no prevocalic rising tone
        host.spect(ix, 1, &mut fmt, modulation);
    } else if prev.kind == PH_LIQUID || prev.kind == PH_NASAL {
        host.amplitude(amp, amp_env);
        host.spect(ix, 1, &mut fmt, modulation); // continue the pre-vocalic rise
        host.pitch(pitch_env, pitch1, pitch2);
    } else if vowelstart_prev {
        // a vowel start from the previous phoneme, not a liquid or nasal
        host.pitch(Envelope::Table(PITCH_RISE), pitch2 - 15, pitch2);
        host.amplitude(amp - 1, amp_env);
        host.spect(ix, 1, &mut fmt, modulation);
        host.pitch(pitch_env, pitch1, pitch2);
    } else {
        if p.synthflags & SEQ_CONTINUE == 0 {
            host.amplitude(amp, amp_env);
            host.pitch(pitch_env, pitch1, pitch2);
        }
        host.spect(ix, 1, &mut fmt, modulation);
    }

    if s.phoneme_events != 0 && !done_phoneme_marker {
        host.phoneme_marker(ix, use_ipa, state.source);
    }

    fmt.fmt_addr = data.sound_addresses[FMT];
    fmt.fmt_amp = data.sound_parameters[FMT];
    fmt.transition0 = 0;
    fmt.transition1 = 0;
    fmt.fmt2_addr = data.sound_addresses[VOWEL_END];
    if fmt.fmt2_addr != 0 {
        fmt.fmt2_lenadj = data.sound_parameters[VOWEL_END];
    } else if clause.at(ix + 1)?.kind != PH_PAUSE {
        fmt.fmt2_lenadj = 0;
        let after = host.interpret(ix + 1, 0, false);
        fmt.use_vowelin = 1;
        // always the vowel transition, even with an ending ?? consider [N]
        fmt.transition0 = after.vowel_transitions[2];
        fmt.transition1 = after.vowel_transitions[3];
        fmt.fmt2_addr = after.sound_addresses[VOWEL_END];
        if fmt.fmt2_addr != 0 {
            fmt.fmt2_lenadj = after.sound_parameters[VOWEL_END];
        }
    }
    host.spect(ix, 2, &mut fmt, modulation);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        log: Vec<String>,
        free: Vec<i32>,
        pitch: bool,
    }
    impl Host for Recorder {
        fn free(&mut self) -> i32 {
            self.free.pop().unwrap_or(100)
        }
        fn reset(&mut self) {
            self.pitch = false;
            self.log.push("reset".into());
        }
        fn pitch_started(&mut self) -> bool {
            self.pitch
        }
        fn alignment(&mut self, _: usize) {}
        fn embedded(&mut self, ix: &mut i32, _: i32) {
            *ix += 1;
        }
        fn break_frame(&mut self) {
            self.log.push("break".into());
        }
        fn marker(&mut self, kind: i32, position: i32, length: i32, value: i32) {
            self.log
                .push(format!("marker {kind} {position} {length} {value}"));
        }
        fn phoneme_marker(&mut self, index: usize, _: bool, _: i32) {
            self.log.push(format!("phoneme {index}"));
        }
        fn end_amplitude(&mut self) {}
        fn end_pitch(&mut self, voice_break: bool) {
            self.log.push(format!("end pitch {voice_break}"));
        }
        fn pause(&mut self, length: i32, control: i32) {
            self.log.push(format!("pause {length} {control}"));
        }
        fn amplitude(&mut self, amp: i32, _: Option<Envelope>) {
            self.log.push(format!("amp {amp}"));
        }
        fn pitch(&mut self, envelope: Envelope, pitch1: i32, pitch2: i32) {
            self.pitch = true;
            self.log
                .push(format!("pitch {envelope:?} {pitch1} {pitch2}"));
        }
        fn start_syllable(&mut self) {
            self.log.push("syllable".into());
        }
        fn interpret(&mut self, index: usize, _: i32, _: bool) -> PhonemeData {
            let mut data = PhonemeData::default();
            data.sound_addresses[FMT] = 100 + index as i32;
            data
        }
        fn tone(&mut self, _: usize) -> Result<PhonemeData, Error> {
            Err(Error::Host)
        }
        fn spect(&mut self, index: usize, which: i32, fmt: &mut FmtParams, modulation: i32) {
            self.log.push(format!(
                "spect {index} {which} {} {modulation}",
                fmt.fmt_addr
            ));
        }
        fn sample(&mut self, _: &mut PhonemeData, _: i32, _: i32) {}
        fn set_synthflags(&mut self, _: usize, _: u16) {}
        fn set_std_length(&mut self, _: usize, _: u8) {}
    }

    fn entry(kind: u8) -> Entry {
        Entry {
            kind,
            present: 1,
            amp: 20,
            pitch1: 40,
            pitch2: 70,
            ..Entry::default()
        }
    }

    #[test]
    fn vowel_after_pause_and_suspension() {
        let mut list = [
            entry(PH_PAUSE),
            Entry {
                new_word: START_OF_WORD,
                source: 3,
                stress: 7,
                ..entry(PH_VOWEL)
            },
            Entry {
                length: 30,
                ..entry(PH_PAUSE)
            },
            entry(PH_PAUSE),
        ];
        let mut count = 3;
        let mut state = State::default();
        let settings = Settings {
            clause_start_char: 10,
            ..Settings::default()
        };
        // short of queue space at the vowel
        let mut host = Recorder {
            free: vec![5],
            ..Recorder::default()
        };
        assert_eq!(
            generate(&mut list, &mut count, false, &mut state, &settings, &mut host),
            Ok(true)
        );
        assert_eq!(host.log, ["reset", "pause 0 0"]);
        assert_eq!(
            generate(&mut list, &mut count, true, &mut state, &settings, &mut host),
            Ok(false)
        );
        assert_eq!(count, 0);
        assert_eq!(
            host.log[2..],
            [
                "break",
                "marker 1 13 0 0",
                "syllable",
                "amp 20",
                "pitch Table(0) 40 70",
                "spect 1 1 101 3",
                "spect 1 2 101 3",
                "pause 30 0",
                "end pitch true",
                "marker 5 0 0 0",
            ]
        );
    }
}
