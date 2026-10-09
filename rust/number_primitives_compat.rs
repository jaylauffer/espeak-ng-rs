//! Bounded serialized C spans for native number/spelling primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_primitives as number;
#[no_mangle]
extern "C" fn espeak_rs_superscript(letter: i32) -> i32 {
    number::superscript(letter)
}
#[no_mangle]
extern "C" fn espeak_rs_number_variant(value: i32, options: i32) -> *const u8 {
    number::thousands_variant(value, options).as_ptr()
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_spelling(
    phonemes: *mut u8,
    length: usize,
    capacity: usize,
    initial: i32,
    control: i32,
    chars: i32,
) -> i32 {
    if phonemes.is_null() || length == 0 || length > capacity || capacity > 200 {
        return -1;
    }
    let mut scratch = [0; 200];
    // SAFETY: caller admits only length initialized input bytes, plus capacity
    // writable storage. Never borrow its uninitialized foreign output tail.
    scratch[..length].copy_from_slice(unsafe { std::slice::from_raw_parts(phonemes, length) });
    if scratch[length - 1] != 0 {
        return -1;
    }
    let Ok(written) = number::spelling(&mut scratch[..capacity], initial != 0, control, chars)
    else {
        return -1;
    };
    // SAFETY: validated output prefix fits admitted writable capacity; stack
    // scratch is initialized and disjoint. Reject paths leave all bytes intact.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), phonemes, written + 1);
    }
    written as i32
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_number_hungarian(
    word: *const u8,
    thousandplex: i32,
    value: i32,
) -> i32 {
    if word.is_null() {
        return 0;
    }
    let mut bytes = [0; 3];
    for (index, byte) in bytes.iter_mut().enumerate() {
        // SAFETY: caller admits a C string through its terminator. Stop there,
        // never project a padded/uninitialized tail beyond the initialized text.
        *byte = unsafe { word.add(index).read() };
        if *byte == 0 {
            break;
        }
    }
    i32::from(number::hungarian_e(&bytes, thousandplex, value))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_number_group(word: *const u8, digits: i32) -> i32 {
    if word.is_null() || digits < 0 || digits as usize > number::WORD_BYTES {
        return 0;
    }
    let mut frame = [0; number::WORD_BYTES + 2];
    for index in 0..digits as usize {
        // SAFETY: admitted C string; reject immediately at a non-digit/NUL,
        // before reading any unused tail after an early terminator.
        let byte = unsafe { word.add(index).read() };
        if !byte.is_ascii_digit() {
            return 0;
        }
        frame[index + 1] = byte;
    }
    // SAFETY: one preceding byte and the initialized following digit boundary
    // are admitted by the engine word owner, as in the original helper.
    unsafe {
        frame[0] = word.sub(1).read();
        frame[digits as usize + 1] = word.add(digits as usize).read();
    }
    i32::from(number::thousands_group(
        &frame[..digits as usize + 2],
        digits as usize,
    ))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_number_roman(
    word: *const u8,
    predecessor: u8,
    flags: u32,
    options: i32,
    min: i32,
    max: i32,
    value: *mut i32,
    after: *mut usize,
) -> i32 {
    if word.is_null() || value.is_null() || after.is_null() {
        return 0;
    }
    let mut input = [0; number::WORD_BYTES + 1];
    for index in 0..number::WORD_BYTES {
        // SAFETY: caller admits an engine word through its space/NUL separator
        // and one following initialized byte within the bounded word extent.
        let byte = unsafe { word.add(index).read() };
        input[index] = byte;
        if matches!(byte, 0 | b' ') {
            // SAFETY: one initialized following boundary admitted as above.
            input[index + 1] = unsafe { word.add(index + 1).read() };
            let Some(parsed) =
                number::roman(&input[..index + 2], predecessor, flags, options, min, max)
            else {
                return 0;
            };
            // SAFETY: separate writable scalar outputs, disjoint from input;
            // failed recognition leaves these and the word owner untouched.
            unsafe {
                value.write(parsed.value);
                after.write(parsed.after);
            }
            return 1;
        }
    }
    0
}
