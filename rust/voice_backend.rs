//! Bounded voice backend directives and phoneme replacement state.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{phoneme::mnemonic, phoneme_data::InvalidPhonemeData as Error, voice::decimal};

pub const MAX_REPLACEMENTS: usize = 60;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Replacement {
    pub old: u8,
    pub new: u8,
    pub flags: u8,
}
fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())]
}
fn token(bytes: &[u8]) -> (&[u8], &[u8]) {
    let start = bytes
        .iter()
        .position(|b| !matches!(b, b'\t'..=b'\r' | b' '))
        .unwrap_or(bytes.len());
    let bytes = &bytes[start..];
    let end = bytes
        .iter()
        .position(|b| matches!(b, b'\t'..=b'\r' | b' '))
        .unwrap_or(bytes.len());
    (&bytes[..end], &bytes[end..])
}

/// Owner resolves against the already selected native table. Unknown old
/// phonemes and full storage leave the count/storage unchanged; unknown new
/// phonemes encode deletion as zero. No allocation or I/O occurs here.
pub fn replace(
    storage: &mut [Replacement],
    count: &mut usize,
    value: &[u8],
    mut lookup: impl FnMut(u32) -> u8,
) -> Result<bool, Error> {
    if storage.len() > MAX_REPLACEMENTS || *count > storage.len() {
        return Err(Error("invalid phoneme replacement storage bound"));
    }
    if *count == storage.len() {
        return Ok(false);
    }
    let value = text(value);
    let Some((flags, consumed)) = decimal(value)? else {
        return Ok(false);
    };
    let (old, rest) = token(&value[consumed..]);
    if old.is_empty() {
        return Ok(false);
    }
    let (new, _) = token(rest);
    if old.len() >= 12 || new.len() >= 12 {
        return Err(Error("phoneme replacement token exceeds 11 bytes"));
    }
    let old = lookup(mnemonic(old));
    if old == 0 {
        return Ok(false);
    }
    let new = lookup(mnemonic(if new.is_empty() { b"NULL" } else { new }));
    storage[*count] = Replacement {
        old,
        new,
        flags: flags as u8,
    };
    *count += 1;
    Ok(true)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Mbrola {
    pub voice: [u8; 40],
    pub table: [u8; 80],
    pub sample_rate: i32,
}
impl Mbrola {
    /// Return a startup request. Loading the backend and its table belongs to
    /// initialization/the caller's bounded worker path, outside completions.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let (voice, rest) = token(text(bytes));
        let (table, rest) = token(rest);
        if voice.is_empty() || voice.len() >= 40 || table.len() >= 80 {
            return Err(Error("MBROLA voice/table name missing or exceeds bound"));
        }
        let mut request = Self {
            voice: [0; 40],
            table: [0; 80],
            sample_rate: 16000,
        };
        request.voice[..voice.len()].copy_from_slice(voice);
        request.table[..table.len()].copy_from_slice(table);
        if let Some((rate, _)) = decimal(rest)? {
            request.sample_rate = rate;
        }
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacements_preserve_partial_scans_and_bounds() {
        let mut storage = [Replacement::default(); 2];
        let mut count = 0;
        let lookup = |word| {
            if word == mnemonic(b"a") {
                10
            } else if word == mnemonic(b"b") {
                11
            } else {
                0
            }
        };
        assert!(replace(&mut storage, &mut count, b"257a b", lookup).unwrap());
        assert_eq!(
            storage[0],
            Replacement {
                old: 10,
                new: 11,
                flags: 1
            }
        );
        assert!(!replace(&mut storage, &mut count, b"1 unknown b", lookup).unwrap());
        let before = storage;
        assert!(replace(&mut storage, &mut count, b"999999999999 a", lookup).is_err());
        assert!(replace(&mut storage, &mut count, b"0 a overlongtoken", lookup).is_err());
        assert_eq!(storage, before);
        assert_eq!(count, 1);
        assert!(replace(&mut storage, &mut count, b"-1 a", lookup).unwrap());
        assert_eq!(
            storage[1],
            Replacement {
                old: 10,
                new: 0,
                flags: 255
            }
        );
        assert!(!replace(&mut storage, &mut count, b"0 a b", |_| panic!(
            "full table must not resolve"
        ))
        .unwrap());
    }
    #[test]
    fn mbrola_requests_retain_defaults_without_starting_backend() {
        let request = Mbrola::parse(b"en1 en1_phtrans +22050suffix").unwrap();
        assert_eq!(&request.voice[..4], b"en1\0");
        assert_eq!(request.sample_rate, 22050);
        assert_eq!(Mbrola::parse(b"en1").unwrap().sample_rate, 16000);
        assert_eq!(
            Mbrola::parse(b"en1 table invalid").unwrap().sample_rate,
            16000
        );
        assert!(Mbrola::parse(b"").is_err());
        assert!(Mbrola::parse(b"en1 table 9999999999999").is_err());
        assert!(Mbrola::parse(&[b'a'; 40]).is_err());
    }
}
