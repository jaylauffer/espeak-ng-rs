//! Allocation-free dictionary control for thousands-name pronunciation.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::fmt::{self, Write};

pub const PHONEME_BYTES: usize = 200;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Key,
    Phonemes,
    Capacity,
}

/// Serialized dictionary/state primitives. Each projection is read freshly;
/// no language or foreign output borrow survives a dictionary operation.
pub trait Host {
    fn lookup(&mut self, key: &[u8], phonemes: &mut [u8; PHONEME_BYTES]) -> i32;
    fn numbers(&self) -> i32;
    fn variants(&self) -> i32;
    fn control(&self) -> i32;
    fn missing(&mut self, value: i32);
}

struct Key {
    bytes: [u8; 32],
    length: usize,
}
impl Write for Key {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        if end >= self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.length..end].copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
fn key(value: i32, plex: i32, suffix: &str, variant: Option<i32>) -> Result<Key, Error> {
    let mut key = Key {
        bytes: [0; 32],
        length: 0,
    };
    if let Some(options) = variant {
        let variant = crate::number_primitives::thousands_variant(value, options);
        let variant = std::str::from_utf8(&variant[..variant.len() - 1]).map_err(|_| Error::Key)?;
        write!(key, "_{variant}{plex}{suffix}")
    } else {
        write!(key, "_{value}M{plex}{suffix}")
    }
    .map_err(|_| Error::Key)?;
    Ok(key)
}
fn length(bytes: &[u8], limit: usize) -> Result<usize, Error> {
    bytes
        .iter()
        .take(limit)
        .position(|byte| *byte == 0)
        .ok_or(Error::Phonemes)
}
fn lookup<H: Host>(
    host: &mut H,
    key: &[u8],
    output: &mut [u8; PHONEME_BYTES],
    limit: usize,
) -> Result<i32, Error> {
    let found = host.lookup(key, output);
    length(output, limit)?;
    Ok(found)
}
fn name<H: Host>(
    host: &mut H,
    value: i32,
    plex: i32,
    suffix: &str,
    variant: bool,
    output: &mut [u8; PHONEME_BYTES],
) -> Result<i32, Error> {
    let key = key(value, plex, suffix, variant.then(|| host.variants()))?;
    lookup(host, &key.bytes[..=key.length], output, 160)
}

/// Preserve exact-value/ordinal/e/x lookup order and missing-power state.
/// Output publication is atomic on span/phoneme failure. Already executed
/// dictionary operations and missing-state stores are not replayed or undone.
pub fn thousands<H: Host>(
    host: &mut H,
    value: i32,
    plex: i32,
    exact: i32,
    output: &mut [u8],
) -> Result<i32, Error> {
    let mut phonemes = [0; PHONEME_BYTES];
    let mut of = [0; PHONEME_BYTES];
    let mut lower = [0; PHONEME_BYTES];
    let mut found_value = 0;
    if value > 0 {
        if exact & 1 != 0 {
            if exact & 2 != 0 {
                found_value = name(host, value, plex, "o", false, &mut phonemes)?;
            }
            if found_value == 0 && host.control() & 1 != 0 {
                found_value = name(host, value, plex, "e", false, &mut phonemes)?;
            }
            if found_value == 0 {
                found_value = name(host, value, plex, "x", false, &mut phonemes)?;
            }
        }
        if found_value == 0 {
            found_value = name(host, value, plex, "", false, &mut phonemes)?;
        }
    }
    if found_value == 0 {
        if value % 100 >= 20 {
            lookup(host, b"_0of\0", &mut of, 12)?;
        }
        let mut found = 0;
        if exact & 1 != 0 {
            if exact & 2 != 0 {
                found = name(host, value, plex, "o", true, &mut phonemes)?;
            }
            if found == 0 && host.control() & 1 != 0 {
                found = name(host, value, plex, "e", true, &mut phonemes)?;
            }
            if found == 0 {
                found = name(host, value, plex, "x", true, &mut phonemes)?;
            }
        }
        if found == 0 && name(host, value, plex, "", true, &mut phonemes)? == 0 {
            if plex > 3 && name(host, 0, plex - 1, "", false, &mut lower)? == 0 {
                lookup(host, b"_0M2\0", &mut phonemes, 160)?;
                host.missing(3);
            }
            if phonemes[0] == 0 {
                found_value = name(host, value, 1, "", false, &mut phonemes)?;
                if found_value == 0 {
                    lookup(host, b"_0M1\0", &mut phonemes, 160)?;
                }
                host.missing(2);
            }
        }
    }
    let of_len = length(&of, 12)?;
    let ph_len = length(&phonemes, 160)?;
    let total = of_len + ph_len;
    if total >= output.len() {
        return Err(Error::Capacity);
    }
    output[..of_len].copy_from_slice(&of[..of_len]);
    output[of_len..total].copy_from_slice(&phonemes[..ph_len]);
    output[total] = 0;
    Ok(
        if value == 1 && plex == 1 && host.numbers() & 0x0020_0000 != 0 {
            1
        } else {
            found_value
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        entries: Vec<(&'static [u8], &'static [u8], i32)>,
        trace: Vec<Vec<u8>>,
        missing: i32,
        unterminated: bool,
    }
    impl Host for Fixture {
        fn lookup(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) -> i32 {
            self.trace.push(key.to_vec());
            if self.unterminated {
                output.fill(31);
                return 1;
            }
            output[0] = 0;
            for &(name, phonemes, found) in &self.entries {
                if name == key {
                    output[..phonemes.len()].copy_from_slice(phonemes);
                    return found;
                }
            }
            0
        }
        fn numbers(&self) -> i32 {
            0
        }
        fn variants(&self) -> i32 {
            0
        }
        fn control(&self) -> i32 {
            1
        }
        fn missing(&mut self, value: i32) {
            self.missing = value;
        }
    }
    fn fixture(entries: Vec<(&'static [u8], &'static [u8], i32)>) -> Fixture {
        Fixture {
            entries,
            trace: Vec::new(),
            missing: -9,
            unterminated: false,
        }
    }
    #[test]
    fn thousands_retains_lower_probe_and_fallback_return_policy() {
        let mut host = fixture(vec![
            (b"_0of\0", b"of\0", 1),
            (b"_0M3\0", b"lower\0", 1),
            (b"_20M1\0", b"value\0", -7),
        ]);
        let mut output = [97; 50];
        assert_eq!(thousands(&mut host, 20, 4, 3, &mut output), Ok(-7));
        assert_eq!(&output[..8], b"ofvalue\0");
        assert!(output[8..].iter().all(|byte| *byte == 97));
        assert_eq!(host.missing, 2);
        let expected: &[&[u8]] = &[
            b"_20M4o\0",
            b"_20M4e\0",
            b"_20M4x\0",
            b"_20M4\0",
            b"_0of\0",
            b"_0M4o\0",
            b"_0M4e\0",
            b"_0M4x\0",
            b"_0M4\0",
            b"_0M3\0",
            b"_20M1\0",
        ];
        assert_eq!(
            host.trace.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            expected
        );
    }
    #[test]
    fn thousands_rejects_bad_phonemes_and_capacity_before_publication() {
        let mut host = fixture(vec![(b"_1M1\0", b"abc\0", 2)]);
        let mut short = [97; 3];
        assert_eq!(
            thousands(&mut host, 1, 1, 0, &mut short),
            Err(Error::Capacity)
        );
        assert_eq!(short, [97; 3]);
        let mut exact = [97; 4];
        assert_eq!(thousands(&mut host, 1, 1, 0, &mut exact), Ok(2));
        assert_eq!(&exact, b"abc\0");
        host.unterminated = true;
        let mut output = [97; 200];
        assert_eq!(
            thousands(&mut host, i32::MAX, i32::MIN, 3, &mut output),
            Err(Error::Phonemes)
        );
        assert_eq!(output, [97; 200]);
        assert_eq!(host.trace.last().unwrap(), b"_2147483647M-2147483648o\0");
    }
}
