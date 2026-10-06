//! The engine's permissive forward/backward UTF-8 character reader.
// Copyright (C) 2005-2015 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Character {
    pub code: u32,
    /// Character bytes only; skipped continuation bytes are excluded.
    pub width: usize,
}
pub(crate) fn head(mut read: impl FnMut(usize) -> Option<u8>) -> Result<Character, Error> {
    let first = read(0).ok_or(Error::Bounds)?;
    let following = match first {
        0xc0..=0xdf => 1,
        0xe0..=0xef => 2,
        0xf0..=0xf7 => 3,
        _ => 0,
    };
    let mut code = u32::from(first & [0xff, 0x1f, 0x0f, 0x07][following]);
    let mut width = 1;
    for index in 1..=following {
        let byte = read(index).ok_or(Error::Bounds)?;
        if byte == 0 {
            break;
        }
        code = (code << 6) + u32::from(byte & 0x3f);
        width += 1;
    }
    Ok(Character { code, width })
}
/// Locate a non-continuation byte in the requested direction, then decode
/// forward. Nonzero tails need not be continuation bytes. A NUL truncates a
/// character and is not consumed as a tail. Surrogates, overlong encodings and
/// values above Unicode's limit retain their original numeric results.
/// Missing readable head/tail bytes reject rather than read outside the span.
pub fn decode(input: &[u8], mut position: usize, backwards: bool) -> Result<Character, Error> {
    loop {
        let byte = *input.get(position).ok_or(Error::Bounds)?;
        if byte & 0xc0 != 0x80 {
            break;
        }
        position = if backwards {
            position.checked_sub(1)
        } else {
            position.checked_add(1)
        }
        .ok_or(Error::Bounds)?;
    }
    head(|index| {
        position
            .checked_add(index)
            .and_then(|i| input.get(i))
            .copied()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_reader_retains_permissive_tails_and_character_width() {
        assert_eq!(
            decode(b"\x80\x81\xe2AB\0", 0, false),
            Ok(Character {
                code: 8258,
                width: 3
            })
        );
        assert_eq!(
            decode(b"\xe2\x80\x81\0", 2, true),
            Ok(Character {
                code: 8193,
                width: 3
            })
        );
        assert_eq!(
            decode(b"\xf7\xbf\xbf\xbf\0", 0, false),
            Ok(Character {
                code: 0x1fffff,
                width: 4
            })
        );
        assert_eq!(
            decode(b"\xe2\x82\0", 0, false),
            Ok(Character {
                code: 130,
                width: 2
            })
        );
        assert_eq!(
            decode(b"\xf8\0", 0, false),
            Ok(Character {
                code: 248,
                width: 1
            })
        );
        assert_eq!(decode(b"\0", 0, false), Ok(Character { code: 0, width: 1 }));
    }
    #[test]
    fn head_and_tail_extent_checks_reject_missing_storage() {
        assert_eq!(decode(b"\x80", 0, true), Err(Error::Bounds));
        assert_eq!(decode(b"\x80", 0, false), Err(Error::Bounds));
        assert_eq!(decode(b"\xe2\x82", 0, false), Err(Error::Bounds));
    }
}
