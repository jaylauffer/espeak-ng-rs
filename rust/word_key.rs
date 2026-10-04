//! Byte-exact dictionary alphabet transposition and six-bit compression.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::dictionary::InvalidDictionary;

/// The engine's UTF-8 helper accepts overlong/non-continuation bytes. Preserve
/// defined legacy decoding here; missing bytes act as a terminal NUL.
pub fn decode(bytes: &[u8], mut cursor: usize) -> (u32, usize) {
    while bytes.get(cursor).is_some_and(|b| b & 0xc0 == 0x80) {
        cursor += 1;
    }
    let first = bytes.get(cursor).copied().unwrap_or(0);
    let extra = match first {
        0xc0..=0xdf => 1,
        0xe0..=0xef => 2,
        0xf0..=0xf7 => 3,
        _ => 0,
    };
    let mut code = u32::from(first & [0xff, 0x1f, 0x0f, 0x07][extra]);
    let mut length = 1;
    for index in 1..=extra {
        let byte = bytes.get(cursor + index).copied().unwrap_or(0);
        if byte == 0 {
            break;
        }
        code = (code << 6) | u32::from(byte & 0x3f);
        length += 1;
    }
    (code, length)
}

pub struct Alphabet<'a> {
    pub min: u32,
    pub max: u32,
    pub map: Option<&'a [u8]>,
    /// Sorted pair codes, excluding the legacy 0x7fff sentinel.
    pub pairs: &'a [i16],
}
impl Alphabet<'_> {
    /// Buffer includes a NUL. Compression deliberately overwrites only the
    /// packed prefix: HashDictionary historically hashes the unchanged tail
    /// until the first NUL, while entry comparison uses the returned descriptor.
    /// Adding a new terminator would change the fork's compiled dictionary ABI.
    pub fn transpose(&self, text: &mut [u8]) -> Result<usize, InvalidDictionary> {
        let original_len = text
            .iter()
            .position(|b| *b == 0)
            .ok_or(InvalidDictionary("dictionary key lacks NUL"))?;
        if self.min == 0 || self.max < self.min || self.max > 0x10ffff {
            return Err(InvalidDictionary("invalid alphabet range"));
        }
        if self
            .map
            .is_some_and(|map| map.len() < (self.max - self.min + 1) as usize)
        {
            return Err(InvalidDictionary("short alphabet map"));
        }
        let mut codes = [0_u8; 161];
        let mut cursor = 0;
        let mut count = 0;
        loop {
            let (code, width) = decode(text, cursor);
            cursor += width;
            if code == 0 {
                break;
            }
            if code < self.min || code > self.max {
                return Ok(original_len);
            }
            let transposed = self.map.map_or((code - self.min + 1) as u8, |map| {
                map[(code - self.min) as usize]
            });
            // Language maps contain positive signed-char values. Zero rejects
            // compression; values >=128 are not valid map entries either.
            if self.map.is_some() && (transposed == 0 || transposed >= 128) {
                return Ok(original_len);
            }
            codes[count] = transposed;
            count += 1;
            if count == 160 {
                break;
            }
        }
        let pair_start = self.max - self.min + 2;
        let mut input = 0;
        let mut output = 0;
        let mut accumulator = 0_u32;
        let mut bits = 0;
        while input < count && codes[input] != 0 {
            let mut code = u32::from(codes[input]);
            input += 1;
            let pair = code as i32 + (i32::from(codes[input]) << 8);
            if let Some(index) = self
                .pairs
                .iter()
                .take_while(|value| pair >= i32::from(**value))
                .position(|value| pair == i32::from(*value))
            {
                code = index as u32 + pair_start;
                input += 1;
            }
            accumulator = accumulator.wrapping_shl(6).wrapping_add(code & 0x3f);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                codes[output] = (accumulator >> bits) as u8;
                output += 1;
            }
        }
        if bits > 0 {
            codes[output] = (accumulator << (8 - bits)) as u8;
            output += 1;
        }
        if output > original_len {
            return Err(InvalidDictionary("compressed key exceeds original storage"));
        }
        text[..output].copy_from_slice(&codes[..output]);
        Ok(output | 0x40)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packs_six_bit_codes_but_preserves_hash_tail_and_rejects_mixed_script() {
        let alphabet = Alphabet {
            min: 0x430,
            max: 0x451,
            map: None,
            pairs: &[0x010c],
        };
        let mut key = "ла\0".as_bytes().to_vec();
        let old = key.clone();
        assert_eq!(alphabet.transpose(&mut key).unwrap(), 0x41);
        assert_eq!(key[0], 35 << 2);
        assert_eq!(&key[1..], &old[1..]);
        let mut mixed = "лаx\0".as_bytes().to_vec();
        let old = mixed.clone();
        assert_eq!(alphabet.transpose(&mut mixed).unwrap(), 5);
        assert_eq!(mixed, old);
        let mut unterminated = *b"a";
        assert!(alphabet.transpose(&mut unterminated).is_err());
    }
}
