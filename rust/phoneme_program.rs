//! Bounded phoneme-program execution over resident little-endian instructions.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2018 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
//!
//! The owner supplies language/stress conditions and neighbouring vowel types.
//! No allocation, I/O, executor or process-global state belongs in this VM.

use crate::phoneme::Phoneme;
use crate::phoneme_data::InvalidPhonemeData as Error;

/// Layout matches legacy PHONEME_DATA, including its fixed IPA output buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct PhonemeData {
    pub control: i32,
    pub parameters: [i32; 16],
    pub sound_addresses: [i32; 5],
    pub sound_parameters: [i32; 5],
    pub vowel_transitions: [i32; 4],
    pub pitch_envelope: i32,
    pub amplitude_envelope: i32,
    pub ipa: [u8; 18],
}

/// Callbacks must be synchronous and bounded, borrowing the owner's current
/// phoneme context. A condition can refresh neighbouring phoneme resolution.
pub trait Environment {
    fn condition(&mut self, word_offset: usize) -> Result<bool, Error>;
    fn stress(&mut self, condition: u8) -> bool;
    fn next_is_vowel(&mut self) -> bool;
    /// Next vowel start type or previous vowel end type; None means a missing
    /// phoneme after a table switch, preserving the fork's switch behavior.
    fn vowel_type(&mut self, next: bool) -> Option<u8>;
    fn invalid_instruction(&mut self, _instruction: u16) {}
}

/// A borrowed phonindex. Addresses and call targets are 16-bit word offsets.
#[derive(Clone, Copy, Debug)]
pub struct Program<'a> {
    bytes: &'a [u8],
}
impl<'a> Program<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() % 2 != 0 {
            return Err(Error("phonindex has an incomplete 16-bit word"));
        }
        Ok(Self { bytes })
    }
    pub(crate) fn word(&self, offset: usize) -> Result<u16, Error> {
        let offset = offset
            .checked_mul(2)
            .ok_or(Error("phoneme instruction offset overflow"))?;
        let bytes = self
            .bytes
            .get(
                offset
                    ..offset
                        .checked_add(2)
                        .ok_or(Error("phoneme instruction offset overflow"))?,
            )
            .ok_or(Error("phoneme instruction outside resident phonindex"))?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn sound_lookahead(&self, offset: usize) -> Result<u16, Error> {
        // The final compiled FMT/WAV/addWav can end exactly at EOF. Legacy C
        // reads one word past the allocation to decide its implicit return.
        // Treat that boundary as Return; never permit other out-of-range reads.
        if offset == self.bytes.len() / 2 {
            Ok(1)
        } else {
            self.word(offset)
        }
    }
    /// Preserve the legacy skip widths, including switch and sound lookahead.
    pub fn instruction_words(&self, offset: usize) -> Result<usize, Error> {
        let instruction = self.word(offset)?;
        let kind = instruction >> 12;
        Ok(match kind {
            0 if (instruction >> 8) & 15 == 13 => usize::from((instruction & 255).div_ceil(2)) + 1,
            0 | 1 | 4 | 5 | 7 | 8 => 1,
            2 | 3 if matches!(instruction & 0xf00, 0x600 | 0xd00) => 2,
            2 | 3 => 1,
            6 if matches!((instruction & 0xf00) >> 9, 5 | 6) => 12,
            6 => 1,
            9 => 2,
            10 => 4,
            _ => match self.sound_lookahead(offset + 2)? {
                next if next >> 12 == 15 => 4,
                2 => 3,
                _ => 2,
            },
        })
    }
    /// Execute with a ten-entry return stack and a fixed instruction budget.
    /// Malformed/truncated programs and nonterminating control flow return an
    /// error. Valid fork behavior includes full-stack calls being ignored.
    pub fn interpret<E: Environment>(
        &self,
        phoneme: &Phoneme,
        control: u32,
        has_translator: bool,
        environment: &mut E,
    ) -> Result<PhonemeData, Error> {
        let mut output = PhonemeData::default();
        output.parameters[10] = i32::from(phoneme.standard_length);
        output.parameters[9] = i32::from(phoneme.length_modifier);
        if phoneme.program == 0 {
            return Ok(output);
        }
        let mut pc = usize::from(phoneme.program);
        let mut returns = [0; 10];
        let mut depth = 0;
        let mut end = 0;
        let mut budget = 65_536;
        loop {
            spend(&mut budget)?;
            let instruction = self.word(pc)?;
            let operand = i32::from(instruction & 255);
            let sub = (instruction >> 8) & 15;
            let mut next = pc + 1;
            match instruction >> 12 {
                0 => match sub {
                    0 => match operand {
                        1 => end = 1,
                        2 => {}
                        _ => environment.invalid_instruction(instruction),
                    },
                    5 => {
                        if environment.next_is_vowel() {
                            output.parameters[4] = operand;
                        }
                    }
                    12 => {
                        output.parameters[10] =
                            output.parameters[10].wrapping_add(i32::from(operand as i8))
                    }
                    13 => {
                        let count = (operand as usize).min(16).div_ceil(2);
                        for index in 0..count {
                            let word = self.word(pc + 1 + index)?;
                            output.ipa[index * 2] = (word >> 8) as u8;
                            output.ipa[index * 2 + 1] = word as u8;
                        }
                        output.ipa[count * 2] = 0;
                        next += count;
                    }
                    _ => {
                        output.parameters[sub as usize] = operand;
                        if sub == 1 && control & 0x100 != 0 {
                            end = 1;
                        }
                    }
                },
                1 => {
                    if has_translator && sub < 8 && environment.stress(sub as u8) {
                        output.parameters[1] = operand;
                        end = 1;
                    }
                }
                2 | 3 => {
                    let mut truth = true;
                    let mut or = false;
                    let mut cursor = pc;
                    let mut current = instruction;
                    while current & 0xe000 == 0x2000 {
                        spend(&mut budget)?;
                        let width = self.instruction_words(cursor)?;
                        // Check extended selectors before handing their offset to the host.
                        self.word(cursor + width - 1)?;
                        let mut matched = environment.condition(cursor)?;
                        cursor += width;
                        if self.word(cursor)? == 3 {
                            matched = !matched;
                            cursor += 1;
                        }
                        truth = if or {
                            truth || matched
                        } else {
                            truth && matched
                        };
                        or = current & 0x1000 != 0;
                        current = self.word(cursor)?;
                    }
                    if !truth {
                        if current & 0xf800 == 0x6800 {
                            cursor += usize::from(current & 255);
                        } else {
                            cursor += self.instruction_words(cursor)?;
                            if self.word(cursor)? & 0xfe00 == 0x6000 {
                                cursor += 1;
                            }
                        }
                    }
                    next = cursor;
                }
                6 => match sub >> 1 {
                    0 => next = pc + operand as usize,
                    5 | 6 => {
                        let following = sub >> 1 == 5;
                        if following {
                            output.control |= 2;
                        }
                        if let Some(kind) = environment.vowel_type(following) {
                            if let Some(kind) = kind.checked_sub(28).filter(|n| *n < 6) {
                                let data = self.word(pc + usize::from(kind) * 2 + 1)?;
                                let address = self.word(pc + usize::from(kind) * 2 + 2)?;
                                let slot = if following { 2 } else { 3 };
                                output.sound_addresses[slot] =
                                    (i32::from(data & 15) << 18) + (i32::from(address) << 2);
                                output.sound_parameters[slot] = i32::from((data >> 4) as u8 as i8);
                            }
                            next = pc + 13;
                        }
                    }
                    _ => {}
                },
                9 => {
                    let address =
                        (i32::from(instruction & 15) << 16) + i32::from(self.word(pc + 1)?);
                    next = pc + 2;
                    match sub {
                        1 if depth < returns.len() => {
                            returns[depth] = next;
                            depth += 1;
                            next = address as usize;
                        }
                        2 => output.pitch_envelope = address,
                        3 => output.amplitude_envelope = address,
                        _ => {}
                    }
                }
                10 => {
                    let slot = if sub == 1 { 0 } else { 2 };
                    output.vowel_transitions[slot] =
                        (i32::from(instruction & 255) << 16) + i32::from(self.word(pc + 1)?);
                    output.vowel_transitions[slot + 1] = (u32::from(self.word(pc + 2)?) << 16
                        | u32::from(self.word(pc + 3)?))
                        as i32;
                    next = pc + 4;
                }
                11..=15 => {
                    let slot = usize::from((instruction >> 12) - 11);
                    output.sound_addresses[slot] =
                        (i32::from(instruction & 15) << 18) + (i32::from(self.word(pc + 1)?) << 2);
                    let parameter = (instruction >> 4) as u8;
                    output.sound_parameters[slot] = i32::from(parameter);
                    let after = self.sound_lookahead(pc + 2)?;
                    if after != 2 {
                        if slot < 2 {
                            end = if after >> 12 == 15 { 2 } else { 1 };
                        } else if slot == 4 {
                            end -= 1;
                        }
                        if slot == 2 || slot == 3 {
                            output.sound_parameters[slot] = i32::from(parameter as i8);
                        }
                    }
                    next = pc + 2;
                }
                _ => environment.invalid_instruction(instruction),
            }
            if end == 1 {
                if depth == 0 {
                    return Ok(output);
                }
                depth -= 1;
                next = returns[depth];
                end = 0;
            }
            pc = next;
        }
    }
}
fn spend(budget: &mut usize) -> Result<(), Error> {
    *budget = budget
        .checked_sub(1)
        .ok_or(Error("phoneme program instruction budget exceeded"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Inputs {
        condition: bool,
        vowel: Option<u8>,
        invalid: usize,
    }
    impl Environment for Inputs {
        fn condition(&mut self, _: usize) -> Result<bool, Error> {
            Ok(self.condition)
        }
        fn stress(&mut self, _: u8) -> bool {
            self.condition
        }
        fn next_is_vowel(&mut self) -> bool {
            self.vowel.is_some()
        }
        fn vowel_type(&mut self, _: bool) -> Option<u8> {
            self.vowel
        }
        fn invalid_instruction(&mut self, _: u16) {
            self.invalid += 1;
        }
    }
    fn bytes(words: &[u16]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
    fn phoneme() -> Phoneme {
        Phoneme {
            program: 1,
            standard_length: 40,
            length_modifier: 3,
            ..Phoneme::default()
        }
    }
    #[test]
    fn calls_conditions_and_implicit_sound_returns_preserve_parameters() {
        let data = bytes(&[
            0, 0x9100, 12, 0x2182, 3, 0x0711, 0xbff1, 0x1234, 0xfff2, 0x5678, 1, 0, 0x0cf8, 0x050a,
            1,
        ]);
        let result = Program::new(&data)
            .unwrap()
            .interpret(
                &phoneme(),
                0,
                true,
                &mut Inputs {
                    vowel: Some(28),
                    ..Inputs::default()
                },
            )
            .unwrap();
        assert_eq!(result.parameters[10], 32);
        assert_eq!(result.parameters[4], 10);
        assert_eq!(result.parameters[7], 17);
        assert_eq!(result.sound_addresses[0], (1 << 18) + (0x1234 << 2));
        assert_eq!(result.sound_parameters[0], 255);
        assert_eq!(result.sound_parameters[4], 255);
        let terminal = bytes(&[0, 0xbff1, 0x1234, 0xfff2, 0x5678]);
        assert_eq!(
            Program::new(&terminal)
                .unwrap()
                .interpret(&phoneme(), 0, false, &mut Inputs::default())
                .unwrap()
                .sound_parameters[4],
            255
        );
        let changed = bytes(&[0, 0x0107, 0x0a20, 1]);
        let result = Program::new(&changed)
            .unwrap()
            .interpret(&phoneme(), 0x100, true, &mut Inputs::default())
            .unwrap();
        assert_eq!(result.parameters[1], 7);
        assert_eq!(result.parameters[10], 40); // change pass stops before SetLength
    }
    #[test]
    fn switch_tables_and_ipa_keep_legacy_skip_and_signedness() {
        let mut words = vec![0, 0x6a00];
        words.extend_from_slice(&[0xff1, 0x1234, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        words.extend_from_slice(&[0x0d03, 0xc9aa, 0x7800, 1]);
        let data = bytes(&words);
        let program = Program::new(&data).unwrap();
        assert_eq!(program.instruction_words(1).unwrap(), 12);
        let output = program
            .interpret(
                &phoneme(),
                0,
                false,
                &mut Inputs {
                    vowel: Some(28),
                    ..Inputs::default()
                },
            )
            .unwrap();
        assert_eq!(output.control, 2);
        assert_eq!(output.sound_addresses[2], (1 << 18) + (0x1234 << 2));
        assert_eq!(output.sound_parameters[2], -1);
        assert_eq!(&output.ipa[..5], &[0xc9, 0xaa, b'x', 0, 0]);
        // Missing neighbouring phonemes advance over only the switch opcode.
        let missing = bytes(&[0, 0x6a00, 1]);
        assert_eq!(
            Program::new(&missing)
                .unwrap()
                .interpret(&phoneme(), 0, false, &mut Inputs::default())
                .unwrap()
                .control,
            2
        );
    }
    #[test]
    fn truncated_programs_and_cycles_are_bounded_without_host_access() {
        assert!(Program::new(&[1]).is_err());
        for words in [
            &[0, 0x9100][..],
            &[0, 0xb001][..],
            &[0, 0xa100, 0][..],
            &[0, 0x2601][..],
            &[0, 0x0d10, 0][..],
        ] {
            let data = bytes(words);
            assert!(Program::new(&data)
                .unwrap()
                .interpret(&phoneme(), 0, false, &mut Inputs::default())
                .is_err());
        }
        let looped = bytes(&[0, 0x6000]);
        assert_eq!(
            Program::new(&looped)
                .unwrap()
                .interpret(&phoneme(), 0, false, &mut Inputs::default())
                .unwrap_err(),
            Error("phoneme program instruction budget exceeded")
        );
        let recursive = bytes(&[0, 0x9100, 1, 1]);
        assert!(Program::new(&recursive)
            .unwrap()
            .interpret(&phoneme(), 0, false, &mut Inputs::default())
            .is_ok());
    }
}
