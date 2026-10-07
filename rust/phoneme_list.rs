//! Clause phoneme list construction (`MakePhonemeList`).
//!
//! Turns the translator's first-stage list (`PHONEME_LIST2`) into the
//! synthesis list: last-word stress promotion, removal of redundant table
//! switches, regressive voicing, voice phoneme replacements, per-word stress,
//! then each phoneme's program (change, insert, append and next-phoneme
//! replacement), unstressed-syllable reduction, word-boundary pauses and the
//! two terminating pauses. The working list is owned here and phoneme programs
//! run on the native VM against it. The host only selects phoneme tables.
//!
//! Output entries name their phoneme as a slot in the table that was current
//! when C looked it up, so the host can resolve the same record pointers.
// Copyright (C) 2005 to 2014 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme::Phoneme,
    phoneme_context::{self, Context, Position, Storage},
    phoneme_program::{PhonemeData, Program},
};

/// `N_PHONEME_LIST`: first-stage and working list capacity.
pub const N_LIST: usize = 1000;
/// `N_PHONEME_LIST + 1`: the synthesis list capacity.
pub const MAX_OUTPUT: usize = N_LIST + 1;

const EMBEDDED: u16 = 0x02;
const SYLLABLE: u16 = 0x04;
const LENGTHEN: u16 = 0x08;
const SWITCHED_LANG: u16 = 0x20;
const PROMOTE_STRESS: u16 = 0x40;

const PH_PAUSE: u8 = 0;
const PH_VOWEL: u8 = 2;
const PH_LIQUID: u8 = 3;
const PH_VSTOP: u8 = 5;
const PH_FRICATIVE: u8 = 6;
const PH_VFRICATIVE: u8 = 7;
const PH_NASAL: u8 = 8;

const UNSTRESSED: u32 = 1 << 1;
const PREVOICE: u32 = 1 << 25;

const PHON_PAUSE: u8 = 9;
const PHON_PAUSE_SHORT: u8 = 10;
const PHON_PAUSE_NOLINK: u8 = 11;
const PHON_GLOTTALSTOP: u8 = 19;
const PHON_SWITCH: u8 = 21;
const PHON_PAUSE_VSHORT: u8 = 23;
const PHON_PAUSE_LONG: u8 = 24;
const PAUSE_PHONEMES: [u8; 8] = [
    0,
    PHON_PAUSE_VSHORT,
    PHON_PAUSE_SHORT,
    PHON_PAUSE,
    PHON_PAUSE_LONG,
    PHON_GLOTTALSTOP,
    PHON_PAUSE_LONG,
    PHON_PAUSE_LONG,
];

const S_NO_DIM: u32 = 0x02;
const START_OF_WORD: u8 = 1;
const END_OF_CLAUSE: u8 = 2;
const START_OF_SENTENCE: u8 = 4;
const START_OF_CLAUSE: u8 = 8;
const PITCH_FALL: u8 = 0;

// phoneme program parameters
const CHANGE_PHONEME: usize = 1;
const CHANGE_NEXT_PHONEME: usize = 2;
const INSERT_PHONEME: usize = 3;
const APPEND_PHONEME: usize = 4;
const SET_LENGTH: usize = 10;

/// A first-stage entry; the layout of `PHONEME_LIST2`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Source {
    pub synthflags: u16,
    pub code: u8,
    pub stress: u8,
    pub source: u16,
    pub word_stress: u8,
    pub tone: u8,
}

/// A voice phoneme replacement; the layout of `REPLACE_PHONEMES`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Replacement {
    pub old: u8,
    pub new: u8,
    /// 1 only at word end, 2 not in stressed syllables, 4 only at word start.
    pub kind: i8,
}

/// One synthesis-list entry. The two terminating pauses set only `new_word`,
/// `code`, `kind`, `length`, `source`, `synthflags`, `prepause` and the
/// phoneme; the host leaves their other fields as they were.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Output {
    pub length: u32,
    /// The phoneme record is slot `slot` of this table (`ph`).
    pub table: i32,
    /// `tone_ph_data` is slot `tone` of this table, or none when negative.
    pub tone_table: i32,
    pub source: u16,
    pub synthflags: u16,
    pub slot: u8,
    /// `phcode`, the record's code.
    pub code: u8,
    pub kind: u8,
    pub stress: u8,
    pub word_stress: u8,
    pub tone: u8,
    pub new_word: u8,
    pub env: u8,
    pub prepause: u8,
    pub amp: u8,
    pub pitch1: u8,
    pub pitch2: u8,
}

/// Translator, voice and global options for one clause.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// The translator's phoneme table (`tr->phoneme_tab_ix`).
    pub table: i32,
    /// `param[LOPT_REGRESSIVE_VOICING]`.
    pub regression: i32,
    /// `param[LOPT_REDUCE]`, for phoneme programs.
    pub reduction: i32,
    pub stress_flags: u32,
    pub vowel_pause: i32,
    pub word_gap: i32,
    /// The global `option_wordgap`.
    pub option_wordgap: i32,
    pub klatt: bool,
    pub mbrola: bool,
    pub post_pause: i32,
    pub start_sentence: bool,
    pub replacements: &'a [Replacement],
    /// Compiled phoneme programs (`phonindex`).
    pub programs: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A first-stage or working-list read past the supplied entries.
    Bounds,
    /// A phoneme C dereferences is absent from the current table.
    Phoneme,
    /// The synthesis list would exceed [`MAX_OUTPUT`] entries.
    Capacity,
    /// The host could not select a table.
    Host,
}

pub type Table = [Option<Phoneme>; 256];

/// Effects the computation needs from the engine.
pub trait Host {
    /// Makes phoneme table `index` current and copies its 256 slots.
    fn select(&mut self, index: i32, table: &mut Table) -> Result<(), Error>;
    /// Reports an invalid phoneme-program instruction.
    fn invalid_instruction(&mut self, _phoneme: &Phoneme, _instruction: u16) {}
}

/// A working-list entry (`ph_list3`), with the fields this module and the
/// phoneme programs use.
#[derive(Clone, Copy, Debug, Default)]
struct Item {
    phoneme: Option<Phoneme>,
    flags: u16,
    code: u8,
    stress: u8,
    source: u16,
    word_stress: u8,
    tone: u8,
    kind: u8,
}

impl Item {
    fn entry(&self) -> phoneme_context::Entry {
        phoneme_context::Entry {
            phoneme: self.phoneme,
            code: self.code,
            stress: self.stress,
            word_stress: self.word_stress,
            source: self.source,
            flags: self.flags,
        }
    }
}

/// The current phoneme table and its index.
struct Tables<'h, H> {
    host: &'h mut H,
    current: Option<i32>,
    table: Table,
}

impl<H: Host> Tables<'_, H> {
    fn select(&mut self, index: i32) -> Result<(), Error> {
        if self.current != Some(index) {
            self.host.select(index, &mut self.table)?;
            self.current = Some(index);
        }
        Ok(())
    }
    fn get(&self, code: u8) -> Option<Phoneme> {
        self.table[usize::from(code)]
    }
    fn index(&self) -> i32 {
        self.current.unwrap_or(-1)
    }
}

/// The phoneme-program view of the working list.
struct ClauseStorage<'a, H> {
    list: &'a mut [Item],
    table: &'a Table,
    previous_vowel: &'a mut Item,
    phoneme: Phoneme,
    host: &'a mut H,
}

impl<H: Host> Storage for ClauseStorage<'_, H> {
    fn entry(&self, position: Position) -> Option<phoneme_context::Entry> {
        match position {
            Position::List(index) => self.list.get(index).map(Item::entry),
            Position::PreviousVowel => self
                .previous_vowel
                .phoneme
                .map(|_| self.previous_vowel.entry()),
        }
    }
    fn phoneme(&self, code: u8) -> Option<Phoneme> {
        self.table[usize::from(code)]
    }
    fn refresh(&mut self, position: Position) {
        let item = match position {
            Position::List(index) => self.list.get_mut(index),
            Position::PreviousVowel => Some(&mut *self.previous_vowel),
        };
        if let Some(item) = item {
            item.phoneme = self.table[usize::from(item.code)];
        }
    }
    fn invalid_instruction(&mut self, instruction: u16) {
        self.host.invalid_instruction(&self.phoneme, instruction);
    }
}

struct Clause<'o, 'h, H> {
    items: [Item; N_LIST],
    /// Initialized working entries, the phoneme-program list length.
    count: usize,
    previous_vowel: Item,
    tables: Tables<'h, H>,
    options: &'o Options<'o>,
}

impl<H: Host> Clause<'_, '_, H> {
    fn item(&self, index: usize) -> Result<Item, Error> {
        if index < self.count {
            Ok(self.items[index])
        } else {
            Err(Error::Bounds)
        }
    }

    /// `InterpretPhonemeWithLength(tr, 0x100, ...)` with word data.
    fn interpret(&mut self, index: usize) -> PhonemeData {
        let item = self.items[index];
        if item.source != 0 {
            self.previous_vowel.phoneme = None;
        }
        let mut data = PhonemeData::default();
        let Some(phoneme) = item.phoneme else {
            return data;
        };
        data.parameters[SET_LENGTH] = i32::from(phoneme.standard_length);
        data.parameters[9] = i32::from(phoneme.length_modifier);
        if phoneme.program == 0 {
            return data;
        }
        let settings = phoneme_context::Settings {
            length: self.count,
            current: index,
            control: 0x100,
            has_translator: 1,
            reduction: self.options.reduction,
            klatt: u32::from(self.options.klatt),
            mbrola: u32::from(self.options.mbrola),
        };
        let storage = ClauseStorage {
            list: &mut self.items[..self.count],
            table: &self.tables.table,
            previous_vowel: &mut self.previous_vowel,
            phoneme,
            host: &mut *self.tables.host,
        };
        let result = Program::new(self.options.programs).and_then(|program| {
            let mut context = Context::new(program, settings, storage)?;
            program.interpret(&phoneme, 0x100, true, &mut context)
        });
        if let Ok(result) = result {
            if self.items[index].kind == PH_VOWEL {
                self.previous_vowel = self.items[index];
            }
            data = result;
        }
        data
    }

    /// `ReInterpretPhoneme` after a program changed this entry's phoneme.
    fn reinterpret(
        &mut self,
        index: usize,
        phoneme: Option<Phoneme>,
        before: Phoneme,
    ) -> Result<PhonemeData, Error> {
        let phoneme = phoneme.ok_or(Error::Phoneme)?;
        let item = &mut self.items[index];
        if phoneme.kind == PH_VOWEL {
            item.flags |= SYLLABLE;
            if before.kind != PH_VOWEL {
                item.stress = 0; // a non-vowel became a vowel: unstressed
            }
        } else {
            item.flags &= !SYLLABLE;
        }
        Ok(self.interpret(index))
    }
}

fn source_at(list: &[Source], index: usize) -> Result<Source, Error> {
    list.get(index).copied().ok_or(Error::Bounds)
}

fn parameter(data: &PhonemeData, index: usize) -> Option<u8> {
    // Program parameters are 8-bit phoneme codes.
    let value = data.parameters[index];
    (value > 0).then_some(value as u8)
}

/// Builds the synthesis list for one clause (`MakePhonemeList`).
///
/// `source` holds the first stage list; `count` of its entries are the clause
/// (ending in two pauses) and any further entries are readable lookahead. It
/// is updated in place, and `count` reduced by deleted table switches, as in
/// C. Returns the number of `output` entries, the last two being the
/// terminating pauses. On error `source` and `output` may be partly updated
/// and should be discarded.
pub fn make_phoneme_list(
    source: &mut [Source],
    count: &mut usize,
    output: &mut [Output],
    options: &Options<'_>,
    host: &mut impl Host,
) -> Result<usize, Error> {
    let n2 = *count;
    if n2 == 0 || n2 > source.len().min(N_LIST) {
        return Err(Error::Bounds);
    }
    let end_source = source[n2 - 1].source;

    // is the last word of the clause unstressed?
    let mut max_stress = 0;
    let mut j = n2 as isize - 3;
    while j >= 0 {
        let s = &source[j as usize];
        max_stress = max_stress.max(s.stress & 0x7f);
        if s.source != 0 {
            break;
        }
        j -= 1;
    }
    if max_stress < 4 {
        // look for a previous word that can be stressed
        loop {
            j -= 1;
            if j < 0 {
                break;
            }
            let s = &mut source[j as usize];
            if s.synthflags & PROMOTE_STRESS != 0 {
                s.stress = 4; // dictionary flags allow promotion
                break;
            }
            if s.stress >= 4 {
                break;
            }
        }
    }

    // remove switches to the current table and switches followed by another
    let mut deleted_count = 0;
    let mut current = options.table;
    let mut deleted_source: Option<u16> = None;
    for j in 0..n2 {
        if current != options.table {
            source[j].synthflags |= SWITCHED_LANG;
        }
        if deleted_count > 0 {
            source[j - deleted_count] = source[j];
            if let Some(s) = deleted_source.take() {
                source[j - deleted_count].source = s;
            }
        }
        let s = source[j];
        if s.code == PHON_SWITCH {
            let redundant = s.synthflags & EMBEDDED == 0
                && (i32::from(s.tone) == current
                    || source_at(source, j + 1)?.code == PHON_SWITCH
                    || (source_at(source, j + 1)?.code == PHON_PAUSE
                        && source_at(source, j + 2)?.code == PHON_SWITCH));
            if redundant {
                if deleted_source.is_none() && s.source != 0 {
                    deleted_source = Some(s.source);
                }
                deleted_count += 1;
            } else {
                current = i32::from(s.tone);
            }
        }
    }
    let n2 = n2 - deleted_count;
    *count = n2;

    let mut tables = Tables {
        host,
        current: None,
        table: [None; 256],
    };
    tables.select(current)?;
    if options.regression != 0 {
        regressive_voicing(&mut source[..n2], options, &mut tables)?;
    }

    tables.select(options.table)?;
    let mut clause = Clause {
        items: [Item::default(); N_LIST],
        count: 0,
        previous_vowel: Item::default(),
        tables,
        options,
    };
    clause.count = substitute(&mut source[..n2], &mut clause)?;
    let n3 = clause.count as isize - 2;

    // the highest stress in each word
    let mut word_stress = 0;
    let mut j = 0;
    while (j as isize) < n3 {
        if clause.items[j].source != 0 {
            word_stress = 0;
            let mut next_word = j;
            while (next_word as isize) < n3 {
                word_stress = word_stress.max(clause.items[next_word].stress);
                next_word += 1;
                if clause.item(next_word)?.source != 0 {
                    break; // start of the next word
                }
            }
            for item in &mut clause.items[j..next_word] {
                item.word_stress = word_stress;
            }
            j = next_word;
        } else {
            j += 1;
        }
    }

    // the table left by substitution resolves the leading pause
    clause.items[0].phoneme = clause.tables.get(PHON_PAUSE);
    clause.tables.select(options.table)?;

    let mut ix = 0;
    let mut insert_ph: u8 = 0;
    let mut unstress_count = 0;
    let mut word_start = 1;
    let mut inserted_at: Option<usize> = None;
    let mut start_of_clause = true;
    let mut start_sentence = options.start_sentence;
    let mut j = 0usize;
    while insert_ph != 0 || ((j as isize) < n3 && ix < N_LIST - 3) {
        let mut inserted = false;
        let mut deleted = false;
        let mut ph: Option<Phoneme>;
        // where C looked `ph` up: the table current then, and the slot
        let mut ph_table = clause.tables.index();
        let mut ph_slot: u8;
        let mut next: Option<Phoneme>;
        if insert_ph != 0 {
            // insert a (linking) phoneme here, re-using the previous entry
            next = clause.tables.get(clause.item(j)?.code); // the phoneme after the insert
            j -= 1;
            inserted_at = Some(j);
            if j > 0 {
                // move the word's earlier phonemes back one place
                let first = if word_start > 0 {
                    word_start -= 1;
                    word_start + 1
                } else {
                    2 // no more space; keep the start-of-word mark
                };
                for k in first..=j {
                    clause.items[k - 1] = clause.items[k];
                }
            }
            ph = clause.tables.get(insert_ph);
            ph_slot = insert_ph;
            clause.items[j] = Item {
                code: insert_ph,
                phoneme: ph,
                ..Item::default()
            };
            insert_ph = 0;
            inserted = true;
        } else {
            let item = clause.item(j)?;
            if item.source != 0 {
                word_start = j;
            }
            ph = clause.tables.get(item.code);
            ph_slot = item.code;
            clause.items[j].phoneme = ph;
            if item.code == PHON_SWITCH {
                clause.tables.select(i32::from(item.tone))?;
            }
            next = clause.tables.get(clause.item(j + 1)?.code);
            clause.items[j + 1].phoneme = next;
        }
        let Some(mut phoneme) = ph else {
            j += 1;
            continue;
        };

        let mut data = clause.interpret(j);

        if let Some(alternative) = parameter(&data, CHANGE_NEXT_PHONEME) {
            let record = clause.tables.get(alternative).ok_or(Error::Phoneme)?;
            let after = &mut clause.items[j + 1];
            after.phoneme = Some(record);
            after.code = alternative;
            after.kind = record.kind;
            next = Some(record);
        }

        if let Some(alternative) = parameter(&data, INSERT_PHONEME).filter(|_| !inserted) {
            // PROBLEM: inserting before a vowel loses the stress
            let before = phoneme;
            insert_ph = clause.items[j].code;
            ph = clause.tables.get(alternative);
            (ph_table, ph_slot) = (clause.tables.index(), alternative);
            clause.items[j].phoneme = ph;
            clause.items[j].code = alternative;
            data = clause.reinterpret(j, ph, before)?;
            phoneme = ph.ok_or(Error::Phoneme)?;
        }

        if let Some(alternative) = parameter(&data, CHANGE_PHONEME) {
            let before = phoneme;
            ph = clause.tables.get(alternative);
            (ph_table, ph_slot) = (clause.tables.index(), alternative);
            clause.items[j].phoneme = ph;
            clause.items[j].code = alternative;
            if alternative == 1 {
                deleted = true; // NULL phoneme, discard
            } else {
                data = clause.reinterpret(j, ph, before)?;
            }
        }
        // C reads the type even of a discarded phoneme
        let phoneme = ph.ok_or(Error::Phoneme)?;

        if phoneme.kind == PH_VOWEL && !deleted {
            // consecutive unstressed syllables, even across word boundaries
            if clause.items[j].stress <= 1 {
                unstress_count += 1;
                if options.stress_flags & 0x08 != 0 {
                    // unstressed vowel sequences in unstressed words: diminished
                    let mut p = j + 1;
                    loop {
                        let after = clause.item(p)?;
                        if after.kind == PH_PAUSE {
                            break;
                        }
                        if after.kind == PH_VOWEL {
                            if after.stress <= 1 {
                                if clause.items[j].word_stress < 4 {
                                    clause.items[j].stress = 0;
                                }
                                if after.word_stress < 4 {
                                    clause.items[p].stress = 0;
                                }
                            }
                            break;
                        }
                        p += 1;
                    }
                } else if unstress_count > 1 && unstress_count & 1 == 0 {
                    // reduce alternate syllables, but not the last of a stressed word
                    if options.stress_flags & S_NO_DIM != 0
                        || (word_stress > 3 && clause.items[j + 1].source != 0)
                    {
                        unstress_count = 1; // try again for the next syllable
                    } else {
                        clause.items[j].stress = 0;
                    }
                }
            } else {
                unstress_count = 0;
            }
        }

        if clause.items[j + 1].flags & LENGTHEN != 0 && j > 0 {
            // lengthen this consonant by doubling it. The legacy strchr also
            // matches the list terminator, so a pause doubles too.
            let next = next.ok_or(Error::Phoneme)?;
            if matches!(
                next.kind,
                PH_FRICATIVE | PH_VFRICATIVE | PH_NASAL | PH_LIQUID | PH_PAUSE
            ) {
                insert_ph = next.code;
                clause.items[j + 1].flags ^= LENGTHEN;
            }
        }

        if clause.items[j + 1].source != 0 {
            let vowel_pause = options.vowel_pause;
            if vowel_pause != 0 && phoneme.kind != PH_PAUSE {
                if phoneme.kind != PH_VOWEL && vowel_pause & 0x200 != 0 {
                    insert_ph = PHON_PAUSE_NOLINK; // after a word ending in a consonant
                }
                if next.ok_or(Error::Phoneme)?.kind == PH_VOWEL {
                    let x = vowel_pause & 0x0c;
                    if x != 0 {
                        // break before a word starting with a vowel
                        insert_ph = if x == 0xc {
                            PHON_PAUSE_NOLINK
                        } else {
                            PHON_PAUSE_VSHORT
                        };
                    }
                    let x = vowel_pause & 0x03;
                    if phoneme.kind == PH_VOWEL && x != 0 {
                        // adjacent vowels over a word boundary
                        insert_ph = if x == 2 {
                            PHON_PAUSE_SHORT
                        } else {
                            PHON_PAUSE_VSHORT
                        };
                    }
                    if clause.items[j + 1].stress >= 4 && vowel_pause & 0x100 != 0 {
                        insert_ph = PHON_PAUSE_SHORT; // before a stressed initial vowel
                    }
                }
            }
            if inserted_at != Some(j) && ix > 0 {
                let x = (options.word_gap & 7) as usize;
                if x != 0
                    && (x > 1 || (insert_ph != PHON_PAUSE_SHORT && insert_ph != PHON_PAUSE_NOLINK))
                {
                    insert_ph = PAUSE_PHONEMES[x]; // don't reduce the pause
                }
                if options.option_wordgap > 0 {
                    insert_ph = PHON_PAUSE_LONG;
                }
            }
        }

        let after_next = clause.item(j + 2)?.code;
        clause.items[j + 2].phoneme = clause.tables.get(after_next);

        if insert_ph == 0 {
            if let Some(append) = parameter(&data, APPEND_PHONEME) {
                insert_ph = append;
            }
        }

        if !deleted {
            let item = clause.items[j];
            let out = output.get_mut(ix).ok_or(Error::Capacity)?;
            *out = Output {
                table: ph_table,
                slot: ph_slot,
                kind: phoneme.kind,
                env: PITCH_FALL, // default, can be changed by intonation
                synthflags: item.flags,
                stress: item.stress & 0xf,
                word_stress: item.word_stress,
                tone: item.tone,
                tone_table: if item.tone != 0 && item.flags & SWITCHED_LANG != 0 {
                    clause.tables.index()
                } else {
                    -1
                },
                code: phoneme.code,
                length: (data.parameters[SET_LENGTH] as u32).wrapping_mul(2),
                prepause: 0,
                amp: 20, // default, changed later
                pitch1: 255,
                pitch2: 255,
                ..Output::default()
            };
            if item.source != 0 {
                out.source = item.source;
                out.new_word = START_OF_WORD;
                if start_sentence {
                    out.new_word |= START_OF_SENTENCE;
                    start_sentence = false;
                }
                if start_of_clause {
                    out.new_word |= START_OF_CLAUSE;
                    start_of_clause = false;
                }
            }
            if phoneme.code == PHON_PAUSE_LONG
                && options.option_wordgap > 0
                && clause.items[j + 1].source != 0
            {
                out.table = clause.tables.index();
                out.slot = PHON_PAUSE_SHORT;
                out.length = (options.option_wordgap as u32).wrapping_mul(14); // 10mS per unit
            }
            if matches!(
                phoneme.kind,
                PH_VOWEL | PH_LIQUID | PH_NASAL | PH_VSTOP | PH_VFRICATIVE
            ) || phoneme.flags & PREVOICE != 0
            {
                out.length = 128; // length_mod
            }
            ix += 1;
        }
        j += 1;
    }

    // terminate with two pauses
    let table = clause.tables.index();
    let pause = output.get_mut(ix).ok_or(Error::Capacity)?;
    *pause = Output {
        new_word: END_OF_CLAUSE,
        code: PHON_PAUSE,
        kind: PH_PAUSE,
        length: options.post_pause as u32,
        source: end_source,
        table,
        slot: PHON_PAUSE,
        tone_table: -1,
        ..Output::default()
    };
    let pause = output.get_mut(ix + 1).ok_or(Error::Capacity)?;
    *pause = Output {
        code: PHON_PAUSE,
        kind: PH_PAUSE,
        table,
        slot: PHON_PAUSE_SHORT,
        tone_table: -1,
        ..Output::default()
    };
    Ok(ix + 2)
}

/// `SubstitutePhonemes`: copy into the working list with voice replacements.
fn substitute<H: Host>(
    source: &mut [Source],
    clause: &mut Clause<'_, '_, H>,
) -> Result<usize, Error> {
    let n2 = source.len();
    let mut out = 0;
    let mut next: Option<Phoneme> = None;
    let mut deleted_source: Option<u16> = None;
    let mut ix = 0;
    while ix < n2 && out < N_LIST {
        if let Some(s) = deleted_source.take() {
            source[ix].source = s;
        }
        if source[ix].code == PHON_SWITCH {
            clause.tables.select(i32::from(source[ix].tone))?;
        }
        // no substitution while the language is temporarily changed
        if source[ix].synthflags & SWITCHED_LANG == 0 {
            if ix < n2 - 1 {
                next = clause.tables.get(source[ix + 1].code);
            }
            let word_end = ix == n2 - 1
                || source[ix + 1].source != 0
                || next.is_some_and(|p| p.kind == PH_PAUSE);
            let s = &mut source[ix];
            for replacement in clause.options.replacements {
                if s.code != replacement.old {
                    continue;
                }
                let kind = replacement.kind;
                if (kind & 1 != 0 && !word_end)
                    || (kind & 2 != 0 && s.stress & 7 > 3)
                    || (kind & 4 != 0 && s.source == 0)
                {
                    continue;
                }
                s.code = replacement.new;
                if s.stress > 1
                    && clause
                        .tables
                        .get(s.code)
                        .is_some_and(|p| p.flags & UNSTRESSED != 0)
                {
                    s.stress = 0; // the replacement must be unstressed
                }
                break;
            }
            if s.code == 0 {
                // replaced by NULL: drop it, keeping a word start
                deleted_source = Some(s.source);
                ix += 1;
                continue;
            }
        }
        let s = source[ix];
        let Some(phoneme) = clause.tables.get(s.code) else {
            // not in the current table: nothing downstream can interpret it
            deleted_source = Some(s.source);
            ix += 1;
            continue;
        };
        clause.items[out] = Item {
            phoneme: Some(phoneme),
            flags: s.synthflags,
            code: s.code,
            stress: s.stress,
            source: s.source,
            word_stress: s.word_stress,
            tone: s.tone,
            kind: phoneme.kind,
        };
        out += 1;
        ix += 1;
    }
    Ok(out)
}

/// Sets consonant clusters to all voiced or all unvoiced, right to left.
fn regressive_voicing<H: Host>(
    source: &mut [Source],
    options: &Options<'_>,
    tables: &mut Tables<'_, H>,
) -> Result<(), Error> {
    let regression = options.regression;
    let mut stop_propagation = false;
    let mut voicing = 0;
    for j in (0..source.len()).rev() {
        if source[j].code == PHON_SWITCH {
            // the table we're switching back to
            match source[..j].iter().rposition(|s| s.code == PHON_SWITCH) {
                Some(k) => tables.select(i32::from(source[k].tone))?,
                None => tables.select(options.table)?,
            }
        }
        let Some(ph) = tables.get(source[j].code) else {
            continue;
        };
        if source[j].synthflags & SWITCHED_LANG != 0 {
            stop_propagation = false;
            voicing = if regression & 0x100 != 0 { 1 } else { 0 }; // word-end devoicing
            continue;
        }
        if regression & 0x2 != 0 {
            // [v], [v;] and [R^] don't cause regression
            let first = ph.mnemonic & 0xff;
            if first == u32::from(b'v') || first == u32::from(b'R') {
                stop_propagation = true;
                if regression & 0x10 != 0 {
                    voicing = 0;
                }
            }
        }
        match ph.kind {
            4 | PH_FRICATIVE => {
                if voicing == 0 && regression & 0xf != 0 {
                    voicing = 1;
                } else if voicing == 2 && ph.end_type != 0 {
                    source[j].code = ph.end_type; // the voiced equivalent
                }
            }
            PH_VSTOP | PH_VFRICATIVE => {
                if voicing == 0 && regression & 0xf != 0 {
                    voicing = 2;
                } else if voicing == 1 && ph.end_type != 0 {
                    source[j].code = ph.end_type; // the unvoiced equivalent
                }
            }
            kind => {
                // Polish propagates through liquids and nasals
                if regression & 0x8 == 0 || kind == PH_PAUSE || kind == PH_VOWEL {
                    voicing = 0;
                }
            }
        }
        if stop_propagation {
            voicing = 0;
            stop_propagation = false;
        }
        if source[j].source != 0 {
            if regression & 0x04 != 0 {
                voicing = 0; // stop at a word boundary
            }
            if regression & 0x100 != 0 && voicing == 0 {
                voicing = 1; // devoice word-final consonants
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed {
        tables: [Table; 2],
        selected: Vec<i32>,
    }
    impl Host for Fixed {
        fn select(&mut self, index: i32, table: &mut Table) -> Result<(), Error> {
            self.selected.push(index);
            *table = *self.tables.get(index as usize).ok_or(Error::Host)?;
            Ok(())
        }
    }

    fn host() -> Fixed {
        let mut table = [None; 256];
        for (code, kind, flags) in [
            (PHON_PAUSE, PH_PAUSE, 0),
            (PHON_PAUSE_SHORT, PH_PAUSE, 0),
            (PHON_SWITCH, PH_PAUSE, 0),
            (30, PH_VOWEL, 0),
            (31, 4, 0),
            (32, PH_NASAL, 0),
        ] {
            table[usize::from(code)] = Some(Phoneme {
                code,
                kind,
                flags,
                standard_length: 50,
                ..Phoneme::default()
            });
        }
        Fixed {
            tables: [table; 2],
            selected: Vec::new(),
        }
    }

    fn src(code: u8, stress: u8, source: u16) -> Source {
        Source {
            code,
            stress,
            source,
            ..Source::default()
        }
    }

    fn options(replacements: &[Replacement]) -> Options<'_> {
        Options {
            table: 0,
            regression: 0,
            reduction: 0,
            stress_flags: 0,
            vowel_pause: 0,
            word_gap: 0,
            option_wordgap: 0,
            klatt: false,
            mbrola: false,
            post_pause: 30,
            start_sentence: true,
            replacements,
            programs: &[],
        }
    }

    #[test]
    fn words_stress_and_terminators() {
        // "ta na" with an unstressed final word whose predecessor can be promoted
        let mut source = [
            src(PHON_PAUSE, 0, 0),
            Source {
                synthflags: PROMOTE_STRESS,
                ..src(31, 1, 1)
            },
            src(30, 1, 0),
            src(32, 1, 3),
            src(30, 1, 0),
            src(PHON_PAUSE, 0, 0),
            src(PHON_PAUSE, 0, 9),
        ];
        let mut count = source.len();
        let mut output = [Output::default(); MAX_OUTPUT];
        let mut host = host();
        let n = make_phoneme_list(
            &mut source,
            &mut count,
            &mut output,
            &options(&[]),
            &mut host,
        )
        .unwrap();
        assert_eq!(source[1].stress, 4);
        // leading pause, t a n a, then the two terminators
        assert_eq!(n, 7);
        let codes: Vec<u8> = output[..n].iter().map(|o| o.slot).collect();
        assert_eq!(
            codes,
            [PHON_PAUSE, 31, 30, 32, 30, PHON_PAUSE, PHON_PAUSE_SHORT]
        );
        assert_eq!(
            output[1].new_word,
            START_OF_WORD | START_OF_SENTENCE | START_OF_CLAUSE
        );
        assert_eq!(output[3].new_word, START_OF_WORD);
        assert_eq!((output[1].word_stress, output[3].word_stress), (4, 1));
        assert_eq!(output[2].length, 128);
        assert_eq!(output[1].length, 100);
        assert_eq!(output[5].new_word, END_OF_CLAUSE);
        assert_eq!((output[5].length, output[5].source), (30, 9));
    }

    #[test]
    fn redundant_switch_and_deleting_replacement() {
        let mut source = [
            src(PHON_PAUSE, 0, 0),
            src(PHON_SWITCH, 0, 0), // to the current table: deleted
            src(31, 4, 1),
            src(32, 1, 0), // replaced by nothing at word end
            src(30, 4, 0),
            src(PHON_PAUSE, 0, 0),
            src(PHON_PAUSE, 0, 0),
        ];
        let mut count = source.len();
        let mut output = [Output::default(); MAX_OUTPUT];
        let mut host = host();
        let replace = [Replacement {
            old: 32,
            new: 0,
            kind: 0,
        }];
        let n = make_phoneme_list(
            &mut source,
            &mut count,
            &mut output,
            &options(&replace),
            &mut host,
        )
        .unwrap();
        assert_eq!(count, 6);
        assert_eq!(source[1].code, 31);
        let codes: Vec<u8> = output[..n].iter().map(|o| o.slot).collect();
        assert_eq!(codes, [PHON_PAUSE, 31, 30, PHON_PAUSE, PHON_PAUSE_SHORT]);

        // a table the host can't supply
        let mut source = [src(PHON_PAUSE, 0, 0); 5];
        source[1] = Source {
            tone: 5,
            ..src(PHON_SWITCH, 0, 0)
        };
        source[2] = src(30, 4, 1);
        let mut count = 5;
        assert_eq!(
            make_phoneme_list(
                &mut source,
                &mut count,
                &mut output,
                &options(&[]),
                &mut host
            ),
            Err(Error::Host)
        );
    }

    #[test]
    fn program_replaces_next_phoneme() {
        // No compiled phoneme uses ChangeNextPhoneme, so the oracle can't
        // reach it: vowel 30 runs [0x0220 (replace next with 32), return].
        let programs = [0u8, 0, 0x20, 0x02, 0x01, 0x00];
        let mut host = host();
        for table in &mut host.tables {
            table[30].as_mut().unwrap().program = 1;
        }
        let mut source = [
            src(PHON_PAUSE, 0, 0),
            src(31, 4, 1),
            src(30, 4, 0),
            src(31, 1, 0),
            src(PHON_PAUSE, 0, 0),
            src(PHON_PAUSE, 0, 0),
        ];
        let mut count = source.len();
        let mut output = [Output::default(); MAX_OUTPUT];
        let options = Options {
            programs: &programs,
            ..options(&[])
        };
        let n =
            make_phoneme_list(&mut source, &mut count, &mut output, &options, &mut host).unwrap();
        let codes: Vec<u8> = output[..n].iter().map(|o| o.slot).collect();
        assert_eq!(
            codes,
            [PHON_PAUSE, 31, 30, 32, PHON_PAUSE, PHON_PAUSE_SHORT]
        );
        assert_eq!(output[3].kind, PH_NASAL);
    }
}
