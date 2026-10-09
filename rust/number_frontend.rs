//! Native number parsing, grouping, suffix/context and decimal pronunciation.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_digits::{self as digits, Buffer};
use crate::number_lookup::{self, Error, PHONEME_BYTES};
use crate::number_ordinal;
use std::fmt::Write;

const ALLOW_SPACE: i32 = 0x1000;
const NO_PAUSE: i32 = 0x20000;
const MYRIADS: i32 = 0x4000;
const SWAP_THOUSANDS: i32 = 0x200;
const PERCENT_BEFORE: i32 = 0x10000;
const FRACTION_FEMININE: i32 = 0x200000;
const NO_SPACE: u32 = 0x100;
const HYPHEN_AFTER: u32 = 0x4000;
const ORDINAL: u32 = 0x8000;
const MULTIPLE_SPACES: u32 = 0x40000;
const INDIVIDUAL_DIGITS: u32 = 0x80000;
const SKIP_WORDS: u32 = 0x80;
const FOUND: u32 = 0x80000000;

/// Serialized engine primitives, with an initialized source extent <=800 bytes,
/// three predecessor bytes and at least the current initialized word row.
/// Embedded NUL separators are initialized source bytes within that extent.
/// Outside that extent byte reads return virtual NUL. All source/state access
/// is scalar, so no foreign loan survives a dictionary or translation callback.
pub trait Host {
    fn byte(&self, offset: isize) -> u8;
    fn write_byte(&mut self, offset: usize, byte: u8);
    /// Options, global variants, numbers2, language, missing-thousands, say-as,
    /// decimal separator, thousands separator, previous dictionary, char signed.
    fn value(&self, field: u32) -> i32;
    fn word_flags(&self, index: usize) -> u32;
    fn lookup(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) -> i32;
    fn list(
        &mut self,
        offset: isize,
        output: &mut [u8; PHONEME_BYTES],
        flags: &mut [u32; 2],
    ) -> i32;
    /// Primary ordinal (0), alternate ordinal (1), optional indicator (2).
    fn text(&self, kind: u32, output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error>;
    fn store_text(&mut self, kind: u32, text: &[u8]) -> Result<(), Error>;
    /// Engine alphabetic, Unicode digit, Unicode alphabetic, byte whitespace.
    fn classify(&self, code: u32, kind: u32) -> bool;
    fn translate(&mut self, offset: usize) -> u32;
    fn missing(&mut self, value: i32);
    fn skip_words(&mut self, value: i32);
    fn phoneme_type(&self, code: u8) -> Result<i32, Error>;
}

struct Invocation<'a, H> {
    host: &'a mut H,
    cache: [u8; PHONEME_BYTES],
    count: i32,
    control: i32,
    remaining: usize,
}
impl<H: Host> number_lookup::Host for Invocation<'_, H> {
    fn lookup(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
        self.host.lookup(key, out)
    }
    fn numbers(&self) -> i32 {
        self.host.value(0)
    }
    fn variants(&self) -> i32 {
        self.host.value(1)
    }
    fn control(&self) -> i32 {
        self.control
    }
    fn missing(&mut self, value: i32) {
        self.host.missing(value);
    }
}
impl<H: Host> digits::Host for Invocation<'_, H> {
    fn numbers2(&self) -> i32 {
        self.host.value(2)
    }
    fn digit_count(&self) -> i32 {
        self.count
    }
    fn language(&self) -> i32 {
        self.host.value(3)
    }
    fn text(&self, kind: u32, out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        if kind == 0 {
            out.copy_from_slice(&self.cache);
            Ok(())
        } else {
            self.host.text(kind - 1, out)
        }
    }
    fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
        self.host.phoneme_type(code)
    }
}
impl<H: Host> number_ordinal::Host for Invocation<'_, H> {
    fn byte(&self, offset: isize) -> u8 {
        self.host.byte(offset)
    }
    fn space(&mut self, offset: usize) {
        self.host.write_byte(offset, b' ');
    }
    fn value(&self, field: u32) -> u32 {
        match field {
            0 => self.host.value(0) as u32,
            1 => self.host.value(3) as u32,
            2 => self.host.word_flags(0),
            3 => {
                if self.remaining > 1 {
                    self.host.word_flags(1)
                } else {
                    0
                }
            }
            _ => self.host.value(8) as u32,
        }
    }
    fn alpha(&self, c: u32) -> bool {
        self.host.classify(c, 0)
    }
    fn digit(&self, c: u32) -> bool {
        self.host.classify(c, 1)
    }
    fn translate(&mut self, offset: usize) -> u32 {
        self.host.translate(offset)
    }
}

fn byte(host: &impl Host, i: usize) -> u8 {
    host.byte(i as isize)
}
fn separator(host: &impl Host, b: u8, field: u32) -> bool {
    let c = if host.value(9) != 0 {
        i32::from(b as i8)
    } else {
        i32::from(b)
    };
    c == host.value(field)
}
fn decimal_value(host: &impl Host, mut offset: usize) -> Result<i32, Error> {
    let mut value = 0i32;
    while byte(host, offset).is_ascii_digit() {
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(i32::from(byte(host, offset) - b'0')))
            .ok_or(Error::Key)?;
        offset += 1;
    }
    Ok(value)
}
fn decoded(host: &impl Host, start: usize) -> (u32, usize) {
    let mut offset = start;
    while byte(host, offset) & 0xc0 == 0x80 {
        offset += 1;
    }
    let c = crate::utf8::head(|i| Some(byte(host, offset + i))).unwrap();
    (c.code, offset - start + c.width)
}
fn lookup(host: &mut impl Host, key: &[u8], out: &mut Buffer) -> Result<i32, Error> {
    let mut scratch = [0; PHONEME_BYTES];
    let found = host.lookup(key, &mut scratch);
    out.assign(&scratch)?;
    Ok(found)
}
fn named(
    host: &mut impl Host,
    prefix: &[u8],
    value: i32,
    suffix: &[u8],
    out: &mut Buffer,
) -> Result<i32, Error> {
    let mut key = Buffer::new(32);
    key.append(prefix)?;
    write!(key, "{value}").map_err(|_| Error::Key)?;
    key.append(suffix)?;
    lookup(host, key.terminated(), out)
}
fn three<H: Host>(
    inv: &mut Invocation<'_, H>,
    value: i32,
    plex: i32,
    ctl: i32,
    suppress: bool,
    out: &mut Buffer,
    capacity: usize,
) -> Result<(), Error> {
    let mut scratch = [0; PHONEME_BYTES];
    digits::three(inv, value, plex, ctl, suppress, &mut scratch[..capacity])?;
    out.assign(&scratch)
}
fn group(host: &impl Host, start: usize, size: usize) -> bool {
    let mut frame = [0; 6];
    frame[0] = host.byte(start as isize - 1);
    for (i, b) in frame[1..size + 2].iter_mut().enumerate() {
        *b = byte(host, start + i);
    }
    crate::number_primitives::thousands_group(&frame[..size + 2], size)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub recognized: bool,
    /// None leaves the caller's original output untouched.
    pub written: Option<usize>,
}
const DECLINED: Outcome = Outcome {
    recognized: false,
    written: None,
};

/// Own all main number and decimal modes. Intentional late decimal capacity
/// stops publish the valid partial pronunciation and footer, as the C engine
/// does. Undefined intermediate overflows fail before output publication. Rust
/// flags are working parsing state on error; the C ABI copies them and publishes
/// only on success. Applied primitive effects are not replayed or undone.
pub fn translate<H: Host>(
    host: &mut H,
    remaining: usize,
    control: i32,
    output: &mut [u8],
    flags: &mut [u32; 2],
) -> Result<Outcome, Error> {
    if output.is_empty() || output.len() > PHONEME_BYTES || remaining > 300 {
        return Err(Error::Capacity);
    }
    if host.value(5) == 0xc1 || host.word_flags(0) & INDIVIDUAL_DIGITS != 0 || host.value(0) == 0 {
        return Ok(DECLINED);
    }
    flags[0] = 0;
    let mut inv = Invocation {
        host,
        cache: [0; PHONEME_BYTES],
        count: 0,
        control,
        remaining,
    };
    let mut n = 0usize;
    while byte(inv.host, n).is_ascii_digit() {
        n += 1;
    }
    if n == 0 {
        return Ok(DECLINED);
    }
    let Ok(mut value) = decimal_value(inv.host, 0) else {
        return Ok(DECLINED);
    };
    let group_len = if inv.host.value(2) & MYRIADS != 0 {
        4
    } else {
        3
    };
    let previous = if n == group_len
        && separator(inv.host, inv.host.byte(-2), 7)
        && inv.host.byte(-3).is_ascii_digit()
    {
        true
    } else {
        (inv.host.value(7) == i32::from(b' ') || inv.host.value(0) & ALLOW_SPACE != 0)
            && n == 3
            && inv.host.word_flags(0) & MULTIPLE_SPACES == 0
            && inv.host.byte(-2).is_ascii_digit()
    };
    if !previous {
        inv.host.missing(0);
    }
    inv.host.store_text(0, b"\0")?;
    let mut ordinal = if previous || byte(inv.host, 0) != b'0' {
        number_ordinal::dot(&mut inv, n, false)
    } else {
        0
    };
    if byte(inv.host, n) == b'.'
        && !byte(inv.host, n + 1).is_ascii_digit()
        && !byte(inv.host, n + 2).is_ascii_digit()
        && (remaining <= 1 || inv.host.word_flags(1) & NO_SPACE == 0)
    {
        inv.host.write_byte(n, 0);
    }
    let mut skip = false;
    if ordinal == 0 || inv.host.value(3) == 0x6875 {
        let mut ix = n + 1;
        let mut suffix = Buffer::new(30);
        if inv.host.word_flags(0) & HYPHEN_AFTER != 0 {
            suffix.append(b"-")?;
            ix += 1;
        }
        // Legacy bound is the absolute source cursor, not suffix length.
        while !matches!(byte(inv.host, ix), 0 | b' ') && ix < 29 {
            suffix.append(&[byte(inv.host, ix)])?;
            ix += 1;
        }
        if !suffix.bytes().is_empty() {
            let mut scratch = [0; PHONEME_BYTES];
            inv.host.text(2, &mut scratch)?;
            let mut indicator = Buffer::new(32);
            indicator.assign(&scratch)?;
            if suffix.bytes() == indicator.bytes() {
                ordinal = 2;
            } else if !suffix.bytes()[0].is_ascii_digit() {
                let mut key = Buffer::new(32);
                key.append(b"_#")?;
                key.append(suffix.bytes())?;
                let mut phonemes = Buffer::new(12);
                let found = lookup(inv.host, key.terminated(), &mut phonemes)?;
                inv.host.store_text(0, phonemes.terminated())?;
                if found != 0 {
                    ordinal = 2;
                    flags[0] |= SKIP_WORDS;
                    skip = true;
                    key.clear();
                    key.append(b"_x#")?;
                    key.append(suffix.bytes())?;
                    lookup(inv.host, key.terminated(), &mut phonemes)?;
                    inv.host.store_text(1, phonemes.terminated())?;
                }
            }
        }
    }
    if inv.host.word_flags(0) & ORDINAL != 0 {
        ordinal = 2;
    }
    let mut zeros = Buffer::new(50);
    let mut append = Buffer::new(50);
    if byte(inv.host, 0) == b'0'
        && !previous
        && byte(inv.host, 1) != b' '
        && !separator(inv.host, byte(inv.host, 1), 6)
    {
        let time = n == 2
            && byte(inv.host, 3) == b':'
            && byte(inv.host, 5).is_ascii_digit()
            && inv.host.classify(u32::from(byte(inv.host, 7)), 3);
        if !time {
            if n > 3 {
                flags[0] &= !SKIP_WORDS;
                return Ok(DECLINED);
            }
            let mut i = 0;
            while byte(inv.host, i) == b'0' && i < n - 1 {
                let mut ph = Buffer::new(50);
                lookup(inv.host, b"_0\0", &mut ph)?;
                zeros.append(ph.bytes())?;
                i += 1;
            }
        }
    }
    let inc = if inv.host.value(0) & ALLOW_SPACE != 0 && byte(inv.host, n) == b' ' {
        1
    } else if separator(inv.host, byte(inv.host, n), 7) {
        2
    } else {
        0
    };
    let mut plex = 0usize;
    let mut exact = true;
    let mut suffix_ix = n + 2;
    if inc != 0 {
        let mut ix = n + inc;
        while plex + 1 < remaining
            && inv.host.word_flags(plex + 1) & MULTIPLE_SPACES == 0
            && group(inv.host, ix, group_len)
        {
            if (0..group_len).any(|i| byte(inv.host, ix + i) != b'0') {
                exact = false;
            }
            plex += 1;
            ix += group_len;
            if separator(inv.host, byte(inv.host, ix), 7)
                || (inv.host.value(0) & ALLOW_SPACE != 0 && byte(inv.host, ix) == b' ')
            {
                suffix_ix = ix + 2;
                ix += inc;
            } else {
                break;
            }
        }
    }
    let mut suppress = value == 0 && previous;
    if inv.host.value(3) == 0x6875
        && plex < remaining
        && inv.host.word_flags(plex) & HYPHEN_AFTER != 0
        && exact
    {
        let suffix = [
            byte(inv.host, suffix_ix),
            byte(inv.host, suffix_ix + 1),
            byte(inv.host, suffix_ix + 2),
        ];
        if crate::number_primitives::hungarian_e(&suffix, plex as i32, value) {
            inv.control |= 1;
        }
    }
    let mut decimal =
        separator(inv.host, byte(inv.host, n), 6) && byte(inv.host, n + 1).is_ascii_digit();
    if decimal {
        lookup(inv.host, b"_dpt\0", &mut append)?;
    } else if !suppress {
        if inc != 0 && plex != 0 {
            let mut scratch = [0; PHONEME_BYTES];
            let found = number_lookup::thousands(
                &mut inv,
                value,
                plex as i32,
                i32::from(exact),
                &mut scratch[..50],
            )?;
            append.assign(&scratch)?;
            if found != 0 {
                value = 0;
                suppress = true;
            }
        }
    } else if inv.host.value(4) == 1 {
        let mut scratch = Buffer::new(100);
        if named(inv.host, b"_0M", plex as i32 + 1, b"", &mut scratch)? == 0 {
            named(inv.host, b"_0M", plex as i32, b"", &mut append)?;
        }
    }
    if append.bytes().is_empty() && byte(inv.host, n) == b'.' && plex == 0 {
        lookup(inv.host, b"_.\0", &mut append)?;
    }
    let mut result = Buffer::new(output.len());
    if plex == 0 {
        let mut last = 0;
        while byte(inv.host, last + 1).is_ascii_digit() {
            last += 1;
        }
        if inv.host.byte(last as isize - 1).is_ascii_digit() {
            if inv.host.list(last as isize - 1, &mut inv.cache, flags) != 0 {
                inv.count = 2;
            }
            if !inv.cache[..50].contains(&0) {
                return Err(Error::Phonemes);
            }
        }
        if inv.cache[0] == 0 && byte(inv.host, last) != b'0' {
            if inv.host.list(last as isize, &mut inv.cache, flags) != 0 {
                inv.count = 1;
            }
            if !inv.cache[..50].contains(&0) {
                return Err(Error::Phonemes);
            }
        }
        if !previous {
            if !decimal && ordinal == 0 {
                if named(inv.host, b"_", value, b"n", &mut result)? != 0 {
                    result.publish(output)?;
                    return Ok(Outcome {
                        recognized: true,
                        written: Some(result.bytes().len()),
                    });
                }
                result.clear();
            }
            if inv.host.value(2) & PERCENT_BEFORE != 0 {
                let mut end = 0;
                while !matches!(byte(inv.host, end), 0 | b' ') {
                    end += 1;
                }
                if byte(inv.host, end + 1) == b'%' {
                    lookup(inv.host, b"%\0", &mut result)?;
                    inv.host.write_byte(end + 1, b' ');
                }
            }
        }
    }
    // Isolated-name misses write the initial output in C but the normal join
    // replaces it. Retain only an actual percent prefix before that join.
    let base = result.bytes().len();
    let mut phonemes = Buffer::new(200);
    three(
        &mut inv,
        value,
        plex as i32,
        i32::from(previous) | ordinal | if decimal { 0x100 } else { 0 },
        suppress,
        &mut phonemes,
        200,
    )?;
    result.append(zeros.bytes())?;
    if plex > 0 && inv.host.value(2) & SWAP_THOUSANDS != 0 {
        result.append(append.bytes())?;
        result.append(&[15])?;
        result.append(phonemes.bytes())?;
    } else {
        result.append(phonemes.bytes())?;
        result.append(&[15])?;
        result.append(append.bytes())?;
    }
    'fractions: while decimal {
        n += 1;
        let mut count = 0;
        while byte(inv.host, n + count).is_ascii_digit() {
            count += 1;
        }
        let mut fraction_suffix = false;
        let mode = inv.host.value(0) & 0xe000;
        let mut ph = Buffer::new(100);
        match mode {
            0x4000 | 0x8000 => {
                while byte(inv.host, n) == b'0' {
                    lookup(inv.host, b"_0\0", &mut ph)?;
                    if result.append(ph.bytes()).is_err() {
                        break 'fractions;
                    }
                    count -= 1;
                    n += 1;
                }
                if count <= if mode == 0x8000 { 5 } else { 2 } && byte(inv.host, n).is_ascii_digit()
                {
                    let value = decimal_value(inv.host, n)?;
                    three(&mut inv, value, 0, 0, false, &mut ph, 100)?;
                    if result.append(ph.bytes()).is_err() {
                        break 'fractions;
                    }
                    n += count;
                }
            }
            0x2000 | 0xa000 | 0xc000 => {
                value = decimal_value(inv.host, n)?;
                let ctl = if inv.host.value(2) & FRACTION_FEMININE != 0 {
                    0x400
                } else {
                    0
                };
                three(&mut inv, value, 0, ctl, false, &mut phonemes, 200)?;
                let mut suffix_found = true;
                if byte(inv.host, n) == b'0' || mode != 0x2000 {
                    ph.clear();
                    if inv.host.value(2) & FRACTION_FEMININE != 0
                        && value % 10 == 1
                        && value % 100 != 11
                    {
                        named(inv.host, b"_0Z", count as i32, b"s", &mut ph)?;
                    }
                    if ph.bytes().is_empty() {
                        named(inv.host, b"_0Z", count as i32, b"", &mut ph)?;
                    }
                    if ph.bytes().is_empty() {
                        suffix_found = false;
                    } else {
                        if mode == 0xc000 {
                            if result.append(ph.bytes()).is_err() {
                                break 'fractions;
                            }
                        } else {
                            phonemes.append(ph.bytes())?;
                        }
                        fraction_suffix = true;
                    }
                }
                if suffix_found {
                    if result.append(phonemes.bytes()).is_err() {
                        break 'fractions;
                    }
                    n += count;
                }
            }
            0x6000 => {
                if count <= 4 && byte(inv.host, n) != b'0' {
                    let value = decimal_value(inv.host, n)?;
                    three(&mut inv, value, 0, 0, false, &mut ph, 100)?;
                    if result.append(ph.bytes()).is_err() {
                        break 'fractions;
                    }
                    n += count;
                }
            }
            0xe000 => {
                while count > 1 {
                    count -= 1;
                    let key = [b'_', byte(inv.host, n), b'd', 0];
                    if lookup(inv.host, &key, &mut ph)? == 0 {
                        break;
                    }
                    n += 1;
                    if result.append(ph.bytes()).is_err() {
                        break 'fractions;
                    }
                }
            }
            _ => {}
        }
        let mut c = byte(inv.host, n);
        while c.is_ascii_digit() && result.bytes().len() - base < 190 {
            value = i32::from(c - b'0');
            n += 1;
            let mut scratch = [0; PHONEME_BYTES];
            digits::two(&mut inv, value, 0, 2, &mut scratch[..100])?;
            ph.assign(&scratch)?;
            // Admit the whole word separator and digit together, preserving
            // C's capacity stop before either byte span is appended.
            if result.bytes().len() + ph.bytes().len() + 2 > output.len() {
                break 'fractions;
            }
            result.append(&[15])?;
            result.append(ph.bytes())?;
            c = byte(inv.host, n);
        }
        if !fraction_suffix
            && lookup(inv.host, b"_dpt2\0", &mut ph)? != 0
            && result.append(ph.bytes()).is_err()
        {
            break 'fractions;
        }
        // C's decimal-loop cursor is unsigned char, unlike source comparisons.
        if i32::from(c) == inv.host.value(6) && byte(inv.host, n + 1).is_ascii_digit() {
            lookup(inv.host, b"_dpt\0", &mut ph)?;
            if result.append(ph.bytes()).is_err() {
                break 'fractions;
            }
        } else {
            decimal = false;
        }
    }
    if result
        .bytes()
        .get(base)
        .is_some_and(|c| !matches!(c, 0 | 21))
    {
        let (mut next, width) = decoded(inv.host, n + 1);
        if inv.host.value(0) & NO_PAUSE != 0 && next == u32::from(b' ') {
            next = decoded(inv.host, n + 1 + width).0;
        }
        if !inv.host.classify(next, 2) && !exact {
            if result.bytes().len() + 2 > output.len() {
                result.truncate(output.len().checked_sub(2).ok_or(Error::Capacity)?);
            }
            result.append(&[11])?;
        }
    }
    flags[0] |= FOUND;
    inv.host
        .missing(inv.host.value(4).checked_sub(1).ok_or(Error::Key)?);
    if skip {
        inv.host.skip_words(1);
    }
    result.publish(output)?;
    Ok(Outcome {
        recognized: true,
        written: Some(result.bytes().len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        source: [u8; 803],
        length: usize,
        values: [i32; 10],
        words: [u32; 3],
        texts: [[u8; PHONEME_BYTES]; 3],
        keys: Vec<Vec<u8>>,
        isolated: bool,
        cached: bool,
        reenter: bool,
        nested: bool,
        skips: i32,
    }
    impl Fixture {
        fn new(input: &[u8]) -> Self {
            let mut fixture = Self {
                source: [0x97; 803],
                length: input.len() + 1,
                values: [
                    1,
                    0,
                    0,
                    0x656e,
                    9,
                    0,
                    i32::from(b'.'),
                    i32::from(b','),
                    0,
                    1,
                ],
                words: [0; 3],
                texts: [[0; PHONEME_BYTES]; 3],
                keys: Vec::new(),
                isolated: false,
                cached: false,
                reenter: false,
                nested: false,
                skips: 0,
            };
            fixture.source[..3].copy_from_slice(b"   ");
            fixture.source[3..3 + input.len()].copy_from_slice(input);
            fixture.source[3 + input.len()] = 0;
            fixture
        }
    }
    impl Host for Fixture {
        fn byte(&self, offset: isize) -> u8 {
            if offset >= -3 && offset < self.length as isize {
                self.source[(offset + 3) as usize]
            } else {
                0
            }
        }
        fn write_byte(&mut self, offset: usize, value: u8) {
            assert!(offset < self.length);
            self.source[3 + offset] = value;
        }
        fn value(&self, field: u32) -> i32 {
            self.values[field as usize]
        }
        fn word_flags(&self, index: usize) -> u32 {
            self.words[index]
        }
        fn lookup(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
            self.keys.push(key.to_vec());
            if self.reenter && !self.nested && key == b"_2n\0" {
                self.nested = true;
                let source = self.source;
                let length = self.length;
                self.source[3..6].copy_from_slice(b"1 \0");
                self.length = 3;
                let mut nested_output = [0; 200];
                let mut flags = [0; 2];
                assert!(
                    translate(self, 1, 0, &mut nested_output, &mut flags)
                        .unwrap()
                        .recognized
                );
                assert_eq!(&nested_output[..4], &[15, 55, 15, 0]);
                self.source = source;
                self.length = length;
                self.nested = false;
            }
            let isolated_key = key.len() > 2 && key[key.len() - 2] == b'n';
            if (isolated_key && !self.isolated) || key == b"_0X\0" {
                out[0] = 0;
                return 0;
            }
            out[0] = 50;
            out[1] = 0;
            1
        }
        fn list(&mut self, _: isize, out: &mut [u8; PHONEME_BYTES], _: &mut [u32; 2]) -> i32 {
            out[0] = if self.cached {
                if self.nested {
                    55
                } else {
                    52
                }
            } else {
                0
            };
            out[1] = 0;
            i32::from(self.cached)
        }
        fn text(&self, kind: u32, out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
            out.copy_from_slice(&self.texts[kind as usize]);
            Ok(())
        }
        fn store_text(&mut self, kind: u32, text: &[u8]) -> Result<(), Error> {
            self.texts[kind as usize][..text.len()].copy_from_slice(text);
            Ok(())
        }
        fn classify(&self, code: u32, kind: u32) -> bool {
            code < 128
                && match kind {
                    0 | 2 => (code as u8).is_ascii_alphabetic(),
                    1 => (code as u8).is_ascii_digit(),
                    _ => (code as u8).is_ascii_whitespace(),
                }
        }
        fn translate(&mut self, _: usize) -> u32 {
            0
        }
        fn missing(&mut self, value: i32) {
            self.values[4] = value;
        }
        fn skip_words(&mut self, value: i32) {
            self.skips = value;
        }
        fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
            Ok(if code == 6 { 1 } else { 2 })
        }
    }
    #[test]
    fn digit_modes_and_long_initial_values_preserve_output_and_defined_flags() {
        let mut host = Fixture::new(b"2147483648 ");
        let mut output = [0x97; 200];
        let mut flags = [7, 9];
        host.values[5] = 0xc1;
        assert_eq!(
            translate(&mut host, 1, 0, &mut output, &mut flags),
            Ok(DECLINED)
        );
        assert_eq!(flags, [7, 9]);
        assert_eq!(host.values[4], 9);
        host.values[5] = 0;
        assert_eq!(
            translate(&mut host, 1, 0, &mut output, &mut flags),
            Ok(DECLINED)
        );
        assert_eq!(flags, [0, 9]);
        assert_eq!(output, [0x97; 200]);
        assert!(host.keys.is_empty());
        assert_eq!(host.values[4], 9);
    }
    #[test]
    fn isolated_name_bypasses_found_footer_and_missing_decrement() {
        let mut host = Fixture::new(b"2 ");
        host.isolated = true;
        let mut output = [0x97; 200];
        let mut flags = [7, 9];
        assert_eq!(
            translate(&mut host, 1, 0, &mut output, &mut flags)
                .unwrap()
                .written,
            Some(1)
        );
        assert_eq!(&output[..3], &[50, 0, 0x97]);
        assert_eq!(flags, [0, 9]);
        assert_eq!(host.values[4], 0);
    }
    #[test]
    fn decimal_capacity_stop_keeps_admitted_prefix_and_skips_decimal_suffix() {
        let mut host = Fixture::new(b"2.34 ");
        let mut output = [0x97; 5];
        let mut flags = [7, 9];
        assert_eq!(
            translate(&mut host, 1, 0, &mut output, &mut flags)
                .unwrap()
                .written,
            Some(4)
        );
        assert_eq!(output, [15, 50, 15, 50, 0]);
        assert_eq!(flags, [FOUND, 9]);
        assert!(host.keys.iter().any(|k| k == b"_3\0"));
        assert!(!host.keys.iter().any(|k| k == b"_dpt2\0"));
        assert_eq!(host.values[4], -1);
    }
    #[test]
    fn nonexact_group_pause_overwrites_last_byte_at_full_capacity() {
        let mut host = Fixture::new(b"1, 234 ");
        let mut output = [0x97; 4];
        let mut flags = [0, 9];
        assert!(
            translate(&mut host, 2, 0, &mut output, &mut flags)
                .unwrap()
                .recognized
        );
        assert_eq!(output, [15, 15, 11, 0]);
        assert_eq!(flags, [FOUND, 9]);
    }
    #[test]
    fn nested_number_lookup_keeps_parent_digit_cache_owned_and_live() {
        let mut host = Fixture::new(b"2 ");
        host.cached = true;
        host.reenter = true;
        let mut output = [0x97; 200];
        let mut flags = [0, 9];
        assert!(
            translate(&mut host, 1, 0, &mut output, &mut flags)
                .unwrap()
                .recognized
        );
        assert_eq!(&output[..4], &[15, 52, 15, 0]);
        assert_eq!(flags, [FOUND, 9]);
        assert!(host.keys.iter().any(|k| k == b"_1n\0"));
    }
    #[test]
    fn embedded_nul_thousands_separator_retains_initialized_following_groups() {
        let mut host = Fixture::new(b"12\0 345 ");
        host.values[7] = 0;
        let mut output = [0x97; 200];
        let mut flags = [0, 9];
        assert!(
            translate(&mut host, 2, 0, &mut output, &mut flags)
                .unwrap()
                .recognized
        );
        assert!(host.keys.iter().any(|k| k == b"_12M1\0"));
        assert_eq!(&output[..5], &[15, 15, 50, 11, 0]);
        assert_eq!(flags, [FOUND, 9]);
    }
}
