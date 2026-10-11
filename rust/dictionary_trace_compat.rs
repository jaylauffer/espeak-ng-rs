//! Sized native trace admission; formatting performs no callbacks.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::dictionary_trace::{self, RULE_BYTES, TEXT_BYTES};
use std::ffi::{c_char, c_int};
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_flags(
    words: *const u32,
    output: *mut u8,
    capacity: usize,
) -> c_int {
    if words.is_null() || output.is_null() || capacity == 0 || capacity > RULE_BYTES {
        return -1;
    }
    // SAFETY: two initialized words, disjoint from declared exclusive output.
    let flags = unsafe { [words.read(), words.add(1).read()] };
    let mut scratch = [0; RULE_BYTES];
    let Some(length) = dictionary_trace::flags(flags, &mut scratch[..capacity]) else {
        return -1;
    };
    // SAFETY: publish initialized result only after successful bounded rendering.
    unsafe { std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1) };
    length as c_int
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_decode_rule(
    group: *const c_char,
    group_length: usize,
    encoded: *const u8,
    encoded_length: usize,
    control: u32,
    signed: c_int,
    columns: usize,
    output: *mut u8,
    capacity: usize,
) -> c_int {
    if group.is_null()
        || encoded.is_null()
        || output.is_null()
        || group_length >= TEXT_BYTES
        || encoded_length == 0
        || encoded_length > crate::dict_list::MAX_BYTES
        || capacity == 0
        || capacity > RULE_BYTES
    {
        return -1;
    }
    // SAFETY: caller supplies initialized group and immutable encoded extent;
    // output is exclusive/disjoint. Pure rendering makes no foreign effects.
    let (group, encoded) = unsafe {
        (
            std::slice::from_raw_parts(group.cast(), group_length),
            std::slice::from_raw_parts(encoded, encoded_length.min(RULE_BYTES)),
        )
    };
    let mut scratch = [0; RULE_BYTES];
    let Some(length) = dictionary_trace::rule(
        group,
        encoded,
        control,
        signed != 0,
        columns,
        &mut scratch[..capacity],
    ) else {
        return -1;
    };
    // SAFETY: admitted destination and initialized bounded result including NUL.
    unsafe { std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1) };
    length as c_int
}
