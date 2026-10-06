//! Owned clause input state and character preprocessing.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{encoding::Decoder, unicode};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Count,
    Table,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Cursor {
    pub pending: i32,
    pub count: i32,
}
impl Cursor {
    pub fn eof(&self, source_eof: bool) -> bool {
        self.pending == 0 && source_eof
    }
    /// A replayed character never increments the source count. Zero is the
    /// legacy empty slot. Count admission precedes any source advancement.
    pub fn read(&mut self, source: impl FnOnce() -> u32) -> Result<i32, Error> {
        if self.pending != 0 {
            let value = self.pending;
            self.pending = 0;
            return Ok(value);
        }
        let count = self.count.checked_add(1).ok_or(Error::Count)?;
        let value = source() as i32;
        self.count = count;
        Ok(value)
    }
    pub fn unread(&mut self, value: i32) {
        self.pending = value;
    }
}
/// An instance owns its replay/counter and decoder position while borrowing
/// immutable caller input. No allocation, I/O, polling or callbacks are needed.
pub struct Input<'a> {
    pub cursor: Cursor,
    pub decoder: Decoder<'a>,
}
impl Input<'_> {
    pub fn eof(&self) -> bool {
        self.cursor.eof(self.decoder.is_eof())
    }
    pub fn read(&mut self) -> Result<i32, Error> {
        self.cursor.read(|| self.decoder.next().unwrap_or(0))
    }
    /// The legacy peek bypasses replay and may select an AUTO fallback.
    pub fn peek(&mut self) -> u32 {
        self.decoder.peek().unwrap_or(0)
    }
}
pub fn clause_type(code: u32) -> i32 {
    clause_properties(unicode::properties(code, unicode::category(code)))
}
/// Only the engine's top twelve property bits participate in this exact
/// classification; unsupported combinations remain CLAUSE_NONE.
pub fn clause_properties(properties: u64) -> i32 {
    let bits = (properties >> 52) as u16;
    let (kind, flags) = match bits {
        0x800 => (0x80028, 0),
        0x804 => (0x80028, 0x8000),
        0x400 => (0x82028, 0),
        0x404 => (0x82028, 0x8000),
        0x402 => (0x82028, 0x100000),
        0x200 => (0x8302d, 0),
        0x204 => (0x8302d, 0x8000),
        0x202 => (0x8302d, 0x100000),
        0x100 => (0x41014, 0),
        0x104 => (0x41014, 0x8000),
        0x080 => (0x4001e, 0),
        0x084 => (0x4001e, 0x8000),
        0x040 | 0x008 => (0x4101e, 0),
        0x044 | 0x405 | 0x205 => (0x4101e, 0x8000),
        0x020 => (0x4101e, 0x208000),
        0x010 => (0x80046, 0),
        _ => (0x4000, 0),
    };
    kind | flags
}
pub fn roman_upper(code: u32) -> bool {
    matches!(code, 73 | 86 | 88 | 76)
}
pub fn language_word(word: u32) -> ([u8; 5], usize) {
    let mut text = [0; 5];
    let mut length = 0;
    for byte in word.to_be_bytes() {
        if byte != 0 {
            text[length] = byte;
            length += 1;
        }
    }
    (text, length)
}
pub fn phoneme_mode(enabled: i32, mode: i32, current: i32, next: i32) -> i32 {
    if enabled == 0 {
        mode
    } else if mode > 0 {
        mode - 1
    } else if current == 91 && next == 91 {
        -1
    } else if current == 93 && next == 93 {
        2
    } else {
        mode
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub code: i32,
    pub ignore: bool,
}
/// Borrow initialized key/value pairs followed by a zero key. First match
/// wins. Value one deletes; zero leaves the character unchanged.
pub fn replacement(table: &[u16], code: i32) -> Result<Replacement, Error> {
    let end = table
        .iter()
        .step_by(2)
        .position(|key| *key == 0)
        .ok_or(Error::Table)?;
    let mut result = Replacement {
        code,
        ignore: false,
    };
    for pair in table[..end * 2].chunks_exact(2) {
        if i32::from(pair[0]) == code {
            match pair[1] {
                0 => {}
                1 => result.ignore = true,
                value => result.code = i32::from(value),
            }
            break;
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::Encoding;
    #[test]
    fn replay_count_and_decoder_position_are_instance_owned() {
        let mut first = Input {
            cursor: Cursor {
                pending: 0,
                count: -1,
            },
            decoder: Decoder::auto(b"\xa0a\0", Encoding::Iso8859_1).unwrap(),
        };
        let second = Input {
            cursor: Cursor {
                pending: 0,
                count: -1,
            },
            decoder: Decoder::new(b"b\0", Encoding::Utf8).unwrap(),
        };
        assert_eq!(first.peek(), 0xa0);
        assert_eq!(first.decoder.position(), 0);
        assert_eq!(first.read(), Ok(0xa0));
        first.cursor.unread(42);
        assert_eq!(first.read(), Ok(42));
        assert_eq!(first.cursor.count, 0);
        assert_eq!(first.read(), Ok(97));
        assert_eq!(first.read(), Ok(0));
        assert!(first.eof());
        first.cursor.unread(88);
        assert!(!first.eof());
        assert_eq!(first.read(), Ok(88));
        assert!(first.eof());
        assert_eq!(second.cursor.count, -1);
        assert_eq!(second.decoder.position(), 0);
        first.cursor.count = i32::MAX;
        assert_eq!(first.read(), Err(Error::Count));
        assert_eq!(first.cursor.count, i32::MAX);
    }
    #[test]
    fn preprocessing_keeps_exact_properties_and_first_match_rules() {
        assert_eq!(clause_type(0x2026), 0x24901e);
        assert_eq!(clause_properties(0x8050000000000000), 0x4000);
        assert_eq!(language_word(0x61006200), ([97, 98, 0, 0, 0], 2));
        assert_eq!(
            replacement(&[5, 0, 5, 1, 0], 5),
            Ok(Replacement {
                code: 5,
                ignore: false
            })
        );
        assert_eq!(
            replacement(&[5, 1, 0], 5),
            Ok(Replacement {
                code: 5,
                ignore: true
            })
        );
        assert_eq!(
            replacement(&[5, 6, 0], 5),
            Ok(Replacement {
                code: 6,
                ignore: false
            })
        );
        assert_eq!(replacement(&[5, 6], 5), Err(Error::Table));
        assert_eq!(phoneme_mode(1, -1, 93, 93), 2);
        assert_eq!(phoneme_mode(1, 2, 91, 91), 1);
        assert!(!roman_upper(67));
        assert!(roman_upper(76));
    }
}
