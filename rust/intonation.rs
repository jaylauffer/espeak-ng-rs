//! Clause intonation: syllable pitch contours and tone-language sandhi.
//!
//! Ports `CalcPitches` from `intonation.c`. The phoneme list is a copied
//! [`Entry`] span; compiled tunes are borrowed little-endian `intonations`
//! records. Work happens on the stack with no allocation or I/O. On error the
//! entries are left unchanged.
// Copyright (C) 2005 to 2007 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme::{self, Phoneme};

/// `N_PHONEME_LIST + 1`, the capacity of the engine's phoneme list.
pub const MAX_ENTRIES: usize = 1001;
/// Size of one compiled `TUNE` record.
pub const TUNE_SIZE: usize = 68;
/// `OPTION_EMPHASIZE_PENULTIMATE` in the tone flags.
pub const EMPHASIZE_PENULTIMATE: u32 = 0x200;

const SYLLABLE: u16 = 0x04;
const PH_PAUSE: u8 = 0;
const PH_VOWEL: u8 = 2;
const PHON_PAUSE: u8 = 9;
const PHON_DEFAULT_TONE: u8 = 17;
const PHON_PAUSE_CLAUSE: u8 = 27;

const RISE: u8 = 1;
const EMPHASIS: u8 = 2;
const END_CLAUSE: u8 = 4;

const PITCH_FALL: u8 = 0;
const PITCH_RISE: u8 = 2;
const PITCH_FRISE: u8 = 4;
const PITCH_FRISE2: u8 = 6;

const SECONDARY: u8 = 3;
const PRIMARY: u8 = 4;
const PRIMARY_STRESSED: u8 = 6;
const PRIMARY_LAST: u8 = 7;

const fn name2(a: u8, b: u8) -> u32 {
    ((a as u32) << 8) + b as u32
}
const fn name3(a: u8, b: u8, c: u8) -> u32 {
    ((a as u32) << 16) + ((b as u32) << 8) + c as u32
}
const VI: u32 = name2(b'v', b'i');
const ZH: u32 = name2(b'z', b'h');
const CMN: u32 = name3(b'c', b'm', b'n');
const HAK: u32 = name3(b'h', b'a', b'k');

/// One copied phoneme-list entry. Only `stress`, `tone`, `env`, `pitch1`
/// and `pitch2` are written.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Entry {
    pub synthflags: u16,
    /// Phoneme type copied into the list (`PHONEME_LIST.type`).
    pub kind: u8,
    /// `ph->code` of the entry's phoneme.
    pub code: u8,
    /// `ph->std_length` of the entry's phoneme.
    pub std_length: u8,
    pub new_word: u8,
    pub stress: u8,
    pub tone: u8,
    pub env: u8,
    pub pitch1: u8,
    pub pitch2: u8,
    /// Nonzero when `tone_start`/`tone_end` describe the tone phoneme resolved
    /// while the word's own table was current (`tone_ph_data`). Otherwise the
    /// current table's `tone` record is used.
    pub tone_shape: u8,
    pub tone_start: u8,
    pub tone_end: u8,
}

/// Translator intonation options for one clause.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Settings {
    /// Packed translator name, as `L('e','n')`.
    pub translator: u32,
    /// `option_tone_flags`; only [`EMPHASIZE_PENULTIMATE`] is read.
    pub tone_flags: u32,
    pub tone_language: i32,
    pub intonation_group: i32,
    pub tunes: [u8; 6],
    pub punct_to_tone: [[u8; 6]; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// More than [`MAX_ENTRIES`] entries.
    TooLong,
    /// A syllable stress above 7 would index past the drop tables.
    Stress,
    /// Clause type outside the six punctuation intonations.
    ClauseType,
    /// Negative intonation group.
    Group,
    /// Tune number outside the compiled tunes or fixed tone tables, or a head
    /// extension read past the tune data.
    Tune,
    /// A tone phoneme dereferenced by the tone rules is absent from the table.
    Phoneme,
}

/// Borrowed compiled `intonations` data.
#[derive(Clone, Copy, Debug)]
pub struct Tunes<'a>(&'a [u8]);

impl<'a> Tunes<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
    pub fn len(&self) -> usize {
        self.0.len() / TUNE_SIZE
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn tune(&self, number: u8) -> Result<Tune<'a>, Error> {
        let number = usize::from(number);
        if number >= self.len() {
            return Err(Error::Tune);
        }
        Ok(Tune {
            bytes: self.0,
            base: number * TUNE_SIZE,
        })
    }
}

#[derive(Clone, Copy)]
struct Tune<'a> {
    bytes: &'a [u8],
    base: usize,
}

impl Tune<'_> {
    fn byte(&self, offset: usize) -> i32 {
        i32::from(self.bytes[self.base + offset])
    }
    fn signed(&self, offset: usize) -> i32 {
        i32::from(self.bytes[self.base + offset] as i8)
    }
    /// `head_extend[index]`. The legacy reader runs past the eight-entry array
    /// into following tune bytes when `n_head_extend` exceeds eight; this
    /// keeps that addressing but stops at the end of the data.
    fn head_extend(&self, index: usize) -> Result<i32, Error> {
        self.bytes
            .get(self.base + 16 + index)
            .map(|&b| i32::from(b as i8))
            .ok_or(Error::Tune)
    }
    fn prehead_start(&self) -> i32 {
        self.byte(24)
    }
    fn prehead_end(&self) -> i32 {
        self.byte(25)
    }
    fn stressed_env(&self) -> u8 {
        self.bytes[self.base + 26]
    }
    fn stressed_drop(&self) -> i32 {
        self.byte(27)
    }
    fn onset(&self) -> i32 {
        self.byte(30)
    }
    fn head_start(&self) -> i32 {
        self.byte(31)
    }
    fn head_end(&self) -> i32 {
        self.byte(32)
    }
    fn head_last(&self) -> i32 {
        self.byte(33)
    }
    fn head_max_steps(&self) -> i32 {
        self.byte(34)
    }
    fn n_head_extend(&self) -> usize {
        usize::from(self.bytes[self.base + 35])
    }
    fn unstr_start(&self, stage: usize) -> i32 {
        self.signed(36 + stage)
    }
    fn unstr_end(&self, stage: usize) -> i32 {
        self.signed(39 + stage)
    }
    /// (envelope, max, min) for the nucleus without (0) or with (1) a tail.
    fn nucleus(&self, tail: bool) -> (u8, i32, i32) {
        let at = if tail { 45 } else { 42 };
        (
            self.bytes[self.base + at],
            self.byte(at + 1),
            self.byte(at + 2),
        )
    }
    fn tail(&self) -> (i32, i32) {
        (self.byte(48), self.byte(49))
    }
}

// indexed by stress
const MIN_DROP: [i32; 8] = [6, 7, 9, 9, 20, 20, 20, 25];
// pitch change during the main part of the clause
const DROPS_0: [i32; 8] = [9, 9, 16, 16, 16, 23, 55, 32];
// overflow values are 64ths of the body pitch range
const OFLOW: [i8; 5] = [0, 40, 24, 8, 0];
const OFLOW_EMF: [i8; 5] = [10, 52, 32, 20, 10];
const OFLOW_LESS: [i8; 5] = [6, 38, 24, 14, 4];
const CONTINUE_TAB: [i8; 5] = [-26, 32, 20, 8, 0];

struct ToneHead {
    pre_start: i32,
    pre_end: i32,
    body_start: i32,
    body_end: i32,
    body_max_steps: i32,
    body_lower_u: i32,
    overflow: &'static [i8; 5],
}

const fn head(
    body_start: i32,
    body_end: i32,
    body_max_steps: i32,
    body_lower_u: i32,
    overflow: &'static [i8; 5],
) -> ToneHead {
    ToneHead {
        pre_start: 46,
        pre_end: 57,
        body_start,
        body_end,
        body_max_steps,
        body_lower_u,
        overflow,
    }
}

// All legacy heads use drops_0 and five overflow entries.
const TONE_HEADS: [ToneHead; 13] = [
    head(78, 50, 3, 7, &OFLOW),      // 0 statement
    head(78, 46, 3, 7, &OFLOW),      // 1 comma
    head(78, 46, 3, 7, &OFLOW),      // 2 question
    head(90, 50, 3, 9, &OFLOW_EMF),  // 3 exclamation
    head(78, 50, 3, 7, &OFLOW),      // 4 statement, emphatic
    head(74, 55, 4, 7, &OFLOW_LESS), // 5 statement, less intonation
    head(74, 55, 4, 7, &OFLOW_LESS), // 6 comma, less intonation
    head(74, 55, 4, 7, &OFLOW_LESS), // 7 comma, less intonation, less rise
    head(78, 50, 3, 7, &OFLOW),      // 8 pitch raises at end of sentence
    head(78, 46, 3, 7, &OFLOW),      // 9 comma
    head(78, 50, 3, 7, &OFLOW),      // 10 question
    ToneHead {
        pre_start: 34,
        pre_end: 41,
        ..head(41, 32, 3, 7, &OFLOW_LESS)
    }, // 11 test
    head(55, 50, 3, 7, &OFLOW_LESS), // 12 test
];

/// (env, max, min) without and with a tail, tail start/end and emphasis.
/// The legacy `backwards` table is null in every entry, so it is omitted.
struct ToneNucleus {
    end: (u8, i32, i32),
    tailed: (u8, i32, i32),
    tail: (i32, i32),
    emphasis: bool,
}

const fn nucleus(end: (u8, i32, i32), tailed: (u8, i32, i32), tail: (i32, i32)) -> ToneNucleus {
    ToneNucleus {
        end,
        tailed,
        tail,
        emphasis: false,
    }
}

const TONE_NUCLEI: [ToneNucleus; 13] = [
    nucleus((PITCH_FALL, 64, 8), (PITCH_FALL, 70, 18), (24, 12)), // 0 statement
    nucleus((PITCH_FRISE, 80, 18), (PITCH_FRISE2, 78, 22), (34, 52)), // 1 comma
    nucleus((PITCH_FRISE, 88, 22), (PITCH_FRISE2, 82, 22), (34, 64)), // 2 question
    ToneNucleus {
        emphasis: true,
        ..nucleus((PITCH_FALL, 92, 8), (PITCH_FALL, 92, 80), (76, 8))
    }, // 3 exclamation
    nucleus((PITCH_FALL, 86, 4), (PITCH_FALL, 94, 66), (34, 10)), // 4 statement, emphatic
    nucleus((PITCH_FALL, 62, 10), (PITCH_FALL, 62, 20), (28, 16)), // 5 statement, less intonation
    nucleus((PITCH_FRISE, 68, 18), (PITCH_FRISE2, 68, 22), (30, 44)), // 6 comma, less intonation
    nucleus((PITCH_FRISE2, 64, 16), (PITCH_FALL, 66, 32), (32, 18)), // 7 comma, less rise
    nucleus((PITCH_RISE, 68, 46), (PITCH_FALL, 42, 32), (46, 58)), // 8 pitch raises at end
    nucleus((PITCH_FRISE, 78, 24), (PITCH_FRISE2, 72, 22), (42, 52)), // 9 comma
    nucleus((PITCH_FRISE, 88, 34), (PITCH_FALL, 64, 32), (46, 82)), // 10 question
    nucleus((PITCH_FALL, 56, 12), (PITCH_FALL, 56, 20), (24, 12)), // 11 test
    nucleus((PITCH_FALL, 70, 18), (PITCH_FALL, 70, 24), (32, 20)), // 12 test
];

#[derive(Clone, Copy, Default)]
struct Syllable {
    stress: u8,
    env: u8,
    flags: u8,
    pitch1: u8,
    pitch2: u8,
}

/// A tone group computation over the syllable table.
///
/// The legacy pre-head span is `start + number_pre`, where `number_pre` is
/// the absolute `end` (not a count) when a group has no primary stress. A
/// group starting after the first syllable then writes past its end. Those
/// writes are kept: they can leave rise flags or pitches that later groups do
/// not overwrite. The table is twice the list capacity so they stay in bounds;
/// entries past the syllables start zeroed, where C has stack contents.
struct Clause {
    syl: [Syllable; 2 * MAX_ENTRIES + 2],
    number_pre: usize,
    number_tail: i32,
    tone_posn: usize,
    tone_posn2: usize,
    no_tonic: bool,
    penultimate: bool,
}

impl Clause {
    fn stress(&self, ix: usize) -> u8 {
        self.syl[ix].stress
    }

    fn set_pitch(&mut self, ix: usize, base: i32, drop: i32) {
        let pitch2 = base.max(0);
        let (drop, flags) = if drop < 0 { (-drop, RISE) } else { (drop, 0) };
        let pitch1 = (pitch2 + drop).max(0);
        let syl = &mut self.syl[ix];
        syl.pitch1 = pitch1.min(254) as u8;
        syl.pitch2 = pitch2.min(254) as u8;
        syl.flags |= flags;
    }

    fn count_pitch_vowels(&mut self, start: usize, end: usize, clause_end: usize) {
        let mut max_stress = 0;
        let mut max_posn = 0;
        let mut max_posn2 = 0;
        let mut number_pre = None;
        let mut last_primary = None;
        for ix in start..end {
            let stress = self.stress(ix);
            if stress >= max_stress {
                max_posn2 = if stress > max_stress { ix } else { max_posn };
                max_posn = ix;
                max_stress = stress;
            }
            if stress >= PRIMARY {
                number_pre.get_or_insert(ix - start);
                last_primary = Some(ix);
            }
        }
        self.number_pre = number_pre.unwrap_or(end);
        self.number_tail = end as i32 - max_posn as i32 - 1;
        self.tone_posn = max_posn;
        self.tone_posn2 = max_posn2;
        if self.no_tonic {
            self.tone_posn = end;
            self.tone_posn2 = end;
        } else if let Some(last) = last_primary {
            if end == clause_end {
                self.syl[last].stress = PRIMARY_LAST;
            }
        } else {
            // no primary stress, use the highest stress
            self.syl[self.tone_posn].stress = PRIMARY_LAST;
        }
    }

    /// Primary stresses up to the tonic syllable or the end.
    fn count_increments(&self, mut ix: usize, end: usize, min_stress: u8) -> i32 {
        let mut count = 0;
        while ix < end {
            let stress = self.stress(ix);
            ix += 1;
            if stress >= PRIMARY_LAST {
                break;
            }
            if stress >= min_stress {
                count += 1;
            }
        }
        count
    }

    /// Note the inclusive end, as in C.
    fn count_unstressed(&self, start: usize, end: usize, limit: u8) -> i32 {
        let mut ix = start;
        while ix <= end && self.stress(ix) < limit {
            ix += 1;
        }
        (ix - start) as i32
    }

    fn head_intonation(
        &mut self,
        tune: Tune<'_>,
        mut ix: usize,
        end: usize,
    ) -> Result<usize, Error> {
        let mut pitch = 0;
        let mut increment = 0;
        let mut n_steps = 0;
        let mut overflow = 0;
        let mut n_unstressed = 0;
        let mut unstressed_ix = 0;
        let mut used_onset = false;
        let mut head_final = end;
        let secondary = 2;
        let pitch_range = (tune.head_end() - tune.head_start()) * 256;
        let pitch_range_abs = pitch_range.abs();
        let mut initial = true;
        let mut stage = if tune.onset() == 255 { 1 } else { 0 };

        if tune.head_last() != 255 {
            // the last primary stress in the body
            if let Some(last) = (ix..end).rev().find(|&i| self.stress(i) >= 4) {
                head_final = last;
            }
        }

        while ix < end {
            let stress = self.stress(ix);
            if initial || stress >= 4 {
                if initial || stress == 5 {
                    initial = false;
                    overflow = 0;
                    if tune.onset() == 255 {
                        n_steps = self.count_increments(ix, head_final, 4);
                        pitch = tune.head_start() * 256;
                    } else {
                        // the onset has its own pitch, outside the increments
                        n_steps = self.count_increments(ix + 1, head_final, 4);
                        pitch = tune.onset() * 256;
                        used_onset = true;
                    }
                    n_steps = n_steps.min(tune.head_max_steps());
                    increment = if n_steps > 1 {
                        pitch_range / (n_steps - 1)
                    } else {
                        0
                    };
                } else if ix == head_final {
                    pitch = tune.head_last() * 256;
                    stage = 2;
                } else if used_onset {
                    stage = 1;
                    used_onset = false;
                    pitch = tune.head_start() * 256;
                    n_steps += 1;
                } else if n_steps > 0 {
                    pitch += increment;
                } else {
                    pitch =
                        tune.head_end() * 256 + pitch_range_abs * tune.head_extend(overflow)? / 64;
                    overflow += 1;
                    if overflow >= tune.n_head_extend() {
                        overflow = 0;
                    }
                }
                n_steps -= 1;
            }

            if stress >= PRIMARY {
                n_unstressed = self.count_unstressed(ix + 1, end, secondary);
                unstressed_ix = 0;
                self.syl[ix].stress = PRIMARY_STRESSED;
                self.syl[ix].env = tune.stressed_env();
                self.set_pitch(ix, pitch / 256, tune.stressed_drop());
            } else if stress >= secondary {
                n_unstressed = self.count_unstressed(ix + 1, end, secondary);
                unstressed_ix = 0;
                self.set_pitch(ix, pitch / 256, drop(&DROPS_0, stress));
            } else {
                let inc = if n_unstressed > 1 {
                    (tune.unstr_end(stage) - tune.unstr_start(stage)) / (n_unstressed - 1)
                } else {
                    0
                };
                let base = pitch / 256 + tune.unstr_start(stage) + inc * unstressed_ix;
                self.set_pitch(ix, base, drop(&DROPS_0, stress));
                unstressed_ix += 1;
            }
            ix += 1;
        }
        Ok(ix)
    }

    /// Pitches up to the tonic syllable, stepping at `min_stress`.
    fn pitch_segment(
        &mut self,
        mut ix: usize,
        end: usize,
        th: &ToneHead,
        min_stress: u8,
        continuing: bool,
    ) -> usize {
        let mut pitch = 0;
        let mut increment = 0;
        let mut n_steps = 0;
        let mut initial = true;
        let mut overflow = 0;
        let mut overflow_tab: &[i8; 5] = th.overflow;
        let pitch_range = (th.body_end - th.body_start) * 256;
        let pitch_range_abs = pitch_range.abs();

        if continuing {
            initial = false;
            overflow_tab = &CONTINUE_TAB;
            increment = pitch_range / (th.body_max_steps - 1);
        }

        while ix < end {
            let stress = self.stress(ix);
            if initial || stress >= min_stress {
                if initial || stress == 5 {
                    initial = false;
                    overflow = 0;
                    n_steps = self
                        .count_increments(ix, end, min_stress)
                        .min(th.body_max_steps);
                    increment = if n_steps > 1 {
                        pitch_range / (n_steps - 1)
                    } else {
                        0
                    };
                    pitch = th.body_start * 256;
                } else if n_steps > 0 {
                    pitch += increment;
                } else {
                    pitch = th.body_end * 256
                        + pitch_range_abs * i32::from(overflow_tab[overflow]) / 64;
                    overflow += 1;
                    if overflow >= overflow_tab.len() {
                        overflow = 0;
                        overflow_tab = th.overflow;
                    }
                }
                n_steps -= 1;
            }

            if stress >= PRIMARY {
                self.syl[ix].stress = PRIMARY_STRESSED;
                self.set_pitch(ix, pitch / 256, drop(&DROPS_0, stress));
            } else if stress >= SECONDARY {
                self.set_pitch(ix, pitch / 256, drop(&DROPS_0, stress));
            } else {
                // unstressed: lower after a stressed syllable. The first group
                // syllable is primary, so C never reads before the table here.
                let previous = ix.checked_sub(1).map_or(0, |p| self.stress(p));
                let lower = if previous & 0x3f >= SECONDARY {
                    th.body_lower_u
                } else {
                    0
                };
                self.set_pitch(ix, pitch / 256 - lower, drop(&DROPS_0, stress));
            }
            ix += 1;
        }
        ix
    }

    /// Linear change for the pre-head and the tail.
    fn pitch_gradient(&mut self, start: usize, end: usize, start_pitch: i32, end_pitch: i32) {
        if end <= start {
            return;
        }
        let n = (end - start) as i32;
        let mut increment = (end_pitch - start_pitch) * 256;
        if n > 1 {
            increment /= n;
        }
        let mut pitch = start_pitch * 256;
        for ix in start..end {
            let stress = self.stress(ix);
            if increment > 0 {
                self.set_pitch(ix, pitch / 256, -(increment / 256));
                pitch += increment;
            } else {
                let mut drop = (-(increment / 256)).max(MIN_DROP[usize::from(stress & 7)]);
                pitch += increment;
                drop = drop.min(18);
                self.set_pitch(ix, pitch / 256, drop);
            }
        }
    }

    /// One tone group over `[start, end)`, using either a compiled tune
    /// (`control == 0`) or the fixed head/nucleus tables.
    fn group(
        &mut self,
        tunes: Tunes<'_>,
        control: usize,
        start: usize,
        end: usize,
        tune_number: u8,
    ) -> Result<(), Error> {
        enum Shape<'a> {
            Tune(Tune<'a>),
            Fixed(&'static ToneHead, &'static ToneNucleus),
        }
        let shape = if control == 0 {
            Shape::Tune(tunes.tune(tune_number)?)
        } else {
            let n = usize::from(tune_number);
            match (TONE_HEADS.get(n), TONE_NUCLEI.get(n)) {
                (Some(th), Some(tn)) => Shape::Fixed(th, tn),
                _ => return Err(Error::Tune),
            }
        };

        // vowels before the first primary stress
        let mut ix = start;
        let (pre_start, pre_end) = match &shape {
            Shape::Tune(t) => (t.prehead_start(), t.prehead_end()),
            Shape::Fixed(th, _) => (th.pre_start, th.pre_end),
        };
        self.pitch_gradient(ix, ix + self.number_pre, pre_start, pre_end);
        ix += self.number_pre;

        // body of the tonic segment
        if self.penultimate {
            self.tone_posn = self.tone_posn2;
        }
        let tone_posn = self.tone_posn;
        ix = match &shape {
            Shape::Tune(t) => self.head_intonation(*t, ix, tone_posn)?,
            Shape::Fixed(th, _) => self.pitch_segment(ix, tone_posn, th, PRIMARY, start > 0),
        };
        if self.no_tonic {
            return Ok(());
        }

        // tonic syllable
        let tail = self.number_tail != 0;
        let ((env, max, min), (tail_start, tail_end)) = match &shape {
            Shape::Tune(t) => (t.nucleus(tail), t.tail()),
            Shape::Fixed(_, tn) => {
                if tn.emphasis {
                    self.syl[ix].flags |= EMPHASIS;
                }
                (if tail { tn.tailed } else { tn.end }, tn.tail)
            }
        };
        self.set_pitch(ix, min, max - min);
        ix += 1;
        self.syl[tone_posn].env = env;
        if self.syl[tone_posn].stress == PRIMARY {
            self.syl[tone_posn].stress = PRIMARY_STRESSED;
        }

        // tail, after the tonic syllable
        self.pitch_gradient(ix, end, tail_start, tail_end);
        Ok(())
    }
}

/// Stress is validated to at most 7 before any group is computed and is only
/// ever rewritten to 3, 5, 6 or 7; the mask keeps indexing total.
fn drop(table: &[i32; 8], stress: u8) -> i32 {
    table[usize::from(stress & 7)]
}

fn record<'a>(phonemes: &[Option<&'a Phoneme>], code: u8) -> Option<&'a Phoneme> {
    phonemes.get(usize::from(code)).copied().flatten()
}

fn mnemonic(record: Option<&Phoneme>) -> Result<u32, Error> {
    record.map(|r| r.mnemonic).ok_or(Error::Phoneme)
}

/// Sets pitches, envelopes and emphasis for one clause (`CalcPitches`).
///
/// `phonemes` is the current phoneme table. `clause_type` is 0 `.`, 1 `,`,
/// 2 `?`, 3 `!` or 4 none (an incomplete clause with no tonic).
pub fn calc_pitches(
    entries: &mut [Entry],
    phonemes: &[Option<&Phoneme>],
    tunes: Tunes<'_>,
    settings: &Settings,
    clause_type: i32,
) -> Result<(), Error> {
    let n = entries.len();
    if n > MAX_ENTRIES {
        return Err(Error::TooLong);
    }
    let mut c = Clause {
        syl: [Syllable::default(); 2 * MAX_ENTRIES + 2],
        number_pre: 0,
        number_tail: 0,
        tone_posn: 0,
        tone_posn2: 0,
        no_tonic: clause_type == 4,
        penultimate: settings.tone_flags & EMPHASIZE_PENULTIMATE != 0,
    };
    let mut n_st = 0;
    let mut n_primary = 0;
    // The last entry is excluded, as in C. Flags start zeroed; the legacy
    // per-entry clear never reaches a syllable already marked.
    for p in &entries[..n.saturating_sub(1)] {
        if p.synthflags & SYLLABLE != 0 {
            c.syl[n_st].env = PITCH_FALL;
            c.syl[n_st].stress = p.stress;
            n_st += 1;
            if p.stress >= 4 {
                n_primary += 1;
            }
        } else if p.code == PHON_PAUSE_CLAUSE && n_st > 0 {
            c.syl[n_st - 1].flags |= END_CLAUSE;
        }
    }
    c.syl[n_st].stress = 0; // extra 0 entry at the end
    if n_st == 0 {
        return Ok(());
    }
    if settings.tone_language == 1 {
        return calc_tones(entries, phonemes, settings);
    }
    if c.syl[..n_st].iter().any(|s| s.stress > 7) {
        return Err(Error::Stress);
    }

    let option = match settings.intonation_group {
        g if g < 0 => return Err(Error::Group),
        g if g >= 8 => 1,
        g => g as usize,
    };
    let clause = usize::try_from(clause_type)
        .ok()
        .filter(|&t| t < 6)
        .ok_or(Error::ClauseType)?;
    let (mut group_tone, group_tone_comma) = if option == 0 {
        (settings.tunes[clause], settings.tunes[1])
    } else {
        (
            settings.punct_to_tone[option][clause],
            settings.punct_to_tone[option][1],
        )
    };

    let mut st_start = 0;
    let mut count_primary = 0;
    for st_ix in 0..n_st {
        if c.stress(st_ix) >= 4 {
            count_primary += 1;
        }
        if c.stress(st_ix) == 6 {
            // reduce the previous stressed syllable, within the last three
            let floor = st_start.max(st_ix.saturating_sub(3));
            for ix in (floor..st_ix).rev() {
                match c.stress(ix) {
                    6 => break,
                    4 => {
                        c.syl[ix].stress = 3;
                        break;
                    }
                    _ => {}
                }
            }
            // are the next primary syllables also emphasized?
            for ix in st_ix + 1..n_st {
                match c.stress(ix) {
                    4 => break,
                    6 => {
                        // emphasize, but don't end the current tone group
                        c.syl[st_ix].flags = EMPHASIS;
                        c.syl[st_ix].stress = 5;
                        break;
                    }
                    _ => {}
                }
            }
        }

        if c.stress(st_ix) == 6 {
            // end the tone group after the next primary stress
            c.syl[st_ix].flags = EMPHASIS;
            let mut count = i32::from(n_primary - count_primary > 1);
            let mut ix = st_ix + 1;
            while ix < n_st {
                let stress = c.stress(ix);
                if stress > 4 {
                    break;
                }
                if stress == 4 {
                    count += 1;
                    if count > 1 {
                        break;
                    }
                }
                ix += 1;
            }
            c.count_pitch_vowels(st_start, ix, n_st);
            c.group(tunes, option, st_start, ix, group_tone)?;
            if ix < n_st || clause_type == 0 {
                // , or ? remainder has comma tone; . or ! statement tone
                group_tone = if clause_type == 1 || clause_type == 2 {
                    settings.tunes[1]
                } else {
                    settings.tunes[0]
                };
            }
            st_start = ix;
        }
        if st_start < st_ix && c.syl[st_ix].flags & END_CLAUSE != 0 {
            // end of clause after this syllable (phonPAUSE_CLAUSE)
            let clause_end = st_ix + 1;
            c.count_pitch_vowels(st_start, clause_end, clause_end);
            c.group(tunes, option, st_start, clause_end, group_tone_comma)?;
            st_start = clause_end;
        }
    }
    if st_start < n_st {
        c.count_pitch_vowels(st_start, n_st, n_st);
        c.group(tunes, option, st_start, n_st, group_tone)?;
    }

    // unpack pitch data; non-syllables take the next syllable's stress
    let mut st_ix = 0;
    for p in entries.iter_mut() {
        let syl = c.syl[st_ix];
        p.stress = syl.stress;
        if p.synthflags & SYLLABLE == 0 {
            continue;
        }
        p.pitch1 = syl.pitch1;
        p.pitch2 = syl.pitch2;
        p.env = if syl.flags & RISE != 0 {
            PITCH_RISE
        } else if p.stress > 5 {
            syl.env
        } else {
            PITCH_FALL
        };
        if p.pitch1 > p.pitch2 {
            // pitch2 is the higher
            std::mem::swap(&mut p.pitch1, &mut p.pitch2);
        }
        if p.tone != 0 {
            let shape = if p.tone_shape != 0 {
                Some((p.tone_start, p.tone_end))
            } else {
                record(phonemes, p.tone).map(|r| (r.start_type, r.end_type))
            };
            if let Some((start, end)) = shape {
                let x = (i32::from(p.pitch1) + i32::from(p.pitch2)) / 2;
                p.pitch2 = (x + i32::from(end)) as u8;
                p.pitch1 = (x + i32::from(start)) as u8;
            }
        }
        if syl.flags & EMPHASIS != 0 {
            p.stress |= 8;
        }
        st_ix += 1;
    }
    Ok(())
}

/// Tone languages: sandhi, then tone numbers to pitches (`CalcPitches_Tone`).
/// Runs on a copy so a missing tone phoneme leaves the entries unchanged.
fn calc_tones(
    entries: &mut [Entry],
    phonemes: &[Option<&Phoneme>],
    settings: &Settings,
) -> Result<(), Error> {
    let mut work = [Entry::default(); MAX_ENTRIES];
    let work = &mut work[..entries.len()];
    work.copy_from_slice(entries);
    let code = |mnemonic: u32| phoneme::code(phonemes.iter().copied(), mnemonic);
    let pause = record(phonemes, PHON_PAUSE);
    let syllable = |p: &Entry| p.synthflags & SYLLABLE != 0;

    let final_stressed = work
        .iter()
        .rposition(|p| p.kind == PH_VOWEL && p.stress >= 4)
        .unwrap_or(0);
    work[final_stressed].stress = 7;

    if settings.translator == VI && work[final_stressed].tone == 0 {
        // default tone (1) falls at the end of the clause
        work[final_stressed].tone = code(u32::from(b'7'));
    }

    let mandarin = settings.translator == ZH || settings.translator == CMN;
    let mut after_pause = true;
    let mut tone_promoted = false;
    let mut prev_p = 0;
    let mut prev_tph = pause; // forgotten across word boundaries
    let mut prevw_tph = pause; // remembered across word boundaries

    // tone sandhi
    for ix in 0..work.len() {
        let p = work[ix];
        if p.kind == PH_PAUSE && p.std_length > 50 {
            after_pause = true;
            prevw_tph = pause;
        }
        if p.new_word != 0 {
            prev_tph = pause;
        }
        if !syllable(&p) {
            continue;
        }
        let mut tone_ph = p.tone;
        let mut tph = record(phonemes, tone_ph);

        // Hakka: tone 1 before tones 1, 4 or 6 becomes tone 2
        if settings.translator == HAK
            && mnemonic(prev_tph)? == 0x31
            && matches!(mnemonic(tph)?, 0x31 | 0x34 | 0x36)
        {
            work[prev_p].tone = code(u32::from(b'2'));
        }

        if mandarin {
            if tone_ph == 0 {
                if after_pause || tone_promoted {
                    tone_ph = code(0x3535); // no previous vowel, use tone 1
                    tone_promoted = true;
                } else {
                    tone_ph = code(0x3131); // default tone 5
                }
                work[ix].tone = tone_ph;
                tph = record(phonemes, tone_ph);
            } else {
                tone_promoted = false;
            }

            if ix == final_stressed && matches!(mnemonic(tph)?, 0x3535 | 0x3135) {
                // sentence-final tone 1 or 4 takes stress 6, not 7
                work[final_stressed].stress = 6;
            }
            if mnemonic(prevw_tph)? == 0x343132 {
                // [214] before [214] becomes [35], otherwise [21]
                work[prev_p].tone = if mnemonic(tph)? == 0x343132 {
                    code(0x3533)
                } else {
                    code(0x3132)
                };
            }
            if mnemonic(prev_tph)? == 0x3135 && mnemonic(tph)? == 0x3135 {
                work[prev_p].tone = code(0x3335); // [51] + [51]
            }
            if mnemonic(tph)? == 0x3131 {
                // tone 5 level follows the previous tone, across words
                let previous = mnemonic(prevw_tph)?;
                if previous == 0x3535 {
                    work[ix].tone = code(0x3232);
                }
                if previous == 0x3533 {
                    work[ix].tone = code(0x3333);
                }
                if previous == 0x343132 {
                    work[ix].tone = code(0x3434);
                }
                work[ix].stress = 0; // tone 5 is unstressed (shorter)
            }
        }

        prev_p = ix;
        prev_tph = tph;
        prevw_tph = tph;
        after_pause = false;
    }

    // tone numbers to pitch. The legacy clause gradient (adjust, decrement,
    // low and high) is all zeros, so the tone's own levels are used.
    for p in work.iter_mut().filter(|p| syllable(p)) {
        if p.tone == 0 {
            p.tone = PHON_DEFAULT_TONE; // no tone specified, use tone 1
        }
        let tone = record(phonemes, p.tone).ok_or(Error::Phoneme)?;
        p.pitch1 = tone.start_type;
        p.pitch2 = tone.end_type;
    }
    entries.copy_from_slice(work);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tune_bytes() -> [u8; TUNE_SIZE] {
        let mut t = [0u8; TUNE_SIZE];
        t[16..19].copy_from_slice(&[20u8, (-10i8) as u8, 5]);
        t[24..36].copy_from_slice(&[40, 50, 3, 12, 8, 0, 255, 80, 60, 255, 4, 3]);
        t[36..42].copy_from_slice(&[0, 5, 0, (-5i8) as u8, 0, 0]);
        t[42..50].copy_from_slice(&[0, 70, 20, 4, 60, 30, 30, 10]);
        t
    }

    fn entry(syllable: bool, stress: u8) -> Entry {
        Entry {
            synthflags: if syllable { SYLLABLE } else { 0 },
            kind: if syllable { PH_VOWEL } else { 0 },
            code: 30,
            stress,
            ..Entry::default()
        }
    }

    #[test]
    fn statement_falls_to_the_tonic() {
        let tune = tune_bytes();
        let mut list = [
            entry(false, 0),
            entry(true, 1),
            entry(false, 4),
            entry(true, 4),
            entry(true, 1),
            entry(true, 4),
            entry(true, 2),
            entry(false, 0),
        ];
        let settings = Settings::default();
        calc_pitches(&mut list, &[], Tunes::new(&tune), &settings, 0).unwrap();
        // consonants take the following syllable's stress
        assert_eq!(list[0].stress, 1);
        assert_eq!(list[2].stress, 6);
        // tonic is the last primary, with a tail
        assert_eq!(list[5].stress, 7);
        assert_eq!(list[5].env, 4);
        assert_eq!((list[5].pitch1, list[5].pitch2), (30, 60));
        assert!(list[3].pitch2 > list[6].pitch2);
        assert_eq!(list[7].stress, 0);
    }

    #[test]
    fn invalid_input_leaves_entries_unchanged() {
        let tune = tune_bytes();
        let mut list = [entry(true, 9), entry(false, 0)];
        let before = list;
        let settings = Settings::default();
        assert_eq!(
            calc_pitches(&mut list, &[], Tunes::new(&tune), &settings, 0),
            Err(Error::Stress)
        );
        assert_eq!(list, before);
        list[0].stress = 4;
        let before = list;
        assert_eq!(
            calc_pitches(&mut list, &[], Tunes::new(&tune), &settings, 6),
            Err(Error::ClauseType)
        );
        let tunes = Settings {
            tunes: [1; 6],
            ..settings
        };
        assert_eq!(
            calc_pitches(&mut list, &[], Tunes::new(&tune), &tunes, 0),
            Err(Error::Tune)
        );
        assert_eq!(list, before);
    }

    #[test]
    fn fixed_tables_emphasize_exclamations() {
        let mut list = [
            entry(true, 4),
            entry(true, 1),
            entry(true, 4),
            entry(false, 0),
        ];
        let settings = Settings {
            intonation_group: 1,
            punct_to_tone: [[3; 6]; 8],
            ..Settings::default()
        };
        calc_pitches(&mut list, &[], Tunes::new(&[]), &settings, 3).unwrap();
        assert_eq!(list[2].stress, 7 | 8);
        assert_eq!((list[2].pitch1, list[2].pitch2), (8, 92));
        assert_eq!(list[0].stress, 6);
    }

    #[test]
    fn mandarin_third_tone_sandhi() {
        let mut records = [Phoneme::default(); 32];
        let names: [(u8, u32, u8, u8); 6] = [
            (9, u32::from(b'_'), 0, 0),
            (17, 0x3535, 50, 50),
            (20, 0x343132, 20, 10),
            (21, 0x3533, 30, 50),
            (22, 0x3131, 25, 25),
            (23, 0x3132, 20, 10),
        ];
        for (code, mnemonic, start, end) in names {
            records[usize::from(code)] = Phoneme {
                mnemonic,
                code,
                start_type: start,
                end_type: end,
                ..Phoneme::default()
            };
        }
        let table: Vec<_> = records
            .iter()
            .map(|r| (r.mnemonic != 0).then_some(r))
            .collect();
        let tone = |t| Entry {
            tone: t,
            ..entry(true, 4)
        };
        let mut list = [tone(20), tone(20), entry(false, 0)];
        let settings = Settings {
            translator: CMN,
            tone_language: 1,
            ..Settings::default()
        };
        calc_pitches(&mut list, &table, Tunes::new(&[]), &settings, 0).unwrap();
        assert_eq!(list[0].tone, 21);
        assert_eq!((list[0].pitch1, list[0].pitch2), (30, 50));
        assert_eq!(list[1].stress, 7);

        // a missing tone record is an error and changes nothing
        let mut list = [tone(31), entry(false, 0)];
        let before = list;
        assert_eq!(
            calc_pitches(&mut list, &table, Tunes::new(&[]), &settings, 0),
            Err(Error::Phoneme)
        );
        assert_eq!(list, before);
    }
}
