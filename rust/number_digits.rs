//! Native tens/units, hundreds and nested thousands pronunciation control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_lookup::{self, Error, PHONEME_BYTES};
use std::fmt::{self, Write};

const NUM_SWAP_TENS: i32 = 0x10;
const NUM_AND_UNITS: i32 = 0x20;
const NUM_HUNDRED_AND: i32 = 0x40;
const NUM_SINGLE_AND: i32 = 0x80;
const NUM_SINGLE_STRESS: i32 = 0x100;
const NUM_SINGLE_VOWEL: i32 = 0x200;
const NUM_OMIT_1_HUNDRED: i32 = 0x400;
const NUM_1900: i32 = 0x800;
const NUM_AND_HUNDRED: i32 = 0x40000;
const NUM_THOUSAND_AND: i32 = 0x80000;
const NUM_VIGESIMAL: i32 = 0x100000;
const NUM_ZERO_HUNDRED: i32 = 0x400000;
const NUM_HUNDRED_AND_DIGIT: i32 = 0x800000;
const NUM_SINGLE_STRESS_L: i32 = 0x10000000;
const NUM2_SWAP_THOUSANDS: i32 = 0x200;
const NUM2_ORDINAL_NO_AND: i32 = 0x800;
const NUM2_MULTIPLE_ORDINAL: i32 = 0x1000;
const NUM2_NO_TEEN_ORDINALS: i32 = 0x2000;
const NUM2_MYRIADS: i32 = 0x4000;
const NUM2_OMIT_1_HUNDRED_ONLY: i32 = 0x20000;
const NUM2_ORDINAL_AND_THOUSANDS: i32 = 0x40000;
const NUM2_ORDINAL_DROP_VOWEL: i32 = 0x80000;
const NUM2_ZERO_TENS: i32 = 0x100000;

pub trait Host: number_lookup::Host {
    fn numbers2(&self) -> i32;
    fn digit_count(&self) -> i32;
    fn language(&self) -> i32;
    /// Fresh terminated text: cached digits (0), ordinal (1), alternate (2).
    fn text(&self, kind: u32, output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error>;
    fn phoneme_type(&self, code: u8) -> Result<i32, Error>;
}

pub(crate) struct Buffer {
    bytes: [u8; 211],
    length: usize,
    capacity: usize,
}
impl Buffer {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            bytes: [0; 211],
            length: 0,
            capacity,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.length = 0;
        self.bytes[0] = 0;
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub(crate) fn assign(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::Phonemes)?;
        if length >= self.capacity {
            return Err(Error::Capacity);
        }
        self.clear();
        self.append(&bytes[..length])
    }
    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::Capacity)?;
        if end >= self.capacity || end >= self.bytes.len() {
            return Err(Error::Capacity);
        }
        self.bytes[self.length..end].copy_from_slice(bytes);
        self.length = end;
        self.bytes[end] = 0;
        Ok(())
    }
    fn pop(&mut self) {
        if self.length > 0 {
            self.length -= 1;
            self.bytes[self.length] = 0;
        }
    }
    pub(crate) fn publish(&self, output: &mut [u8]) -> Result<(), Error> {
        if self.length >= output.len() {
            return Err(Error::Capacity);
        }
        output[..=self.length].copy_from_slice(&self.bytes[..=self.length]);
        Ok(())
    }
    pub(crate) fn terminated(&self) -> &[u8] {
        &self.bytes[..=self.length]
    }
    pub(crate) fn truncate(&mut self, length: usize) {
        self.length = self.length.min(length);
        self.bytes[self.length] = 0;
    }
}
impl Write for Buffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.append(text.as_bytes()).map_err(|_| fmt::Error)
    }
}
fn lookup<H: Host>(host: &mut H, key: &[u8], output: &mut Buffer) -> Result<i32, Error> {
    let mut scratch = [0; PHONEME_BYTES];
    let found = host.lookup(key, &mut scratch);
    output.assign(&scratch)?;
    Ok(found)
}
fn named<H: Host>(
    host: &mut H,
    value: i32,
    suffix: &str,
    output: &mut Buffer,
) -> Result<i32, Error> {
    let mut key = Buffer::new(32);
    write!(key, "_{value}{suffix}").map_err(|_| Error::Key)?;
    lookup(host, &key.bytes[..=key.length], output)
}
fn text<H: Host>(host: &H, kind: u32, capacity: usize) -> Result<Buffer, Error> {
    let mut scratch = [0; PHONEME_BYTES];
    host.text(kind, &mut scratch)?;
    let mut output = Buffer::new(capacity);
    output.assign(&scratch)?;
    Ok(output)
}
fn ordinal<H: Host>(host: &H) -> Result<Buffer, Error> {
    text(host, 1, 12)
}
fn ordinal_name(control: i32) -> &'static str {
    if control & 0x20 != 0 {
        "q"
    } else {
        "o"
    }
}

/// Dictionary order, cached digits, ordinal joins, vowel elision and stress.
/// Work in initialized bounded scratch; publish only a fully validated output.
pub fn two<H: Host>(
    host: &mut H,
    value: i32,
    plex: i32,
    control: i32,
    output: &mut [u8],
) -> Result<i32, Error> {
    if value < 0 {
        return Err(Error::Key);
    }
    let mut units = value % 10;
    let tens = value / 10;
    let is_ordinal = control & 1 != 0;
    let ord_type = ordinal_name(control);
    let mut ord = Buffer::new(20);
    let mut tens_ph = Buffer::new(50);
    let mut digits = Buffer::new(50);
    let mut and = Buffer::new(12);
    let mut result = Buffer::new(PHONEME_BYTES);
    let mut found = 0;
    let mut found_ordinal = 0;
    let mut used_and = 0;
    if control & 2 != 0 && host.digit_count() == 2 {
        result.append(text(host, 0, 50)?.bytes())?;
    } else {
        if text(host, 0, 50)?.length == 0 {
            if control & 8 != 0 {
                found = named(host, value, "fx", &mut digits)?;
                if found == 0 {
                    found = named(host, value, "f", &mut digits)?;
                }
            } else if is_ordinal {
                ord.append(ordinal(host)?.bytes())?;
                if control & 4 != 0 {
                    found = named(
                        host,
                        value,
                        if ord_type == "o" { "ox" } else { "qx" },
                        &mut digits,
                    )?;
                    if found != 0 {
                        let alternate = text(host, 2, 12)?;
                        if alternate.length != 0 {
                            ord.assign(&alternate.bytes)?;
                        }
                    }
                }
                if found == 0 {
                    found = named(host, value, ord_type, &mut digits)?;
                }
                found_ordinal = found;
            }
            if found == 0 {
                if control & 2 != 0 {
                    if host.control() & 1 != 0 {
                        found = named(host, value, "e", &mut digits)?;
                    }
                } else {
                    let suffix = if host.numbers2() & NUM2_ORDINAL_AND_THOUSANDS != 0 && plex <= 1 {
                        "o"
                    } else {
                        "a"
                    };
                    found = named(host, value, suffix, &mut digits)?;
                }
                if found == 0 && !(is_ordinal && host.numbers2() & NUM2_NO_TEEN_ORDINALS != 0) {
                    found = named(host, value, "", &mut digits)?;
                }
            }
        }
        if value < 10 && control & 0x10 != 0 {
            lookup(host, b"_0\0", &mut tens_ph)?;
        } else if found == 0 {
            if is_ordinal
                && named(
                    host,
                    tens,
                    if ord_type == "o" { "Xo" } else { "Xq" },
                    &mut tens_ph,
                )? != 0
            {
                found_ordinal = 1;
                if units != 0 && host.numbers2() & NUM2_MULTIPLE_ORDINAL != 0 {
                    tens_ph.append(ordinal(host)?.bytes())?;
                }
            }
            if found_ordinal == 0 {
                named(
                    host,
                    tens,
                    if control & 0x200 != 0 { "Xf" } else { "X" },
                    &mut tens_ph,
                )?;
            }
            if tens_ph.length == 0 && host.numbers() & NUM_VIGESIMAL != 0 {
                units = value % 20;
                named(host, tens & 0xfe, "X", &mut tens_ph)?;
            }
            digits.clear();
            if units > 0 {
                found = 0;
                let cached = if control & 2 != 0 {
                    Some(text(host, 0, 50)?)
                } else {
                    None
                };
                if cached.as_ref().is_some_and(|digits| digits.length != 0) {
                    digits.append(cached.unwrap().bytes())?;
                    found_ordinal = 1;
                    ord.clear();
                } else {
                    if control & 8 != 0 {
                        found = named(host, units, "f", &mut digits)?;
                    }
                    if is_ordinal && host.numbers() & NUM_SWAP_TENS == 0 {
                        found = named(host, units, ord_type, &mut digits)?;
                        if found != 0 {
                            found_ordinal = 1;
                        }
                    }
                    if found == 0 {
                        if host.control() & 1 != 0 && control & 2 != 0 {
                            found = named(host, units, "e", &mut digits)?;
                        } else if control & 2 == 0 || host.numbers() & NUM_SWAP_TENS != 0 {
                            let suffix =
                                if host.numbers2() & NUM2_ORDINAL_AND_THOUSANDS != 0 && plex <= 1 {
                                    "o"
                                } else {
                                    "a"
                                };
                            found = named(host, units, suffix, &mut digits)?;
                        }
                    }
                    if found == 0 {
                        named(host, units, "", &mut digits)?;
                    }
                }
            }
        }
        if is_ordinal && found_ordinal == 0 && ord.length == 0 {
            if value >= 20 && (value % 10 == 0 || host.numbers() & NUM_SWAP_TENS != 0) {
                lookup(host, b"_ord20\0", &mut ord)?;
            }
            if ord.length == 0 {
                lookup(host, b"_ord\0", &mut ord)?;
            }
        }
        if host.numbers() & (NUM_SWAP_TENS | NUM_AND_UNITS) != 0
            && tens_ph.length != 0
            && digits.length != 0
        {
            lookup(host, b"_0and\0", &mut and)?;
            if is_ordinal && host.numbers2() & NUM2_ORDINAL_NO_AND != 0 {
                and.clear();
            }
            let (first, last) = if host.numbers() & NUM_SWAP_TENS != 0 {
                (&digits, &tens_ph)
            } else {
                (&tens_ph, &digits)
            };
            result.append(first.bytes())?;
            result.append(and.bytes())?;
            result.append(last.bytes())?;
            result.append(ord.bytes())?;
            used_and = 1;
        } else {
            if host.numbers() & NUM_SINGLE_VOWEL != 0 && tens_ph.length != 0 && digits.length != 0 {
                let mut next = host.phoneme_type(digits.bytes[0])?;
                if next == 1 {
                    next = host.phoneme_type(digits.bytes[1])?;
                }
                if host.phoneme_type(tens_ph.bytes[tens_ph.length - 1])? == 2 && next == 2 {
                    tens_ph.pop();
                }
            }
            result.append(tens_ph.bytes())?;
            result.append(digits.bytes())?;
            if host.numbers2() & NUM2_ORDINAL_DROP_VOWEL != 0
                && ord.length != 0
                && result.length != 0
                && host.phoneme_type(result.bytes[result.length - 1])? == 2
            {
                result.pop();
            }
            result.append(ord.bytes())?;
        }
    }
    if host.numbers() & NUM_SINGLE_STRESS_L != 0 {
        let mut found = false;
        for byte in &mut result.bytes[..result.length] {
            if *byte == 6 {
                if found {
                    *byte = 5;
                } else {
                    found = true;
                }
            }
        }
    } else if host.numbers() & NUM_SINGLE_STRESS != 0 {
        let mut found = false;
        for byte in result.bytes[..result.length].iter_mut().rev() {
            if *byte == 6 {
                if found {
                    *byte = 5;
                } else {
                    found = true;
                }
            }
        }
    }
    result.publish(output)?;
    Ok(used_and)
}

fn feminine(options: i32, plex: i32) -> bool {
    (0..=3).contains(&plex) && options & (1 << plex) != 0
}

pub fn three<H: Host>(
    host: &mut H,
    value: i32,
    plex: i32,
    mut control: i32,
    mut suppress: bool,
    output: &mut [u8],
) -> Result<(), Error> {
    if value < 0 || plex < 0 {
        return Err(Error::Key);
    }
    let ordinal_bits = control & 0x22;
    let mut hundreds = value / 100;
    let units = value % 100;
    let mut first = Buffer::new(100);
    let mut last = Buffer::new(100);
    let mut thousand = Buffer::new(211);
    let mut thousand_and = Buffer::new(12);
    let mut hundred_and = Buffer::new(12);
    let mut hundred = Buffer::new(160);
    let mut digits = Buffer::new(50);
    let say_zero = host.numbers() & NUM_ZERO_HUNDRED != 0 && (control & 1 != 0 || hundreds >= 10);
    if hundreds > 0 || say_zero {
        let mut found = 0;
        if ordinal_bits != 0 && units == 0 {
            found = lookup(host, b"_0Co\0", &mut hundred)?;
        }
        if found == 0 {
            if units == 0 {
                found = lookup(host, b"_0C0\0", &mut hundred)?;
            }
            if found == 0 {
                lookup(host, b"_0C\0", &mut hundred)?;
            }
        }
        if !(host.numbers() & NUM_1900 != 0 && hundreds == 19) && hundreds >= 10 {
            let next = plex.checked_add(1).ok_or(Error::Key)?;
            let next = if host.numbers2() & NUM2_MYRIADS != 0 {
                0
            } else {
                next
            };
            let mut ten_thousand = [0; 160];
            if number_lookup::thousands(
                host,
                hundreds / 10,
                next,
                i32::from(value % 1000 == 0) | ordinal_bits,
                &mut ten_thousand,
            )? == 0
            {
                let mut x = if feminine(host.numbers2(), next) {
                    8
                } else {
                    0
                };
                if host.language() == 0x6d6c {
                    x = 0x208;
                }
                let mut scratch = [0; 50];
                two(host, hundreds / 10, plex, x, &mut scratch)?;
                digits.assign(&scratch)?;
            }
            let end = ten_thousand
                .iter()
                .position(|byte| *byte == 0)
                .ok_or(Error::Phonemes)?;
            let (a, b) = if host.numbers2() & NUM2_SWAP_THOUSANDS != 0 {
                (&ten_thousand[..end], digits.bytes())
            } else {
                (digits.bytes(), &ten_thousand[..end])
            };
            thousand.append(a)?;
            thousand.append(&[15])?;
            thousand.append(b)?;
            thousand.append(&[15])?;
            hundreds %= 10;
            if hundreds == 0 && !say_zero {
                hundred.clear();
            }
            suppress = true;
            control |= 1;
        }
        digits.clear();
        if hundreds > 0 || say_zero {
            if host.numbers() & NUM_AND_HUNDRED != 0 && (control & 1 != 0 || thousand.length != 0) {
                lookup(host, b"_0and\0", &mut thousand_and)?;
            }
            suppress = true;
            found = 0;
            if ordinal_bits != 0 && (units == 0 || host.numbers2() & NUM2_MULTIPLE_ORDINAL != 0) {
                found = named(host, hundreds, "Co", &mut digits)?;
                if host.numbers2() & NUM2_MULTIPLE_ORDINAL != 0 && units > 0 {
                    digits.append(ordinal(host)?.bytes())?;
                }
            }
            if hundreds == 0 && say_zero {
                lookup(host, b"_0\0", &mut digits)?;
            } else {
                if !(hundreds == 1
                    && host.numbers2() & NUM2_OMIT_1_HUNDRED_ONLY != 0
                    && control & 1 == 0)
                {
                    if found == 0 && units == 0 {
                        found = named(host, hundreds, "C0", &mut digits)?;
                    }
                    if found == 0 {
                        found = named(host, hundreds, "C", &mut digits)?;
                    }
                }
                if found != 0 {
                    hundred.clear();
                } else if hundreds != 1 || host.numbers() & NUM_OMIT_1_HUNDRED == 0 {
                    let mut scratch = [0; 50];
                    two(host, hundreds, plex, 0, &mut scratch)?;
                    digits.assign(&scratch)?;
                }
            }
        }
        first.append(thousand.bytes())?;
        first.append(thousand_and.bytes())?;
        first.append(digits.bytes())?;
        first.append(hundred.bytes())?;
    }
    if units > 0 && !(control & 2 != 0 && host.numbers2() & NUM2_MULTIPLE_ORDINAL != 0) {
        if (value > 100 || control & 1 != 0 && plex == 0)
            && (host.numbers() & NUM_HUNDRED_AND != 0
                || host.numbers() & NUM_HUNDRED_AND_DIGIT != 0 && units < 10)
        {
            lookup(host, b"_0and\0", &mut hundred_and)?;
        }
        if host.numbers() & NUM_THOUSAND_AND != 0
            && hundreds == 0
            && (control & 1 != 0 || thousand.length != 0)
        {
            lookup(host, b"_0and\0", &mut hundred_and)?;
        }
    }
    if units != 0 || !suppress {
        let mut x = 0;
        if plex == 0 {
            x = 2;
            if control & 0x400 != 0 {
                x |= 8;
            }
            if ordinal_bits != 0 {
                x = 3;
            }
            if value < 100 && control & 1 == 0 {
                x |= 4;
            }
            if ordinal_bits & 0x20 != 0 {
                x |= 0x20;
            }
        } else if feminine(host.numbers2(), plex) {
            x = 8;
        }
        if host.language() == 0x6d6c && plex == 1 {
            x |= 0x208;
        }
        if host.numbers2() & NUM2_ZERO_TENS != 0 && (control & 1 != 0 || hundreds > 0) {
            x |= 0x10;
        }
        let mut scratch = [0; 100];
        if two(host, units, plex, x | (control & 0x100), &mut scratch)? != 0
            && host.numbers() & NUM_SINGLE_AND != 0
        {
            hundred_and.clear();
        }
        last.assign(&scratch)?;
    } else {
        let suffix = ordinal(host)?;
        if suffix.length != 0 {
            if first.bytes().last() == Some(&10) {
                first.pop();
            }
            last.append(suffix.bytes())?;
        }
    }
    let mut result = Buffer::new(PHONEME_BYTES);
    result.append(first.bytes())?;
    result.append(hundred_and.bytes())?;
    result.append(&[15])?;
    result.append(last.bytes())?;
    result.publish(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        entries: Vec<(&'static [u8], &'static [u8], i32)>,
        trace: Vec<Vec<u8>>,
        numbers: i32,
        numbers2: i32,
        cached: [u8; 6],
        count: i32,
        ordinal: [u8; 3],
        absent_type: Option<u8>,
        missing: i32,
    }
    impl number_lookup::Host for Fixture {
        fn lookup(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) -> i32 {
            self.trace.push(key.to_vec());
            output[0] = 0;
            for &(name, phonemes, flags) in &self.entries {
                if name == key {
                    output[..phonemes.len()].copy_from_slice(phonemes);
                    return flags;
                }
            }
            0
        }
        fn numbers(&self) -> i32 {
            self.numbers
        }
        fn variants(&self) -> i32 {
            0
        }
        fn control(&self) -> i32 {
            0
        }
        fn missing(&mut self, value: i32) {
            self.missing = value;
        }
    }
    impl Host for Fixture {
        fn numbers2(&self) -> i32 {
            self.numbers2
        }
        fn digit_count(&self) -> i32 {
            self.count
        }
        fn language(&self) -> i32 {
            0x656e
        }
        fn text(&self, kind: u32, output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
            let text: &[u8] = match kind {
                0 => &self.cached,
                1 => &self.ordinal,
                _ => &[0],
            };
            output[..text.len()].copy_from_slice(text);
            Ok(())
        }
        fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
            if self.absent_type == Some(code) {
                return Err(Error::Phonemes);
            }
            Ok(match code {
                6 => 1,
                50 | 200 => 2,
                _ => 4,
            })
        }
    }
    fn fixture(entries: Vec<(&'static [u8], &'static [u8], i32)>) -> Fixture {
        Fixture {
            entries,
            trace: Vec::new(),
            numbers: 0,
            numbers2: 0,
            cached: [0; 6],
            count: 0,
            ordinal: [0; 3],
            absent_type: None,
            missing: -9,
        }
    }
    #[test]
    fn cached_digits_preserve_left_stress_precedence_and_skip_lookups() {
        let mut host = fixture(Vec::new());
        host.cached = [6, 50, 6, 21, 6, 0];
        host.count = 2;
        host.numbers = NUM_SINGLE_STRESS;
        let mut output = [97; 8];
        assert_eq!(two(&mut host, 21, 0, 3, &mut output), Ok(0));
        assert_eq!(&output[..6], &[5, 50, 5, 21, 6, 0]);
        host.numbers |= NUM_SINGLE_STRESS_L;
        assert_eq!(two(&mut host, 21, 0, 3, &mut output), Ok(0));
        assert_eq!(&output[..6], &[6, 50, 5, 21, 5, 0]);
        assert_eq!(&output[6..], &[97, 97]);
        assert!(host.trace.is_empty());
    }
    #[test]
    fn vowel_elision_uses_unsigned_codes_and_rejects_missing_slots() {
        let mut host = fixture(vec![(b"_2X\0", &[6, 50, 0], 1), (b"_1\0", &[6, 200, 0], 1)]);
        host.numbers = NUM_SINGLE_VOWEL;
        let mut output = [97; 8];
        assert_eq!(two(&mut host, 21, 0, 2, &mut output), Ok(0));
        assert_eq!(&output[..4], &[6, 6, 200, 0]);
        host.absent_type = Some(200);
        output.fill(97);
        assert_eq!(two(&mut host, 21, 0, 2, &mut output), Err(Error::Phonemes));
        assert_eq!(output, [97; 8]);
    }
    #[test]
    fn hundred_suffix_removes_short_pause_and_preserves_capacity_failures() {
        let mut host = fixture(vec![
            (b"_0C0\0", &[50, 10, 0], 1),
            (b"_2C0\0", &[51, 10, 0], 2),
        ]);
        host.ordinal = [52, 0, 0];
        let mut output = [97; 4];
        assert_eq!(three(&mut host, 200, 0, 0, false, &mut output), Ok(()));
        assert_eq!(output, [51, 15, 52, 0]);
        let mut short = [97; 3];
        assert_eq!(
            three(&mut host, 200, 0, 0, false, &mut short),
            Err(Error::Capacity)
        );
        assert_eq!(short, [97; 3]);
    }
    #[test]
    fn thousand_powers_bound_shifts_and_checked_increment() {
        let mut host = fixture(vec![(b"_1a\0", &[6, 50, 0], 1)]);
        host.numbers2 = i32::MAX;
        let mut output = [97; 8];
        assert_eq!(three(&mut host, 1, 32, 0, false, &mut output), Ok(()));
        assert_eq!(&output[..4], &[15, 6, 50, 0]);
        output.fill(97);
        assert_eq!(
            three(&mut host, 1000, i32::MAX, 0, false, &mut output),
            Err(Error::Key)
        );
        assert_eq!(output, [97; 8]);
    }
}
