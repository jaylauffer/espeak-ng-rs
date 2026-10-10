//! Mnemonic text to selected-table phoneme codes, preserving legacy byte tails.
// Copyright (C) 2005-2015 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::phoneme::Phoneme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Terminator,
    Table,
    Capacity,
}

/// Snapshot of the platform's unsigned-byte whitespace/lowercase rules.
/// Encoding never invokes effects or allocates while planning/emitting.
pub struct Locale {
    pub space: [bool; 256],
    pub lower: [u8; 256],
}
impl Locale {
    pub fn ascii() -> Self {
        Self {
            space: std::array::from_fn(|i| matches!(i, 9..=13 | 32)),
            lower: std::array::from_fn(|i| (i as u8).to_ascii_lowercase()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Encoded {
    /// Byte cursor, including the legacy single-byte advance on bad UTF-8.
    pub cursor: usize,
    pub bad_phoneme: u32,
    /// Entire written footprint, including NUL and unused language tail bytes.
    pub footprint: usize,
}

fn visit(
    input: &[u8],
    records: &[Option<Phoneme>],
    default_voice: &[u8],
    locale: &Locale,
    mut write: impl FnMut(usize, u8),
) -> Encoded {
    let mut cursor = 0;
    let mut output = 0;
    let mut footprint = 0;
    let mut emit = |position: usize, byte| {
        write(position, byte);
        footprint = footprint.max(position + 1);
    };
    while input[cursor] != 0 && input[cursor] < 128 && locale.space[usize::from(input[cursor])] {
        cursor += 1;
    }
    while input[cursor] != 0 && !locale.space[usize::from(input[cursor])] {
        if input[cursor] == b'|' && input.get(cursor + 1) != Some(&b'|') {
            cursor += 1;
            continue;
        }
        let mut best = None;
        let mut code = 0;
        for record in records.iter().skip(1).flatten() {
            if record.kind == 15 {
                continue;
            }
            let mnemonic = record.mnemonic.to_le_bytes();
            let count = mnemonic
                .iter()
                .enumerate()
                .take_while(|(i, byte)| {
                    input
                        .get(cursor + i)
                        .is_some_and(|c| *c > b' ' && c == *byte)
                })
                .count();
            if best.is_none_or(|previous| count > previous) && (count == 4 || mnemonic[count] == 0)
            {
                best = Some(count);
                code = record.code;
            }
        }
        if code == 0 {
            emit(output, 0);
            return Encoded {
                cursor: cursor + 1,
                bad_phoneme: crate::word_key::decode(input, cursor).0,
                footprint,
            };
        }
        cursor += best.unwrap_or(0).max(1);
        emit(output, code);
        output += 1;
        if code == 21 {
            let language_input = cursor;
            let language_output = output;
            while input[cursor] != 0 && !locale.space[usize::from(input[cursor])] {
                emit(output, locale.lower[usize::from(input[cursor])]);
                output += 1;
                cursor += 1;
            }
            emit(output, 0);
            if input[cursor] == 0 {
                if input[language_input..cursor].len() == default_voice.len()
                    && input[language_input..cursor]
                        .iter()
                        .zip(default_voice)
                        .all(|(byte, expected)| locale.lower[usize::from(*byte)] == *expected)
                {
                    // The C encoder leaves the rest of the already-written
                    // language and its original NUL observable in the tail.
                    emit(language_output, 0);
                    return Encoded {
                        cursor,
                        bad_phoneme: 0,
                        footprint,
                    };
                }
            } else {
                emit(output, b'|');
                output += 1;
            }
        }
    }
    emit(output, 0);
    Encoded {
        cursor,
        bad_phoneme: 0,
        footprint,
    }
}

/// Immutable, allocation-free plan. First equal-length table entry wins;
/// sparse slots, invalid entries, empty mnemonics and record codes are retained.
pub struct Plan<'a> {
    input: &'a [u8],
    records: &'a [Option<Phoneme>],
    default_voice: &'a [u8],
    locale: &'a Locale,
    pub encoded: Encoded,
}
pub fn encode<'a>(
    input: &'a [u8],
    records: &'a [Option<Phoneme>],
    default_voice: &'a [u8],
    locale: &'a Locale,
    capacity: usize,
) -> Result<Plan<'a>, Error> {
    let end = input
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Terminator)?;
    if records.len() > 256 || default_voice.contains(&0) {
        return Err(Error::Table);
    }
    let input = &input[..=end];
    let encoded = visit(input, records, default_voice, locale, |_, _| {});
    if encoded.footprint > capacity {
        return Err(Error::Capacity);
    }
    Ok(Plan {
        input,
        records,
        default_voice,
        locale,
        encoded,
    })
}
impl Plan<'_> {
    /// Only the admitted footprint is written; unused caller bytes are unread.
    pub fn emit(&self, write: impl FnMut(usize, u8)) {
        visit(
            self.input,
            self.records,
            self.default_voice,
            self.locale,
            write,
        );
    }
    pub fn write(&self, output: &mut [u8]) -> Result<Encoded, Error> {
        if self.encoded.footprint > output.len() {
            return Err(Error::Capacity);
        }
        self.emit(|position, byte| output[position] = byte);
        Ok(self.encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(text: &[u8], code: u8) -> Option<Phoneme> {
        Some(Phoneme {
            mnemonic: crate::phoneme::mnemonic(text),
            code,
            ..Phoneme::default()
        })
    }
    #[test]
    fn longest_match_first_tie_sparse_invalid_and_zero_mnemonic() {
        let records = [
            None,
            record(b"a", 5),
            None,
            record(b"ab", 9),
            record(b"ab", 10),
            Some(Phoneme {
                kind: 15,
                ..record(b"abc", 20).unwrap()
            }),
            record(b"", 7),
        ];
        let locale = Locale::ascii();
        let plan = encode(b" \tab|c z\0", &records, b"en", &locale, 4).unwrap();
        let mut output = [0xaa; 8];
        assert_eq!(
            plan.write(&mut output).unwrap(),
            Encoded {
                cursor: 6,
                bad_phoneme: 0,
                footprint: 3
            }
        );
        assert_eq!(output, [9, 7, 0, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa]);
    }
    #[test]
    fn default_language_tail_is_written_and_capacity_failure_is_atomic() {
        let records = [None, record(b"_^_", 21)];
        let locale = Locale::ascii();
        let mut output = [0xaa; 12];
        let plan = encode(b"_^_EN\0", &records, b"en", &locale, 4).unwrap();
        assert_eq!(plan.write(&mut output).unwrap().footprint, 4);
        assert_eq!(&output[..5], &[21, 0, b'n', 0, 0xaa]);
        let mut short = [0xcc; 3];
        assert_eq!(plan.write(&mut short), Err(Error::Capacity));
        assert_eq!(short, [0xcc; 3]);
        assert!(matches!(
            encode(b"_^_EN\0", &records, b"en", &locale, 3),
            Err(Error::Capacity)
        ));
        let plan = encode(b"_^_EN next\0", &records, b"en", &locale, 5).unwrap();
        plan.write(&mut output).unwrap();
        assert_eq!(&output[..5], &[21, b'e', b'n', b'|', 0]);
    }
    #[test]
    fn error_cursor_is_one_byte_and_utf8_reporting_is_permissive() {
        let locale = Locale::ascii();
        let records = [None, record(b"a", 4), record(b"||", 15)];
        for (text, cursor, code) in [
            (b"a\xc3\xa9\0".as_slice(), 2, 233),
            (b"a\xe2\0".as_slice(), 2, 2),
            (b"a\x80\xc3\xa9\0".as_slice(), 2, 233),
        ] {
            let plan = encode(text, &records, b"en", &locale, 2).unwrap();
            assert_eq!(plan.encoded.bad_phoneme, code);
            assert_eq!(plan.encoded.cursor, cursor);
            let mut output = [0xcc; 4];
            plan.write(&mut output).unwrap();
            assert_eq!(output, [4, 0, 0xcc, 0xcc]);
        }
        let plan = encode(b"a||a|\0", &records, b"en", &locale, 4).unwrap();
        let mut output = [0; 4];
        plan.write(&mut output).unwrap();
        assert_eq!(output, [4, 15, 4, 0]);
    }
    #[test]
    fn locale_high_byte_space_is_not_initial_ascii_space() {
        let mut locale = Locale::ascii();
        locale.space[0xa0] = true;
        let records = [None, record(b"a", 4)];
        assert_eq!(
            encode(b"\xa0a\0", &records, b"en", &locale, 1)
                .unwrap()
                .encoded
                .cursor,
            0
        );
        assert!(matches!(
            encode(b"a", &records, b"en", &locale, 1),
            Err(Error::Terminator)
        ));
        assert_eq!(
            encode(b"\0", &[], b"en", &locale, 1)
                .unwrap()
                .encoded
                .footprint,
            1
        );
    }
}
