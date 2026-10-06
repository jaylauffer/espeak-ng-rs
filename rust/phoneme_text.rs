//! Internal phoneme codes to legacy mnemonic text and clause wrappers.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{clause_input, phoneme::Phoneme};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Terminator,
    Character,
    Capacity,
}
fn visit(
    input: &[u8],
    lookup: &impl Fn(u8) -> Option<Phoneme>,
    alpha: &impl Fn(u32) -> bool,
    signed: bool,
    mut emit: impl FnMut(usize, &[u8]) -> Result<(), Error>,
) -> Result<usize, Error> {
    let end = input
        .iter()
        .position(|code| *code == 0)
        .ok_or(Error::Terminator)?;
    let mut position = 0usize;
    let mut length = 0usize;
    while position < end {
        let code = input[position];
        position += 1;
        if code == 255 {
            continue;
        }
        let Some(record) = lookup(code) else { continue };
        if record.kind == 1 && record.standard_length <= 4 && record.program == 0 {
            if record.standard_length > 1 {
                emit(length, &[b"==,,'*  "[record.standard_length as usize]])?;
                length = length.checked_add(1).ok_or(Error::Capacity)?;
            }
        } else {
            for byte in record
                .mnemonic
                .to_le_bytes()
                .into_iter()
                .take_while(|byte| *byte != 0)
            {
                emit(length, &[byte])?;
                length = length.checked_add(1).ok_or(Error::Capacity)?;
            }
            if code == 21 {
                loop {
                    let byte = input[position];
                    // Legacy isalpha(char) is undefined for signed high bytes
                    // other than EOF. Reject before invoking its classifier.
                    if signed && byte >= 128 && byte != 255 {
                        return Err(Error::Character);
                    }
                    if !alpha(u32::from(byte)) {
                        break;
                    }
                    emit(length, &[byte])?;
                    length = length.checked_add(1).ok_or(Error::Capacity)?;
                    position += 1;
                    if position > end {
                        return Err(Error::Terminator);
                    }
                }
            }
        }
    }
    emit(length, b"\0")?;
    Ok(length)
}
/// A replayable allocation-free plan. Table lookup and locale classification
/// must be pure/stable during planning and emission; immutable input/table and
/// destination are disjoint. Legacy initial "* " writes remain observable in
/// otherwise-unused output bytes when the final decoded text is shorter.
pub struct Plan<'a, T, A> {
    input: &'a [u8],
    lookup: T,
    alpha: A,
    signed: bool,
    pub length: usize,
}
pub fn decode<T: Fn(u8) -> Option<Phoneme>, A: Fn(u32) -> bool>(
    input: &[u8],
    lookup: T,
    alpha: A,
    signed: bool,
    capacity: usize,
) -> Result<Plan<'_, T, A>, Error> {
    let length = visit(input, &lookup, &alpha, signed, |_, _| Ok(()))?;
    if capacity < 3
        || length
            .checked_add(1)
            .filter(|end| *end <= capacity)
            .is_none()
    {
        return Err(Error::Capacity);
    }
    Ok(Plan {
        input,
        lookup,
        alpha,
        signed,
        length,
    })
}
impl<T: Fn(u8) -> Option<Phoneme>, A: Fn(u32) -> bool> Plan<'_, T, A> {
    pub fn emit(
        &self,
        mut write: impl FnMut(usize, &[u8]) -> Result<(), Error>,
    ) -> Result<usize, Error> {
        write(0, b"* \0")?;
        visit(self.input, &self.lookup, &self.alpha, self.signed, write)
    }
    pub fn write(&self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < 3
            || self
                .length
                .checked_add(1)
                .filter(|end| *end <= output.len())
                .is_none()
        {
            return Err(Error::Capacity);
        }
        self.emit(|position, bytes| {
            let end = position
                .checked_add(bytes.len())
                .filter(|end| *end <= output.len())
                .ok_or(Error::Capacity)?;
            output[position..end].copy_from_slice(bytes);
            Ok(())
        })
    }
}
pub struct Wrapper {
    pub bytes: [u8; 74],
    pub length: usize,
}
/// Fixed clause phoneme wrapper. Decoded text includes NUL within55 initialized
/// bytes. Optional fallback name is the original default voice string; native
/// language-word conversion removes zero bytes in the packed original language.
pub fn wrapper(
    decoded: &[u8],
    secondary: Option<(&[u8], u32)>,
    capacity: usize,
) -> Result<Wrapper, Error> {
    let length = decoded
        .iter()
        .take(55)
        .position(|byte| *byte == 0)
        .ok_or(Error::Terminator)?;
    let mut result = Wrapper {
        bytes: [0; 74],
        length: 0,
    };
    let mut append = |text: &[u8]| -> Result<(), Error> {
        let end = result
            .length
            .checked_add(text.len())
            .filter(|end| *end < 74 && *end < capacity)
            .ok_or(Error::Capacity)?;
        result.bytes[result.length..end].copy_from_slice(text);
        result.length = end;
        Ok(())
    };
    append(b"[\x02")?;
    if let Some((voice, language)) = secondary {
        if voice.contains(&0) {
            return Err(Error::Terminator);
        }
        append(b"_^_")?;
        append(voice)?;
        append(b" ")?;
        append(&decoded[..length])?;
        append(b" _^_")?;
        let (text, length) = clause_input::language_word(language);
        append(&text[..length])?;
    } else {
        append(&decoded[..length])?;
    }
    append(b"]]")?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(code: u8) -> Option<Phoneme> {
        Some(Phoneme {
            code,
            mnemonic: crate::phoneme::mnemonic(if code == 21 { b"_^_" } else { b"a" }),
            kind: if code < 9 { 1 } else { 2 },
            standard_length: code.min(5),
            ..Phoneme::default()
        })
    }
    #[test]
    fn stress_switch_and_initial_output_tail_are_preserved() {
        let mut output = [0xa5; 40];
        let plan = decode(
            &[2, 3, 4, 21, b'e', b'n', 0],
            record,
            |code| (code as u8).is_ascii_alphabetic(),
            true,
            40,
        )
        .unwrap();
        let length = plan.write(&mut output).unwrap();
        assert_eq!(&output[..=length], b",,'_^_en\0");
        output.fill(0xa5);
        decode(&[255, 0], record, |_| false, true, 40)
            .unwrap()
            .write(&mut output)
            .unwrap();
        assert_eq!(&output[..4], b"\0 \0\xa5");
        assert!(matches!(
            decode(&[21, 128, 0], record, |_| false, true, 40),
            Err(Error::Character)
        ));
        assert!(matches!(
            decode(&[0], record, |_| false, true, 2),
            Err(Error::Capacity)
        ));
    }
    #[test]
    fn wrapper_bounds_and_secondary_language_bytes_are_exact() {
        let result = wrapper(b"abc\0", Some((b"en", 0x65006e00)), 74).unwrap();
        assert_eq!(&result.bytes[..=result.length], b"[\x02_^_en abc _^_en]]\0");
        assert!(matches!(wrapper(b"abc\0", None, 7), Err(Error::Capacity)));
        assert!(matches!(
            wrapper(&[97; 55], None, 74),
            Err(Error::Terminator)
        ));
    }
}
