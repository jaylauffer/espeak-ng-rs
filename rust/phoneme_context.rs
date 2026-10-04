//! Native phoneme conditions over a bounded, owner-supplied clause context.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2018 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    phoneme::Phoneme,
    phoneme_data::InvalidPhonemeData as Error,
    phoneme_program::{Environment, Program},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub phoneme: Option<Phoneme>,
    pub code: u8,
    pub stress: u8,
    pub word_stress: u8,
    pub source: u16,
    pub flags: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    List(usize),
    PreviousVowel,
}

/// Access borrows resident state; refresh only resolves a table-local code.
/// Implementations must not allocate, perform I/O or change list bounds here.
pub trait Storage {
    fn entry(&self, position: Position) -> Option<Entry>;
    fn phoneme(&self, code: u8) -> Option<Phoneme>;
    fn refresh(&mut self, position: Position);
    fn invalid_instruction(&mut self, _instruction: u16) {}
}

/// Native engine owners can borrow reusable clause/table storage directly.
pub struct SliceStorage<'a> {
    pub list: &'a mut [Entry],
    pub table: &'a [Option<Phoneme>; 256],
    pub previous_vowel: Option<&'a mut Entry>,
}
impl Storage for SliceStorage<'_> {
    fn entry(&self, position: Position) -> Option<Entry> {
        match position {
            Position::List(index) => self.list.get(index).copied(),
            Position::PreviousVowel => self.previous_vowel.as_deref().copied(),
        }
    }
    fn phoneme(&self, code: u8) -> Option<Phoneme> {
        self.table[usize::from(code)]
    }
    fn refresh(&mut self, position: Position) {
        let entry = match position {
            Position::List(index) => self.list.get_mut(index),
            Position::PreviousVowel => self.previous_vowel.as_deref_mut(),
        };
        if let Some(entry) = entry {
            entry.phoneme = self.table[usize::from(entry.code)];
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Settings {
    pub length: usize,
    pub current: usize,
    pub control: u32,
    pub has_translator: u32,
    pub reduction: i32,
    pub klatt: u32,
    pub mbrola: u32,
}
/// Bounds cover initialized entries, including any explicit pause sentinels.
pub struct Context<'a, S> {
    program: Program<'a>,
    settings: Settings,
    storage: S,
}
impl<'a, S: Storage> Context<'a, S> {
    pub fn new(program: Program<'a>, settings: Settings, storage: S) -> Result<Self, Error> {
        if settings.length == 0 || settings.length > 1001 || settings.current >= settings.length {
            return Err(Error("invalid phoneme clause bounds"));
        }
        Ok(Self {
            program,
            settings,
            storage,
        })
    }
    fn entry(&self, position: Position) -> Option<Entry> {
        if matches!(position, Position::List(index) if index >= self.settings.length) {
            return None;
        }
        self.storage.entry(position)
    }
    fn shift(&self, position: Position, delta: isize) -> Option<Position> {
        match position {
            Position::List(index) => index
                .checked_add_signed(delta)
                .filter(|index| *index < self.settings.length)
                .map(Position::List),
            Position::PreviousVowel => None, // isolated snapshot, no adjacent allocation
        }
    }
    pub fn stress_condition(&self, position: Position, condition: u8, changing: bool) -> bool {
        let Some(current) = self.entry(position) else {
            return false;
        };
        let vowel = if self
            .storage
            .phoneme(current.code)
            .is_some_and(|ph| ph.kind == 2)
        {
            current
        } else {
            let Some(next) = self.shift(position, 1).and_then(|p| self.entry(p)) else {
                return false;
            };
            if !self
                .storage
                .phoneme(next.code)
                .is_some_and(|ph| ph.kind == 2)
            {
                return false;
            }
            next
        };
        let mut stress = vowel.stress & 15;
        if self.settings.has_translator != 0 {
            if changing && current.flags & 0x10 != 0 && self.settings.reduction & 1 == 0 {
                return false;
            }
            if self.settings.reduction & 2 != 0 && stress >= vowel.word_stress {
                stress = 4;
            }
        }
        match condition {
            4 => stress >= vowel.word_stress,
            3 => stress > 3,
            0..=2 => stress < [1, 2, 4][usize::from(condition)],
            _ => false,
        }
    }
    /// Evaluate a single 2xxx/3xxx condition. Extended selectors require their
    /// second instruction word. Neighbour refresh side effects match C.
    pub fn evaluate(&mut self, instruction: u16, selector: Option<u16>) -> Result<bool, Error> {
        let instruction = instruction & 0xfff;
        let kind = instruction >> 8;
        let data = instruction as u8;
        if kind == 15 {
            return Ok(match data {
                1 => self.settings.control & 1 != 0,
                2 => self.settings.klatt != 0,
                3 => self.settings.mbrola != 0,
                _ => false,
            });
        }
        if kind >= 14 {
            return Ok(false);
        }
        let original = Position::List(self.settings.current);
        let which = if kind % 7 == 6 {
            selector.ok_or(Error("missing extended phoneme selector"))?
        } else {
            kind % 7
        };
        let boundary = |context: &Self, delta| {
            context
                .shift(original, delta)
                .and_then(|p| context.entry(p))
                .is_none_or(|entry| entry.source != 0)
        };
        if (which == 4 && boundary(self, 1))
            || (which == 5 && boundary(self, 0))
            || (which == 6 && (boundary(self, 1) || boundary(self, 2)))
            || (which == 9 && (1..=3).any(|delta| boundary(self, delta)))
            || (which == 10 && (boundary(self, 0) || boundary(self, -1)))
        {
            return Ok(false);
        }
        let check_end = matches!(which, 0 | 5 | 8 | 10);
        let mut position = match which {
            0 | 5 => self.shift(original, -1),
            2 | 4 => self.shift(original, 1),
            3 | 6 => self.shift(original, 2),
            9 => self.shift(original, 3),
            10 => self.shift(original, -2),
            8 => self
                .entry(Position::PreviousVowel)
                .filter(|e| e.phoneme.is_some())
                .map(|_| Position::PreviousVowel),
            7 => {
                let mut selected = None;
                for index in self.settings.current + 1..self.settings.length {
                    let Some(entry) = self.entry(Position::List(index)) else {
                        break;
                    };
                    if entry.source != 0 {
                        break;
                    }
                    let Some(ph) = self.storage.phoneme(entry.code) else {
                        break;
                    };
                    if ph.kind == 2 {
                        selected = Some(Position::List(index));
                        break;
                    }
                }
                selected
            }
            _ => Some(original),
        };
        if matches!(which, 0 | 5)
            && position
                .and_then(|p| self.entry(p))
                .is_some_and(|e| e.code == 1)
        {
            position = position.and_then(|p| self.shift(p, -1));
        }
        let Some(position) = position else {
            return Ok(false);
        };
        if self.settings.control & 0x100 != 0 {
            self.storage.refresh(position);
        }
        let Some(entry) = self.entry(position) else {
            return Ok(false);
        };
        let Some(ph) = entry.phoneme else {
            return Ok(false);
        };
        if kind < 7 {
            if self
                .storage
                .phoneme(data)
                .is_some_and(|target| target.mnemonic == ph.mnemonic)
            {
                return Ok(true);
            }
            return Ok(data
                == if check_end && ph.kind == 2 {
                    ph.end_type
                } else {
                    ph.start_type
                });
        }
        let value = data & 31;
        Ok(match data & 0xe0 {
            0 => ph.kind == value,
            0x20 => (ph.flags >> 16) & 15 == u32::from(value),
            0x40 => ph.flags & (1_u32 << value) != 0,
            0x80 => match value {
                0..=4 => self.stress_condition(position, value, false),
                17 => ph.kind == 0 || self.entry(original).is_some_and(|e| e.flags & 0x2000 != 0),
                18 => entry.source != 0,
                19 => self
                    .shift(position, 1)
                    .and_then(|p| self.entry(p))
                    .is_none_or(|e| e.source != 0 || e.phoneme.is_none_or(|ph| ph.kind == 0)),
                9 => {
                    if entry.source != 0 {
                        false
                    } else {
                        let mut cursor = position;
                        let mut found = false;
                        while let Some(previous) = self.shift(cursor, -1) {
                            let Some(entry) = self.entry(previous) else {
                                break;
                            };
                            if entry.stress & 15 >= 4 {
                                found = true;
                                break;
                            }
                            if entry.source != 0 {
                                break;
                            }
                            cursor = previous;
                        }
                        found
                    }
                }
                10 => ph.kind != 2,
                11 => {
                    let mut cursor = position;
                    let mut final_vowel = true;
                    while let Some(next) = self.shift(cursor, 1) {
                        let Some(entry) = self.entry(next) else {
                            break;
                        };
                        if entry.source != 0 || entry.phoneme.is_none() {
                            break;
                        }
                        if entry.phoneme.is_some_and(|ph| ph.kind == 2) {
                            final_vowel = false;
                            break;
                        }
                        cursor = next;
                    }
                    final_vowel
                }
                12 => matches!(ph.kind, 2 | 3) || ph.flags & 16 != 0,
                13 | 14 => {
                    let mut cursor = position;
                    let mut count: usize = 0;
                    while let Some(entry) = self.entry(cursor) {
                        if entry.phoneme.is_some_and(|ph| ph.kind == 2) {
                            count += 1;
                        }
                        if entry.source != 0 {
                            break;
                        }
                        let Some(previous) = self.shift(cursor, -1) else {
                            break;
                        };
                        cursor = previous;
                    }
                    count == usize::from(value - 12)
                }
                16 => entry.flags & 0x10 != 0,
                _ => false,
            },
            _ => false,
        })
    }
}
impl<S: Storage> Environment for Context<'_, S> {
    fn condition(&mut self, offset: usize) -> Result<bool, Error> {
        let instruction = self.program.word(offset)?;
        let selector = if matches!(instruction & 0xf00, 0x600 | 0xd00) {
            Some(self.program.word(offset + 1)?)
        } else {
            None
        };
        self.evaluate(instruction, selector)
    }
    fn stress(&mut self, condition: u8) -> bool {
        self.stress_condition(Position::List(self.settings.current), condition, true)
    }
    fn next_is_vowel(&mut self) -> bool {
        self.shift(Position::List(self.settings.current), 1)
            .and_then(|p| self.entry(p))
            .and_then(|entry| self.storage.phoneme(entry.code))
            .is_some_and(|ph| ph.kind == 2)
    }
    fn vowel_type(&mut self, next: bool) -> Option<u8> {
        self.shift(
            Position::List(self.settings.current),
            if next { 1 } else { -1 },
        )
        .and_then(|p| self.entry(p))
        .and_then(|entry| entry.phoneme)
        .map(|ph| if next { ph.start_type } else { ph.end_type })
    }
    fn invalid_instruction(&mut self, instruction: u16) {
        self.storage.invalid_instruction(instruction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn phoneme(code: u8, kind: u8) -> Phoneme {
        Phoneme {
            code,
            kind,
            mnemonic: u32::from(code),
            start_type: 28,
            end_type: 29,
            flags: (3 << 16) | 16,
            ..Phoneme::default()
        }
    }
    fn fixture() -> ([Entry; 5], [Option<Phoneme>; 256]) {
        let vowel = phoneme(2, 2);
        let consonant = phoneme(3, 3);
        let mut table = [None; 256];
        table[2] = Some(vowel);
        table[3] = Some(consonant);
        let vowel = Entry {
            phoneme: Some(vowel),
            code: 2,
            stress: 4,
            word_stress: 4,
            ..Entry::default()
        };
        let consonant = Entry {
            phoneme: Some(consonant),
            code: 3,
            ..Entry::default()
        };
        (
            [
                Entry {
                    source: 1,
                    ..consonant
                },
                vowel,
                consonant,
                vowel,
                Entry {
                    source: 1,
                    ..consonant
                },
            ],
            table,
        )
    }
    fn settings(current: usize, length: usize) -> Settings {
        Settings {
            current,
            length,
            ..Settings::default()
        }
    }
    #[test]
    fn stress_uses_table_local_vowels_and_reduction_policy() {
        let (mut list, table) = fixture();
        list[1].stress = 2;
        list[1].word_stress = 2;
        let program = Program::new(&[]).unwrap();
        let mut context = Context::new(
            program,
            Settings {
                has_translator: 1,
                reduction: 2,
                ..settings(0, 5)
            },
            SliceStorage {
                list: &mut list,
                table: &table,
                previous_vowel: None,
            },
        )
        .unwrap();
        assert!(context.stress_condition(Position::List(0), 3, false));
        assert!(context.stress_condition(Position::List(0), 4, false));
        assert!(!context.stress_condition(Position::List(0), 2, false));
        context.settings.reduction = 0;
        assert!(context.stress_condition(Position::List(0), 2, true));
        context.storage.list[0].flags = 0x10;
        assert!(!context.stress_condition(Position::List(0), 2, true));
        assert!(context.stress_condition(Position::List(0), 2, false));
        context.settings.reduction = 1;
        assert!(context.stress_condition(Position::List(0), 2, true));
        assert!(!context.stress_condition(Position::List(0), 7, true));
        context.storage.list[1].code = 255;
        assert!(!context.stress_condition(Position::List(0), 4, false));
    }
    #[test]
    fn selectors_honor_word_boundaries_deleted_records_and_refresh_side_effects() {
        let (mut list, mut table) = fixture();
        table[1] = Some(phoneme(1, 0));
        list[2].code = 1;
        let program = Program::new(&[]).unwrap();
        let mut context = Context::new(
            program,
            settings(3, 5),
            SliceStorage {
                list: &mut list,
                table: &table,
                previous_vowel: None,
            },
        )
        .unwrap();
        assert!(context.evaluate(0x2002, None).unwrap()); // deleted previous code skips back to vowel
        assert!(context.evaluate(0x2602, Some(10)).unwrap()); // previous two in word
        assert!(!context.evaluate(0x2602, Some(7)).unwrap()); // no next vowel across source mark
        assert!(!context.evaluate(0x2403, None).unwrap());
        context.settings.current = 2;
        context.settings.control = 0x100;
        context.storage.list[3].code = 255;
        assert!(!context.evaluate(0x2902, None).unwrap());
        assert!(context.storage.list[3].phoneme.is_none());
        assert!(context.evaluate(0x2602, None).is_err());
        context.settings.current = 0;
        assert!(!context.evaluate(0x2002, None).unwrap());
    }
    #[test]
    fn vowel_positions_flags_and_backend_conditions_use_bounded_storage() {
        let (mut list, table) = fixture();
        let program = Program::new(&[]).unwrap();
        let mut context = Context::new(
            program,
            Settings {
                control: 1,
                klatt: 1,
                mbrola: 1,
                ..settings(3, 5)
            },
            SliceStorage {
                list: &mut list,
                table: &table,
                previous_vowel: None,
            },
        )
        .unwrap();
        assert!(context.evaluate(0x288e, None).unwrap()); // thisPh isSecondVowel: kind 8 % 7 == 1
        assert!(!context.evaluate(0x288d, None).unwrap());
        assert!(context.evaluate(0x2889, None).unwrap()); // after stress
        assert!(context.evaluate(0x288b, None).unwrap()); // final vowel
        assert!(context.evaluate(0x288c, None).unwrap()); // voiced
        assert!(context.evaluate(0x2844, None).unwrap()); // flag bit 4
        assert!(context.evaluate(0x2823, None).unwrap()); // place 3
        assert!(context.evaluate(0x2882, None).is_ok());
        for condition in [0x2f01, 0x2f02, 0x2f03] {
            assert!(context.evaluate(condition, None).unwrap());
        }
        context.settings.current = 4;
        assert!(context.evaluate(0x2893, None).unwrap()); // EOF is word end
        assert!(!context.next_is_vowel());
        let mut long = vec![context.storage.list[1]; 1001];
        let mut context = Context::new(
            program,
            settings(1000, 1001),
            SliceStorage {
                list: &mut long,
                table: &table,
                previous_vowel: None,
            },
        )
        .unwrap();
        assert!(!context.evaluate(0x288e, None).unwrap()); // count >255 must not overflow
        assert!(Context::new(
            program,
            settings(1001, 1001),
            SliceStorage {
                list: &mut [],
                table: &table,
                previous_vowel: None
            }
        )
        .is_err());
    }
    #[test]
    fn previous_vowel_is_an_isolated_snapshot_with_no_neighbour_reads() {
        let (mut list, table) = fixture();
        let mut previous = list[1];
        previous.flags = 0x10;
        let mut context = Context::new(
            Program::new(&[]).unwrap(),
            settings(3, 5),
            SliceStorage {
                list: &mut list,
                table: &table,
                previous_vowel: Some(&mut previous),
            },
        )
        .unwrap();
        assert!(context.evaluate(0x2602, Some(8)).unwrap());
        assert!(context.evaluate(0x2d82, Some(8)).is_ok());
        assert!(context.evaluate(0x2d90, Some(8)).unwrap());
        assert!(context.evaluate(0x2d93, Some(8)).unwrap());
        assert!(context.evaluate(0x2d8b, Some(8)).unwrap());
        assert!(!context.evaluate(0x2d89, Some(8)).unwrap());
        context.settings.control = 0x100;
        context.storage.previous_vowel.as_deref_mut().unwrap().code = 255;
        assert!(!context.evaluate(0x2d02, Some(8)).unwrap());
        assert!(context
            .storage
            .previous_vowel
            .as_deref()
            .unwrap()
            .phoneme
            .is_none());
    }
}
