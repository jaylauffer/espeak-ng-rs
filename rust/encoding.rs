//! eSpeak's byte-oriented text decoding, including its legacy UTF-8 semantics.
// Copyright (C) 2017 Reece H. Dunn; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "encoding_data.rs"]
mod data;

/// Values agree with `espeak-ng/encoding.h`. Names are intentionally case-sensitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Encoding {
    Unknown,
    UsAscii,
    Iso8859_1,
    Iso8859_2,
    Iso8859_3,
    Iso8859_4,
    Iso8859_5,
    Iso8859_6,
    Iso8859_7,
    Iso8859_8,
    Iso8859_9,
    Iso8859_10,
    Iso8859_11,
    Iso8859_13,
    Iso8859_14,
    Iso8859_15,
    Iso8859_16,
    Koi8R,
    Iscii,
    Utf8,
    Ucs2,
}

const ENCODINGS: [Encoding; 21] = [
    Encoding::Unknown,
    Encoding::UsAscii,
    Encoding::Iso8859_1,
    Encoding::Iso8859_2,
    Encoding::Iso8859_3,
    Encoding::Iso8859_4,
    Encoding::Iso8859_5,
    Encoding::Iso8859_6,
    Encoding::Iso8859_7,
    Encoding::Iso8859_8,
    Encoding::Iso8859_9,
    Encoding::Iso8859_10,
    Encoding::Iso8859_11,
    Encoding::Iso8859_13,
    Encoding::Iso8859_14,
    Encoding::Iso8859_15,
    Encoding::Iso8859_16,
    Encoding::Koi8R,
    Encoding::Iscii,
    Encoding::Utf8,
    Encoding::Ucs2,
];

impl Encoding {
    pub fn from_name(name: &str) -> Self {
        data::ALIASES
            .iter()
            .find(|(alias, _)| *alias == name)
            .and_then(|(_, id)| Self::from_id(*id))
            .unwrap_or(Self::Unknown)
    }

    pub fn from_id(id: u32) -> Option<Self> {
        ENCODINGS.get(id as usize).copied()
    }

    fn codepage(self) -> Option<&'static [u16; 128]> {
        (self as usize)
            .checked_sub(2)
            .and_then(|id| data::CODEPAGES.get(id))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownEncoding;

impl std::fmt::Display for UnknownEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unknown text encoding")
    }
}
impl std::error::Error for UnknownEncoding {}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(feature = "c-abi"), allow(dead_code))]
pub(crate) enum Mode {
    Bytes,
    Auto,
    Wide16,
    Wide32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct State {
    pub(crate) offset: usize,
    pub(crate) encoding: Encoding,
    pub(crate) mode: Mode,
}

impl State {
    pub(crate) fn new(encoding: Encoding, mode: Mode) -> Result<Self, UnknownEncoding> {
        if encoding == Encoding::Unknown {
            return Err(UnknownEncoding);
        }
        Ok(Self {
            offset: 0,
            encoding,
            mode,
        })
    }

    pub(crate) fn read(&mut self, input: &[u8]) -> Option<u32> {
        if self.offset >= input.len() {
            return None;
        }
        if matches!(self.mode, Mode::Auto) {
            let start = self.offset;
            let c = self.utf8(input);
            if c != 0xfffd {
                return Some(c);
            }
            self.offset = start;
            self.mode = Mode::Bytes; // permanent fallback, as in eSpeak NG
                                     // AUTO's documented fallback is an 8-bit language codepage. For
                                     // non-codepage encodings, use ASCII instead of dereferencing NULL.
            if self.encoding.codepage().is_none() {
                self.encoding = Encoding::UsAscii;
            }
        }
        match self.mode {
            Mode::Wide16 => Some(self.wide(input, 2, false)),
            Mode::Wide32 => Some(self.wide(input, 4, false)),
            _ => match self.encoding {
                Encoding::Utf8 => Some(self.utf8(input)),
                Encoding::Ucs2 => Some(self.wide(input, 2, true)),
                _ => {
                    let c = input[self.offset];
                    self.offset += 1;
                    Some(if c < 0x80 {
                        u32::from(c)
                    } else {
                        self.encoding
                            .codepage()
                            .map_or(0xfffd, |page| u32::from(page[usize::from(c - 0x80)]))
                    })
                }
            },
        }
    }

    fn wide(&mut self, input: &[u8], width: usize, little_endian: bool) -> u32 {
        let Some(bytes) = input.get(self.offset..self.offset + width) else {
            self.offset = input.len();
            return 0xfffd;
        };
        self.offset += width;
        if width == 2 {
            let pair = [bytes[0], bytes[1]];
            u32::from(if little_endian {
                u16::from_le_bytes(pair)
            } else {
                u16::from_ne_bytes(pair)
            })
        } else {
            u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }
    }

    fn utf8(&mut self, input: &[u8]) -> u32 {
        let first = input[self.offset];
        self.offset += 1;
        let (tails, mut value) = match first {
            0x00..=0x7f => return u32::from(first),
            0x80..=0xbf => return 0xfffd,
            0xc0..=0xdf => (1, u32::from(first & 0x1f)),
            0xe0..=0xef => (2, u32::from(first & 0x0f)),
            _ => (3, u32::from(first & 0x0f)),
        };
        // Legacy decoding reserves a trailing byte, normally the string's NUL.
        // Keep this deliberately unusual rule for phoneme/audio parity.
        if input.len() - self.offset <= tails {
            self.offset = input.len();
            return 0xfffd;
        }
        for _ in 0..tails {
            let c = input[self.offset];
            if c & 0xc0 != 0x80 {
                return 0xfffd;
            } // leave invalid tail unread
            self.offset += 1;
            value = (value << 6) + u32::from(c & 0x3f);
        }
        if tails == 2 && value == 0xfffd {
            0x1a
        } else if value > 0x10ffff {
            0xfffd
        } else {
            value
        }
    }
}

/// An allocation-free decoder borrowing its input. For legacy UTF-8, include
/// the trailing NUL byte in the slice, just as the C API's negative length does.
/// Returns numeric codepoints: the legacy engine accepts some non-scalar values.
#[derive(Clone, Debug)]
pub struct Decoder<'a> {
    input: &'a [u8],
    state: State,
}

impl<'a> Decoder<'a> {
    pub fn new(input: &'a [u8], encoding: Encoding) -> Result<Self, UnknownEncoding> {
        Ok(Self {
            input,
            state: State::new(encoding, Mode::Bytes)?,
        })
    }

    pub fn auto(input: &'a [u8], fallback: Encoding) -> Result<Self, UnknownEncoding> {
        Ok(Self {
            input,
            state: State::new(fallback, Mode::Auto)?,
        })
    }

    pub fn position(&self) -> usize {
        self.state.offset
    }
    pub fn is_eof(&self) -> bool {
        self.position() == self.input.len()
    }

    /// Like the legacy API, peeking can permanently select the AUTO fallback.
    pub fn peek(&mut self) -> Option<u32> {
        let offset = self.state.offset;
        let c = self.next();
        self.state.offset = offset;
        c
    }
}

impl Iterator for Decoder<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.state.read(self.input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_utf8_boundaries_and_replacement() {
        assert_eq!(
            Decoder::new(b"\xc2\xa0", Encoding::Utf8)
                .unwrap()
                .collect::<Vec<_>>(),
            [0xfffd]
        );
        assert_eq!(
            Decoder::new(b"\xc2\xa0\0", Encoding::Utf8)
                .unwrap()
                .collect::<Vec<_>>(),
            [0xa0, 0]
        );
        assert_eq!(
            Decoder::new(b"\xe2\x93D!", Encoding::Utf8)
                .unwrap()
                .collect::<Vec<_>>(),
            [0xfffd, 68, 33]
        );
        assert_eq!(
            Decoder::new(b"\xef\xbf\xbd\0", Encoding::Utf8)
                .unwrap()
                .collect::<Vec<_>>(),
            [0x1a, 0]
        );
    }
    #[test]
    fn auto_fallback_is_permanent_and_peek_preserves_position() {
        let mut d = Decoder::auto(b"\xa0\xc2\xa0 ", Encoding::Iso8859_1).unwrap();
        assert_eq!(d.peek(), Some(0xa0));
        assert_eq!(d.position(), 0);
        assert_eq!(d.collect::<Vec<_>>(), [0xa0, 0xc2, 0xa0, 32]);
    }
    #[test]
    fn aliases_are_case_sensitive_and_eof_is_bounded() {
        assert_eq!(Encoding::from_name("csTIS620"), Encoding::Iso8859_11);
        assert_eq!(Encoding::from_name("utf-8"), Encoding::Unknown);
        let mut d = Decoder::new(b"a", Encoding::UsAscii).unwrap();
        assert_eq!(d.next(), Some(97));
        assert_eq!(d.next(), None);
        assert_eq!(d.next(), None);
        assert_eq!(d.peek(), None);
    }
}
