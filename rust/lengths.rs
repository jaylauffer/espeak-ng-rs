//! Phoneme lengths, pre-pauses, amplitudes and pre-vocalic pitch.
//!
//! Ports `CalcLengths` from `setlengths.c`. Entries are a copied phoneme-list
//! span: the clause's `count` entries and any following entries the legacy
//! code may read (it looks up to four entries ahead and scans to the next
//! word start). A read past the span is an error. Embedded speed commands and
//! tone-phoneme envelopes come from the [`Host`], called in C's order.
// Copyright (C) 2005 to 2007 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::envelope::{ENVELOPES, N_ENVELOPES};

/// `N_PHONEME_LIST + 1`, the capacity of the engine's phoneme list.
pub const MAX_ENTRIES: usize = 1001;
/// Entries in each `length_mods` table (`LENGTH_MOD_LIMIT` squared).
pub const LENGTH_MODS: usize = 100;

const SEQ_CONTINUE: u16 = 0x01;
const EMBEDDED: u16 = 0x02;
const SYLLABLE: u16 = 0x04;
const LENGTHEN: u16 = 0x08;

const PH_PAUSE: u8 = 0;
const PH_VOWEL: u8 = 2;
const PH_LIQUID: u8 = 3;
const PH_STOP: u8 = 4;
const PH_VSTOP: u8 = 5;
const PH_FRICATIVE: u8 = 6;
const PH_VFRICATIVE: u8 = 7;
const PH_NASAL: u8 = 8;

const VOICELESS: u32 = 1 << 3;
const SIBILANT: u32 = 1 << 5;
const BRK_AFTER: u32 = 1 << 14;
const NON_SYLLABIC: u32 = 1 << 20;
const LENGTHEN_STOP: u32 = 1 << 22;
const NO_PAUSE: u32 = 1 << 24;

const PHON_PAUSE_VSHORT: u8 = 23;
const PHON_PAUSE_CLAUSE: u8 = 27;
const END_OF_CLAUSE: u8 = 2;

const S_EO_CLAUSE1: u32 = 0x40000;
const S_NO_EOC_LENGTHEN: u32 = 0x200000;

const PITCH_FALL: u8 = 0;
const PITCH_RISE: u8 = 2;
const SLASH_R: u32 = (b'/' as u32) * 256 + b'r' as u32;

/// One copied phoneme-list entry with the fields of its phoneme record.
/// `prepause`, `length`, `amp`, `pitch1`, `pitch2`, `env` and `synthflags`
/// are written.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Entry {
    pub length: u32,
    /// `ph->phflags`.
    pub phflags: u32,
    /// `ph->mnemonic`.
    pub mnemonic: u32,
    pub synthflags: u16,
    /// `PHONEME_LIST.type`.
    pub kind: u8,
    pub stress: u8,
    pub new_word: u8,
    pub prepause: u8,
    pub amp: u8,
    pub pitch1: u8,
    pub pitch2: u8,
    pub env: u8,
    pub tone: u8,
    /// `ph->code`.
    pub code: u8,
    /// `ph->length_mod`.
    pub length_mod: u8,
    /// `ph->std_length`.
    pub std_length: u8,
    /// Nonzero when the entry's tone phoneme exists; `tone_length` is then
    /// its `std_length`.
    pub tone_known: u8,
    pub tone_length: u8,
}

/// Translator and speed options for one clause.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Speed factors for the last, penultimate and earlier syllables of a word.
    pub len_speeds: [i32; 3],
    pub word_gap: i32,
    pub long_stop: i32,
    pub lengthen_tonic: i32,
    pub max_lengthmod: i32,
    /// `param[LOPT_MAXAMP_EOC]`.
    pub max_amp_eoc: i32,
    pub stress_flags: u32,
    pub stress_lengths: [i16; 8],
    pub stress_amps: [u8; 8],
    pub length_mods: [u8; LENGTH_MODS],
    pub length_mods0: [u8; LENGTH_MODS],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// More than [`MAX_ENTRIES`] entries, or a count past the span.
    TooLong,
    /// A read past the supplied span.
    Bounds,
    /// A length-modifier index past its 100-entry table.
    Table,
    /// The host failed an embedded command or tone envelope.
    Host,
}

/// Effects the computation needs from the engine.
pub trait Host {
    /// Applies the embedded commands at the next entry flagged with them and
    /// returns the resulting speed factors.
    fn embedded(&mut self) -> Result<[i32; 3], Error>;
    /// First byte of the pitch envelope selected by the tone phoneme program
    /// of entry `index`.
    fn tone_envelope(&mut self, index: usize) -> Result<u8, Error>;
}

fn table(mods: &[u8; LENGTH_MODS], index: usize) -> Result<i32, Error> {
    mods.get(index).map(|&v| i32::from(v)).ok_or(Error::Table)
}

/// Sets lengths for one clause (`CalcLengths`).
///
/// `count` is the clause length (`n_phoneme_list`); `entries` may extend past
/// it. `more_syllables` carries the legacy function's static across clauses
/// and is updated only on success. On error `entries` may be partly updated
/// and should be discarded. Returns the number of envelopes reset to 0 as bad
/// intonation data.
pub fn calc_lengths(
    entries: &mut [Entry],
    count: usize,
    settings: &Settings,
    more_syllables: &mut i32,
    host: &mut impl Host,
) -> Result<u32, Error> {
    let span = entries.len();
    if span > MAX_ENTRIES || count > span {
        return Err(Error::TooLong);
    }
    let at = |i: usize| if i < span { Ok(i) } else { Err(Error::Bounds) };
    let s = settings;
    let mut len_speeds = s.len_speeds;
    let mut more = *more_syllables;
    let mut pre_sonorant = false;
    let mut pre_voiced = false;
    let mut last_pitch: i32 = 0;
    let mut bad_envelopes = 0;
    let word_gap_pause = |e: &Entry| s.word_gap & 0x10 != 0 && e.new_word != 0;

    for ix in 1..count {
        let prev = ix - 1;
        let pv = entries[prev];
        let p = entries[ix];
        let stress = usize::from(p.stress & 7);
        let emphasized = p.stress & 8 != 0;

        if p.synthflags & EMBEDDED != 0 {
            len_speeds = host.embedded()?;
        }
        let kind = if p.synthflags & SYLLABLE != 0 {
            PH_VOWEL
        } else {
            p.kind
        };

        match kind {
            PH_PAUSE => last_pitch = 0,
            PH_STOP => {
                last_pitch = 0;
                let mut prepause: u8 = if pv.kind == PH_FRICATIVE {
                    25
                } else if more > 0 || stress < 4 {
                    48
                } else {
                    60
                };
                if pv.kind == PH_STOP || word_gap_pause(&p) {
                    prepause = 60;
                }
                if p.phflags & LENGTHEN_STOP != 0 {
                    prepause = prepause.wrapping_add(30);
                }
                if p.synthflags & LENGTHEN != 0 {
                    prepause = i32::from(prepause).wrapping_add(s.long_stop) as u8;
                }
                entries[ix].prepause = prepause;
            }
            PH_VFRICATIVE | PH_FRICATIVE => {
                let e = &mut entries[ix];
                if p.new_word != 0 && !(pv.kind == PH_VOWEL && p.phflags & NO_PAUSE != 0) {
                    e.prepause = 15;
                }
                let next = at(ix + 1)?;
                let nx = entries[next];
                let e = &mut entries[ix];
                if nx.kind == PH_PAUSE && pv.kind == PH_NASAL && p.phflags & VOICELESS == 0 {
                    e.prepause = 25;
                }
                if pv.phflags & BRK_AFTER != 0 || word_gap_pause(&p) {
                    e.prepause = 30;
                }
                e.length = if p.phflags & SIBILANT != 0 && nx.kind == PH_STOP && nx.new_word == 0 {
                    if pv.kind == PH_VOWEL {
                        200 // ?? should do this if it's from a prefix
                    } else {
                        150
                    }
                } else {
                    256
                };
                if kind == PH_VFRICATIVE {
                    if nx.kind == PH_VOWEL {
                        pre_voiced = true;
                    }
                    if pv.kind == PH_VOWEL || pv.kind == PH_LIQUID {
                        e.length = 255u32.wrapping_add(pv.length) / 2;
                    }
                }
            }
            PH_VSTOP => {
                let mut prepause = p.prepause;
                if matches!(pv.kind, PH_VFRICATIVE | PH_FRICATIVE | PH_LIQUID)
                    || pv.phflags & SIBILANT != 0
                {
                    prepause = 30;
                }
                let nx = entries[at(ix + 1)?];
                if nx.kind == PH_VOWEL || nx.kind == PH_LIQUID {
                    if nx.kind == PH_VOWEL || nx.new_word == 0 {
                        pre_voiced = true;
                    }
                    prepause = 40;
                    if pv.kind == PH_VOWEL {
                        prepause = 0; // murmur links from the preceding vowel
                    } else if pv.kind == PH_PAUSE {
                        // reduce by the length of the preceding pause
                        prepause = if pv.length < u32::from(prepause) {
                            prepause.wrapping_sub(pv.length as u8)
                        } else {
                            0
                        };
                    } else if p.new_word == 0 {
                        if pv.kind == PH_LIQUID {
                            prepause = 20;
                        }
                        if pv.kind == PH_NASAL {
                            prepause = 12;
                        }
                        if pv.kind == PH_STOP && pv.phflags & VOICELESS == 0 {
                            prepause = 0;
                        }
                    }
                }
                if word_gap_pause(&p) && prepause < 20 {
                    prepause = 20;
                }
                entries[ix].prepause = prepause;
            }
            PH_LIQUID | PH_NASAL => {
                let mut e = p;
                e.amp = s.stress_amps[0]; // unless changed later
                e.length = 256;
                if p.new_word != 0 {
                    if pv.kind == PH_LIQUID {
                        e.prepause = 25;
                    }
                    if pv.kind == PH_VOWEL && p.phflags & NO_PAUSE == 0 {
                        e.prepause = 12;
                    }
                }
                let nx = entries[at(ix + 1)?];
                if nx.kind == PH_VOWEL {
                    pre_sonorant = true;
                } else {
                    e.pitch2 = last_pitch as u8;
                    if pv.kind == PH_VOWEL || pv.kind == PH_LIQUID {
                        e.length = pv.length;
                        if p.kind == PH_LIQUID {
                            e.length = len_speeds[0] as u32;
                        }
                        if nx.kind == PH_VSTOP {
                            e.length = e.length.wrapping_mul(160) / 100;
                        }
                        if nx.kind == PH_VFRICATIVE {
                            e.length = e.length.wrapping_mul(120) / 100;
                        }
                    } else if let Some(vowel) =
                        entries[ix..count].iter().find(|v| v.kind == PH_VOWEL)
                    {
                        e.pitch2 = vowel.pitch2;
                    }
                    e.pitch1 = e.pitch2.saturating_sub(16);
                    e.env = PITCH_FALL;
                    pre_voiced = false;
                }
                entries[ix] = e;
            }
            PH_VOWEL => {
                let mut e = p;
                let mut min_drop = 0;
                // swap diminished and unstressed
                let stress = if stress <= 1 { stress ^ 1 } else { stress };
                e.amp = if pre_sonorant {
                    s.stress_amps[stress].wrapping_sub(1)
                } else {
                    s.stress_amps[stress]
                };
                if emphasized {
                    e.amp = 25;
                }
                if ix + 3 >= count && i32::from(e.amp) > s.max_amp_eoc {
                    // last phoneme of a clause, limit its amplitude
                    e.amp = s.max_amp_eoc as u8;
                }

                // is this the last syllable of a word?
                more = 0;
                let mut end_of_clause = false;
                let mut p2 = at(ix + 1)?;
                while entries[p2].new_word == 0 {
                    let q = &entries[p2];
                    if q.kind == PH_VOWEL && q.phflags & NON_SYLLABIC == 0 {
                        more += 1;
                    }
                    if q.code == PHON_PAUSE_CLAUSE {
                        end_of_clause = true;
                    }
                    p2 = at(p2 + 1)?;
                }
                let q = &entries[p2];
                if q.code == PHON_PAUSE_CLAUSE || (q.new_word & END_OF_CLAUSE != 0 && more == 0) {
                    end_of_clause = true;
                }

                // length modifier
                let (mut next, mut next2, mut next3) = (ix + 1, ix + 2, ix + 3);
                if entries[at(next)?].code == PHON_PAUSE_VSHORT
                    && entries[at(next2)?].kind == PH_PAUSE
                {
                    // a very short pause followed by a pause: use that
                    next = next2;
                    next2 = next3;
                    next3 = ix + 4;
                }
                let nx = entries[at(next)?];
                let n2 = entries[at(next2)?];
                let mut next2type = usize::from(n2.length_mod);
                let next_mod = usize::from(nx.length_mod);
                let mut length_mod;
                if more == 0 {
                    if (nx.new_word != 0 || n2.new_word != 0) && next2type != 1 {
                        // not the 2nd phoneme over a word boundary, unless a pause
                        next2type = 0;
                    }
                    let len = table(&s.length_mods0, next2type * 10 + next_mod)?;
                    length_mod = if nx.new_word != 0 && s.word_gap & 0x20 != 0 {
                        // a pause + first phoneme of the next word
                        (len + table(&s.length_mods0, next_mod * 10 + 1)?) / 2
                    } else {
                        len
                    };
                } else {
                    length_mod = table(&s.length_mods, next2type * 10 + next_mod)?;
                    if nx.kind == PH_NASAL
                        && (n2.kind == PH_STOP || n2.kind == PH_VSTOP)
                        && entries[at(next3)?].phflags & VOICELESS != 0
                    {
                        length_mod -= 15;
                    }
                }

                let speed = len_speeds[match more {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                }];
                length_mod = (length_mod.wrapping_mul(speed) / 128).max(8);
                if stress >= 7 {
                    // tonic: a constant part so it doesn't shrink directly with speed
                    length_mod = length_mod.wrapping_add(s.lengthen_tonic);
                    if emphasized {
                        length_mod = length_mod.wrapping_add(s.lengthen_tonic / 2);
                    }
                } else if emphasized {
                    length_mod = length_mod.wrapping_add(s.lengthen_tonic);
                }
                let len = match s.stress_lengths[stress] {
                    0 => s.stress_lengths[6],
                    len => len,
                };
                length_mod = length_mod.wrapping_mul(i32::from(len));
                if p.tone != 0 && p.tone_known != 0 && p.tone_length > 0 {
                    // a tone phoneme gives a percentage change to the length
                    length_mod = length_mod.wrapping_mul(i32::from(p.tone_length)) / 100;
                }
                if end_of_clause && s.stress_flags & S_NO_EOC_LENGTHEN == 0 {
                    // last syllable of the clause: lengthen, more for short vowels
                    let len = if s.stress_flags & S_EO_CLAUSE1 != 0 {
                        200
                    } else {
                        i32::from(p.std_length) * 2
                    };
                    length_mod = length_mod.wrapping_mul(256 + (280 - len) / 3) / 256;
                }
                let limit = s.max_lengthmod.wrapping_mul(len_speeds[0]);
                if length_mod > limit {
                    length_mod = limit;
                }
                length_mod /= 128;
                if p.kind != PH_VOWEL {
                    length_mod = 256; // syllabic consonant
                    min_drop = 16;
                }
                e.length = length_mod as u32;

                if usize::from(e.env) >= N_ENVELOPES - 1 {
                    bad_envelopes += 1;
                    e.env = 0;
                }

                // pre-vocalic part: the version after a semivowel
                let env2 = e.env + 1;
                let first = if p.tone != 0 {
                    host.tone_envelope(ix)?
                } else {
                    ENVELOPES[usize::from(env2)][0]
                };
                let (pitch1, pitch2) = (i32::from(e.pitch1), i32::from(e.pitch2));
                let pitch_start = pitch1 + (pitch2 - pitch1) * i32::from(first) / 256;
                if pre_sonorant || pre_voiced {
                    if pitch_start == 255 {
                        last_pitch = pitch_start; // pitch is not set
                    }
                    if pitch_start - last_pitch > 16 {
                        last_pitch = pitch_start - 16;
                    }
                    let pr = &mut entries[prev];
                    pr.pitch1 = last_pitch as u8;
                    pr.pitch2 = pitch_start as u8;
                    if last_pitch < pitch_start {
                        pr.env = PITCH_RISE;
                        e.env = env2;
                    } else {
                        pr.env = PITCH_FALL;
                    }
                    pr.length = length_mod as u32;
                    pr.amp = e.amp;
                    if pr.kind != PH_LIQUID && pr.amp > 18 {
                        pr.amp = 18;
                    }
                }

                // vowel and post-vocalic part
                let n2_kind = || entries[next2].kind;
                let mut flags = nx.synthflags & !SEQ_CONTINUE;
                if nx.kind == PH_NASAL && n2_kind() != PH_VOWEL {
                    flags |= SEQ_CONTINUE;
                }
                if nx.kind == PH_LIQUID {
                    flags |= SEQ_CONTINUE;
                    if n2_kind() == PH_VOWEL || nx.mnemonic == SLASH_R {
                        flags &= !SEQ_CONTINUE;
                    }
                }
                entries[next].synthflags = flags;

                if min_drop > 0 && i32::from(e.pitch2) - i32::from(e.pitch1) < min_drop {
                    e.pitch1 = (i32::from(e.pitch2) - min_drop).max(0) as u8;
                }
                let tail = i32::from(ENVELOPES[usize::from(e.env)][127]);
                last_pitch =
                    i32::from(e.pitch1) + (i32::from(e.pitch2) - i32::from(e.pitch1)) * tail / 256;
                pre_sonorant = false;
                pre_voiced = false;
                entries[ix] = e;
            }
            _ => {}
        }
    }
    *more_syllables = more;
    Ok(bad_envelopes)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed {
        speeds: [i32; 3],
        calls: usize,
    }
    impl Host for Fixed {
        fn embedded(&mut self) -> Result<[i32; 3], Error> {
            self.calls += 1;
            Ok(self.speeds)
        }
        fn tone_envelope(&mut self, _: usize) -> Result<u8, Error> {
            Err(Error::Host)
        }
    }

    fn settings() -> Settings {
        Settings {
            len_speeds: [130, 121, 118],
            word_gap: 0,
            long_stop: 100,
            lengthen_tonic: 20,
            max_lengthmod: 500,
            max_amp_eoc: 19,
            stress_flags: 0,
            stress_lengths: [182, 140, 220, 220, 220, 240, 260, 280],
            stress_amps: [18, 18, 20, 20, 20, 22, 22, 20],
            length_mods: [100; LENGTH_MODS],
            length_mods0: [100; LENGTH_MODS],
        }
    }

    fn entry(kind: u8, new_word: u8) -> Entry {
        Entry {
            kind,
            new_word,
            code: if kind == PH_PAUSE { 9 } else { 40 },
            pitch1: 40,
            pitch2: 80,
            synthflags: if kind == PH_VOWEL { SYLLABLE } else { 0 },
            ..Entry::default()
        }
    }

    fn word() -> [Entry; 7] {
        [
            entry(PH_PAUSE, 0),
            entry(PH_NASAL, 1),
            Entry {
                stress: 7,
                std_length: 140, // neutral end-of-clause lengthening
                ..entry(PH_VOWEL, 0)
            },
            entry(PH_STOP, 0),
            entry(PH_PAUSE, END_OF_CLAUSE),
            entry(PH_PAUSE, 0),
            entry(PH_PAUSE, 0),
        ]
    }

    #[test]
    fn tonic_vowel_and_onset() {
        let mut list = word();
        let mut more = 3;
        let mut host = Fixed {
            speeds: [0; 3],
            calls: 0,
        };
        assert_eq!(
            calc_lengths(&mut list, 6, &settings(), &mut more, &mut host),
            Ok(0)
        );
        assert_eq!(more, 0);
        // ((100*130/128 + 20) * 280) / 128
        assert_eq!(list[2].length, (101 + 20) * 280 / 128);
        // the nasal before the vowel takes the vowel's onset
        assert_eq!(list[1].length, list[2].length);
        assert_eq!(list[1].amp, 18);
        assert_eq!(list[3].prepause, 48);
        assert_eq!(host.calls, 0);
    }

    #[test]
    fn embedded_speed_and_errors() {
        let mut list = word();
        list[2].synthflags |= EMBEDDED;
        let mut more = 0;
        let mut host = Fixed {
            speeds: [256, 256, 256],
            calls: 0,
        };
        calc_lengths(&mut list, 6, &settings(), &mut more, &mut host).unwrap();
        assert_eq!(host.calls, 1);
        assert_eq!(list[2].length, (200 + 20) * 280 / 128);

        // a tone envelope the host can't supply, and a scan past the span
        let mut list = word();
        list[2].tone = 30;
        let mut more = 5;
        assert_eq!(
            calc_lengths(&mut list, 6, &settings(), &mut more, &mut host),
            Err(Error::Host)
        );
        assert_eq!(more, 5);
        let mut list = word();
        list[4].new_word = 0;
        list[5].new_word = 0;
        list[6].new_word = 0;
        assert_eq!(
            calc_lengths(&mut list, 6, &settings(), &mut more, &mut host),
            Err(Error::Bounds)
        );
        let mut list = word();
        list[3].length_mod = 100;
        assert_eq!(
            calc_lengths(&mut list, 6, &settings(), &mut more, &mut host),
            Err(Error::Table)
        );
    }
}
