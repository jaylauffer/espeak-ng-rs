//! Native Roman pronunciation, ordinal forms and owned numeric source assembly.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_digits::Buffer;
use crate::number_lookup::{Error, PHONEME_BYTES};
use crate::{number_ordinal, number_primitives};
use std::fmt::Write;

const AFTER: u32 = 0x04000000;
const ORDINAL: u32 = 0x08000000;
const WORD_ORDINAL: u32 = 0x8000;
const HYPHEN_AFTER: u32 = 0x4000;

pub trait Host: number_ordinal::Host {
    fn lookup(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]);
    fn range(&self, maximum: bool) -> i32;
    fn suffix(&self, output: &mut [u8; 160]) -> Result<(), Error>;
    fn store_word_flags(&mut self, flags: u32);
    fn clear_previous(&mut self);
    /// Execute native number translation with owned initialized source storage,
    /// including its three predecessors and complete terminated text. Forward
    /// only dictionary/class/state primitives; no C number-controller replay.
    fn number(
        &mut self,
        source: &mut [u8; 160],
        initialized: usize,
        remaining: usize,
        control: i32,
        output: &mut [u8],
    ) -> Result<(), Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub recognized: bool,
    pub written: usize,
}

/// Preserve prefix output even on defined post-recognition declines. Bounds
/// failures preserve the entire caller output; applied source/state/dictionary
/// effects remain. Word flags and options are read freshly across callbacks.
pub fn translate<H: Host>(
    host: &mut H,
    remaining: usize,
    output: &mut [u8],
) -> Result<Outcome, Error> {
    if output.is_empty() || output.len() > PHONEME_BYTES || remaining > 300 {
        return Err(Error::Capacity);
    }
    let mut frame = [0; 161];
    for (i, byte) in frame.iter_mut().enumerate() {
        *byte = host.byte(i as isize);
    }
    let mut result = Buffer::new(output.len());
    let Some(roman) = number_primitives::roman(
        &frame,
        host.byte(-2),
        host.value(2),
        host.value(0) as i32,
        host.range(false),
        host.range(true),
    ) else {
        result.publish(output)?;
        return Ok(Outcome {
            recognized: false,
            written: 0,
        });
    };
    let mut phonemes = [0; PHONEME_BYTES];
    host.lookup(b"_roman\0", &mut phonemes);
    let mut name = Buffer::new(30);
    name.assign(&phonemes)?;
    let mut available = output.len();
    if host.value(0) & AFTER == 0 {
        result.append(name.bytes())?;
    } else {
        available = available
            .checked_sub(name.bytes().len())
            .ok_or(Error::Capacity)?;
    }
    let mut suffix = [0; 160];
    host.suffix(&mut suffix)?;
    let end = suffix.iter().position(|b| *b == 0).ok_or(Error::Phonemes)?;
    let mut text = Buffer::new(160);
    write!(text, "   {} ", roman.value).map_err(|_| Error::Capacity)?;
    text.append(&suffix[..end])?;
    text.append(b"    ")?;
    let mut control = 0;
    let mut recognized = false;
    if host.byte(roman.after as isize) != b'.' {
        let ordinal = number_ordinal::dot(host, roman.after, true);
        if ordinal != 0 {
            host.store_word_flags(host.value(2) | WORD_ORDINAL);
        }
        let mut admitted = true;
        if host.value(0) & ORDINAL != 0 {
            if host.value(1) == 0x6875 {
                if host.value(2) & WORD_ORDINAL == 0 {
                    let suffix = [
                        host.byte(roman.after as isize),
                        host.byte(roman.after as isize + 1),
                        host.byte(roman.after as isize + 2),
                    ];
                    if host.value(2) & HYPHEN_AFTER != 0
                        && number_primitives::hungarian_e(&suffix, 0, roman.value)
                    {
                        control |= 1;
                    } else {
                        admitted = false;
                    }
                }
            } else {
                host.store_word_flags(host.value(2) | WORD_ORDINAL);
            }
        }
        if admitted {
            host.clear_previous();
            let mut source = [0; 160];
            let initialized = text.terminated().len();
            source[..initialized].copy_from_slice(text.terminated());
            let mut combined = [0; PHONEME_BYTES];
            result.publish(&mut combined)?;
            let start = result.bytes().len();
            if available <= start {
                return Err(Error::Capacity);
            }
            host.number(
                &mut source,
                initialized,
                remaining,
                control,
                &mut combined[start..available],
            )?;
            result.assign(&combined)?;
            if host.value(0) & AFTER != 0 {
                result.append(name.bytes())?;
            }
            recognized = true;
        }
    }
    result.publish(output)?;
    Ok(Outcome {
        recognized,
        written: result.bytes().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        source: [u8; 200],
        values: [u32; 5],
        suffix: [u8; 160],
        name: [u8; PHONEME_BYTES],
        lookup_toggle: bool,
        child_toggle: bool,
        child_declines: bool,
        child: Vec<(Vec<u8>, usize, i32, usize)>,
        lookups: usize,
    }
    impl Fixture {
        fn new(text: &[u8]) -> Self {
            let mut source = [0; 200];
            source[..3].copy_from_slice(b"   ");
            source[3..3 + text.len()].copy_from_slice(text);
            let mut name = [0; PHONEME_BYTES];
            name[0] = 50;
            Self {
                source,
                values: [1, 0x656e, 0, 0, 919],
                suffix: [0; 160],
                name,
                lookup_toggle: false,
                child_toggle: false,
                child_declines: false,
                child: Vec::new(),
                lookups: 0,
            }
        }
    }
    impl number_ordinal::Host for Fixture {
        fn byte(&self, offset: isize) -> u8 {
            usize::try_from(offset + 3)
                .ok()
                .and_then(|i| self.source.get(i))
                .copied()
                .unwrap_or(0)
        }
        fn space(&mut self, offset: usize) {
            self.source[offset + 3] = b' ';
        }
        fn value(&self, field: u32) -> u32 {
            self.values[field as usize]
        }
        fn alpha(&self, code: u32) -> bool {
            code < 128 && (code as u8).is_ascii_alphabetic()
        }
        fn digit(&self, code: u32) -> bool {
            code < 128 && (code as u8).is_ascii_digit()
        }
        fn translate(&mut self, _: usize) -> u32 {
            panic!("Roman dot policy must bypass month translation");
        }
    }
    impl Host for Fixture {
        fn lookup(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) {
            assert_eq!(key, b"_roman\0");
            self.lookups += 1;
            out.copy_from_slice(&self.name);
            if self.lookup_toggle {
                self.values[0] ^= AFTER;
            }
        }
        fn range(&self, maximum: bool) -> i32 {
            if maximum {
                4999
            } else {
                1
            }
        }
        fn suffix(&self, out: &mut [u8; 160]) -> Result<(), Error> {
            out.copy_from_slice(&self.suffix);
            Ok(())
        }
        fn store_word_flags(&mut self, flags: u32) {
            self.values[2] = flags;
        }
        fn clear_previous(&mut self) {
            self.values[4] = 0;
        }
        fn number(
            &mut self,
            source: &mut [u8; 160],
            initialized: usize,
            remaining: usize,
            control: i32,
            out: &mut [u8],
        ) -> Result<(), Error> {
            assert_eq!(self.values[4], 0);
            assert_eq!(source[initialized - 1], 0);
            assert!(source[initialized..].iter().all(|b| *b == 0));
            self.child.push((
                source[..initialized].to_vec(),
                remaining,
                control,
                out.len(),
            ));
            source[3] = b'!';
            if self.child_toggle {
                self.values[0] ^= AFTER;
            }
            if !self.child_declines {
                if out.len() < 2 {
                    return Err(Error::Capacity);
                }
                out[..2].copy_from_slice(&[51, 0]);
            }
            Ok(())
        }
    }
    #[test]
    fn post_recognition_dot_decline_preserves_prefix_and_previous_state() {
        let mut host = Fixture::new(b"xx . next ");
        let mut out = [0x97; 20];
        assert_eq!(
            translate(&mut host, 2, &mut out),
            Ok(Outcome {
                recognized: false,
                written: 1
            })
        );
        assert_eq!(&out[..2], &[50, 0]);
        assert!(out[2..].iter().all(|b| *b == 0x97));
        assert!(host.child.is_empty());
        assert_eq!(host.values[4], 919);
        host.values[0] |= AFTER;
        assert_eq!(translate(&mut host, 2, &mut out).unwrap().written, 0);
    }
    #[test]
    fn hungarian_ordinal_admission_selects_e_form_with_owned_numeric_text() {
        let mut host = Fixture::new(b"xx en ");
        host.values[0] |= ORDINAL;
        host.values[1] = 0x6875;
        let mut out = [0x97; 20];
        assert!(!translate(&mut host, 2, &mut out).unwrap().recognized);
        assert!(host.child.is_empty());
        host.values[2] |= HYPHEN_AFTER;
        assert!(translate(&mut host, 2, &mut out).unwrap().recognized);
        assert_eq!(host.child, [(b"   20     \0".to_vec(), 2, 1, 19)]);
        assert_eq!(&host.source[3..5], b"xx");
        host.source[6..9].copy_from_slice(b"el ");
        host.source[3..5].copy_from_slice(b"mm");
        host.child.clear();
        host.values[4] = 919;
        assert!(!translate(&mut host, 2, &mut out).unwrap().recognized);
        assert!(host.child.is_empty());
        assert_eq!(host.values[4], 919);
    }
    #[test]
    fn callback_changes_keep_live_prefix_policy_and_ordinal_word_flags() {
        // A dot removed by clause preprocessing is retained in the word flags.
        let mut host = Fixture::new(b"xx next ");
        host.values[2] = 0x10000;
        host.values[3] = 0x2; // FIRST_UPPER does not suppress Roman dot ordinals.
        host.values[0] |= 0x10000;
        host.values[1] = 0x6875; // Roman context bypasses Hungarian month lookup.
        host.lookup_toggle = true;
        host.child_toggle = true;
        host.suffix[..2].copy_from_slice(b"o\0");
        let mut out = [0x97; 20];
        assert!(translate(&mut host, 2, &mut out).unwrap().recognized);
        assert_ne!(host.values[2] & WORD_ORDINAL, 0);
        assert_eq!(host.child[0], (b"   20 o    \0".to_vec(), 2, 0, 19));
        assert_eq!(&out[..2], &[51, 0]);
        let mut host = Fixture::new(b"xx ");
        host.child_toggle = true;
        assert!(translate(&mut host, 1, &mut out).unwrap().recognized);
        assert_eq!(&out[..4], &[50, 51, 50, 0]);
    }
    #[test]
    fn malformed_names_and_synthetic_capacity_reject_without_output_publication() {
        let mut host = Fixture::new(b"xx ");
        let mut out = [0x97; 20];
        host.name[..30].fill(51);
        assert!(translate(&mut host, 1, &mut out).is_err());
        assert_eq!(out, [0x97; 20]);
        assert_eq!(host.lookups, 1);
        host.name[1] = 0;
        host.suffix[..152].fill(b'a');
        assert_eq!(translate(&mut host, 1, &mut out), Err(Error::Capacity));
        assert_eq!(out, [0x97; 20]);
        assert!(host.child.is_empty());
        host.suffix[0] = 0;
        host.values[0] |= AFTER;
        assert_eq!(translate(&mut host, 1, &mut out[..1]), Err(Error::Capacity));
        assert_eq!(out, [0x97; 20]);
        host.child_declines = true;
        assert!(translate(&mut host, 1, &mut out).unwrap().recognized);
        assert_eq!(&out[..2], &[51, 0]);
    }
}
