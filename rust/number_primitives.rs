//! Native allocation-free number recognition and spelling primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
pub const WORD_BYTES: usize = 160;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Terminator,
    Capacity,
}

/// Preserve the two-pass spelling policy, including switch escapes and pauses.
/// Validate the final extent before modifying any byte; unused tails stay intact.
pub fn spelling(
    phonemes: &mut [u8],
    initial: bool,
    control: i32,
    chars: i32,
) -> Result<usize, Error> {
    let end = phonemes
        .iter()
        .position(|b| *b == 0)
        .ok_or(Error::Terminator)?;
    let stress = phonemes[..end]
        .iter()
        .enumerate()
        .filter(|(i, b)| **b == 6 && (*i == 0 || phonemes[*i - 1] != 21))
        .count();
    let removed = phonemes[..end]
        .iter()
        .enumerate()
        .filter(|(i, b)| **b == 255 && (control < 2 || *i == 0))
        .count();
    let length = end - removed + usize::from(control >= 2);
    if length >= phonemes.len() {
        return Err(Error::Capacity);
    }
    let (mut count, mut previous, mut output) = (0usize, 0u8, 0usize);
    for index in 0..end {
        let mut code = phonemes[index];
        if code == 6 && chars > 1 && previous != 21 {
            count += 1;
            if if initial {
                count > 1
            } else {
                count != stress && (count % 3 != 0 || count == stress.saturating_sub(1))
            } {
                code = 5;
            }
        } else if code == 255 {
            if control < 2 || index == 0 {
                continue;
            }
            code = if count % 3 == 0 || control > 2 {
                11
            } else {
                23
            };
        }
        phonemes[output] = code;
        output += 1;
        previous = code;
    }
    if control >= 2 {
        phonemes[output] = 11;
        output += 1;
    }
    phonemes[output] = 0;
    Ok(output)
}

pub fn thousands_variant(value: i32, options: i32) -> &'static [u8] {
    let teens = value % 100 > 10 && value % 100 < 20;
    match options & 0x1c0 {
        0x40 if !teens && value % 10 == 1 => b"1MA\0",
        0x40 | 0xc0 if !teens && (2..=4).contains(&(value % 10)) => b"0MA\0",
        0x80 if (2..=4).contains(&value) => b"0MA\0",
        0x100 if teens || value % 10 == 0 => b"0MB\0",
        0x100 if value % 10 == 1 => b"0MA\0",
        0x140 if !teens && value % 10 == 1 => b"1M\0",
        0x140 if !teens && (2..=4).contains(&(value % 10)) => b"0MA\0",
        _ => b"0M\0",
    }
}
pub fn hungarian_e(word: &[u8], thousandplex: i32, value: i32) -> bool {
    let byte = |index| word.get(index).copied().unwrap_or(0);
    matches!(byte(0), b'a' | b'e')
        && byte(1) != b' '
        && byte(1) != b'z'
        && !(byte(1) == b't' && byte(2) == b't')
        && !((thousandplex == 1 || value % 1000 == 0) && byte(1) == b'l')
}
/// Includes the preceding byte, the digit group, and its following byte.
pub fn thousands_group(frame: &[u8], digits: usize) -> bool {
    let Some(end) = digits.checked_add(1) else {
        return false;
    };
    frame.len() > end
        && !frame[0].is_ascii_digit()
        && !frame[end].is_ascii_digit()
        && frame[1..end].iter().all(u8::is_ascii_digit)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roman {
    pub value: i32,
    pub after: usize,
}
/// Engine lowercase Roman grammar, rather than a canonical numeral validator.
/// Input includes a space/NUL separator and one following initialized byte.
pub fn roman(
    input: &[u8],
    predecessor: u8,
    flags: u32,
    options: i32,
    min: i32,
    max: i32,
) -> Option<Roman> {
    if options & 0x02000000 != 0 && flags & 1 == 0 || predecessor.is_ascii_digit() {
        return None;
    }
    if input.get(1) == Some(&b' ')
        && !(options & (0x02000000 | 0x08000000 | 0x10000) != 0 && flags & 0x10000 != 0)
    {
        return None;
    }
    let (mut acc, mut previous, mut subtract, mut repeat) = (0i32, 0i32, 0x7fff, 0);
    for (index, code) in input.iter().copied().enumerate().take(WORD_BYTES) {
        if matches!(code, 0 | b' ') {
            let after = index + 1;
            if input.get(after)?.is_ascii_digit() {
                return None;
            }
            acc = acc.checked_add(previous)?;
            return (acc >= min && acc <= max).then_some(Roman { value: acc, after });
        }
        let mut value = match code {
            b'i' => 1,
            b'x' => 10,
            b'c' => 100,
            b'm' => 1000,
            b'v' => 5,
            b'l' => 50,
            b'd' => 500,
            _ => return None,
        };
        repeat = if value == previous { repeat + 1 } else { 0 };
        if repeat >= 3 || (previous > 1 && previous != 10 && previous != 100 && value >= previous) {
            return None;
        }
        if previous != 0 && previous < value {
            if acc % 10 != 0 || previous * 10 < value {
                return None;
            }
            subtract = previous;
            value -= subtract;
        } else if value >= subtract {
            return None;
        } else {
            acc = acc.checked_add(previous)?;
        }
        previous = value;
    }
    None
}

const DERIVED: [(i32, i32); 62] = [
    (0x00aa, 32865),
    (0x00b2, 32818),
    (0x00b3, 32819),
    (0x00b9, 32817),
    (0x00ba, 32879),
    (0x02b0, 32872),
    (0x02b1, 33382),
    (0x02b2, 32874),
    (0x02b3, 32882),
    (0x02b4, 33401),
    (0x02b5, 33403),
    (0x02b6, 33409),
    (0x02b7, 32887),
    (0x02b8, 32889),
    (0x02c0, 33428),
    (0x02c1, 33429),
    (0x02e0, 33379),
    (0x02e1, 32876),
    (0x02e2, 32883),
    (0x02e3, 32888),
    (0x2070, 32816),
    (0x2071, 32873),
    (0x2074, 32820),
    (0x2075, 32821),
    (0x2076, 32822),
    (0x2077, 32823),
    (0x2078, 32824),
    (0x2079, 32825),
    (0x207a, 32811),
    (0x207b, 32813),
    (0x207c, 32829),
    (0x207d, 32808),
    (0x207e, 32809),
    (0x207f, 32878),
    (0x2080, 16432),
    (0x2081, 16433),
    (0x2082, 16434),
    (0x2083, 16435),
    (0x2084, 16436),
    (0x2085, 16437),
    (0x2086, 16438),
    (0x2087, 16439),
    (0x2088, 16440),
    (0x2089, 16441),
    (0x208a, 16427),
    (0x208b, 16429),
    (0x208c, 16445),
    (0x208d, 16424),
    (0x208e, 16425),
    (0x2090, 16481),
    (0x2091, 16485),
    (0x2092, 16495),
    (0x2093, 16504),
    (0x2094, 16985),
    (0x2095, 16488),
    (0x2096, 16491),
    (0x2097, 16492),
    (0x2098, 16493),
    (0x2099, 16494),
    (0x209a, 16496),
    (0x209b, 16499),
    (0x209c, 16500),
];
pub fn superscript(letter: i32) -> i32 {
    DERIVED
        .binary_search_by_key(&letter, |(code, _)| *code)
        .map_or(0, |i| DERIVED[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spelling_capacity_failure_preserves_input_and_switches_escape_stress() {
        let mut full = [31; 200];
        full[199] = 0;
        let saved = full;
        assert_eq!(spelling(&mut full, false, 2, 5), Err(Error::Capacity));
        assert_eq!(full, saved);
        let mut bytes = [21, 6, 255, 6, 31, 6, 0, 97, 97];
        assert_eq!(spelling(&mut bytes, false, 0, 3), Ok(5));
        assert_eq!(&bytes[..6], &[21, 6, 5, 31, 6, 0]);
    }
    #[test]
    fn roman_keeps_engine_grammar_and_neighbor_constraints() {
        assert_eq!(
            roman(b"xiv  ", b' ', 1, 0, 1, 4000),
            Some(Roman {
                value: 14,
                after: 4
            })
        );
        for text in [b"mm  ".as_slice(), b"iix  ", b"ic  ", b"iiii  ", b"xx 7"] {
            assert!(roman(text, b' ', 1, 0, 1, 4000).is_none());
        }
        assert!(roman(b"x  ", b' ', 1, 0, 1, 4000).is_none());
        assert_eq!(
            roman(b"x  ", b' ', 1 | 0x10000, 0x10000, 1, 4000),
            Some(Roman {
                value: 10,
                after: 2
            })
        );
        assert!(roman(b"xx  ", b'2', 1, 0, 1, 4000).is_none());
        assert!(roman(b"xx", b' ', 1, 0, 1, 4000).is_none());
    }
    #[test]
    fn grouping_and_number_forms_respect_boundaries_and_signed_values() {
        assert!(thousands_group(b" 123 ", 3));
        assert!(!thousands_group(b"2123 ", 3));
        assert!(!thousands_group(b" 1234", 3));
        assert!(!thousands_group(b" 12", 3));
        assert_eq!(thousands_variant(11, 0x40), b"0M\0");
        assert_eq!(thousands_variant(21, 0x40), b"1MA\0");
        assert_eq!(thousands_variant(-11, 0x140), b"0M\0");
        assert!(!hungarian_e(b"el", 1, 1000));
        assert!(hungarian_e(b"al", 0, 21));
        assert!(!hungarian_e(b"ett", 0, 21));
        assert_eq!(superscript(0x2074), i32::from(b'4') + 0x8000);
        assert_eq!(superscript(0x110000), 0);
    }
}
