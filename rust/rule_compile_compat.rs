//! Serialized C compiler adapter; snapshot strings before primitive effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::rule_compile::{self, Diagnostic, Environment, Host};
use std::ffi::{c_char, c_int, c_void};

type Encode =
    unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, usize, *mut c_int) -> c_int;
type Digit = unsafe extern "C" fn(u32) -> c_int;
type Report = unsafe extern "C" fn(*mut c_void, c_int, c_int, *const c_char);
#[repr(C)]
pub struct Descriptor {
    group: *const c_char,
    letters: *const u8,
    spaces: *const u8,
    line: c_int,
    debug: c_int,
    signed_char: c_int,
    context: *mut c_void,
    encode: Option<Encode>,
    digit: Option<Digit>,
    report: Option<Report>,
}
struct Effects {
    context: *mut c_void,
    encode: Encode,
    digit: Digit,
    report: Report,
}
impl Host for Effects {
    fn encode(&mut self, input: &[u8], output: &mut [u8; 80]) -> Option<i32> {
        let mut bad = 0;
        // SAFETY: initialized NUL input and owned writable 80-byte scratch;
        // callback is synchronous and may not retain either pointer.
        let ok = unsafe {
            (self.encode)(
                self.context,
                input.as_ptr().cast(),
                output.as_mut_ptr().cast(),
                output.len(),
                &mut bad,
            )
        };
        (ok != 0).then_some(bad)
    }
    fn digit(&mut self, code: u32) -> bool {
        // SAFETY: synchronous primitive classifier from the descriptor.
        unsafe { (self.digit)(code) != 0 }
    }
    fn diagnostic(&mut self, kind: Diagnostic, value: i32, text: &[u8]) {
        // SAFETY: initialized NUL text; callback cannot retain its pointer.
        unsafe { (self.report)(self.context, kind as i32, value, text.as_ptr().cast()) };
    }
}
unsafe fn string<const N: usize>(source: *const c_char) -> Option<([u8; N], usize)> {
    if source.is_null() {
        return None;
    }
    let mut copy = [0; N];
    for (index, byte) in copy.iter_mut().enumerate() {
        // SAFETY: caller supplies a readable string prefix through its NUL,
        // or all N readable bytes for a rejected unterminated input.
        *byte = unsafe { source.add(index).cast::<u8>().read() };
        if *byte == 0 {
            return Some((copy, index));
        }
    }
    None
}

/// Caller serializes the compiler, keeps inputs/descriptor alive and immutable
/// through snapshot, and supplies a writable output extent disjoint from the
/// primitive effects' state. Effects cannot retain scratch pointers. The output
/// is untouched on rejection; effects already performed remain observable.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_rule(
    descriptor: *const Descriptor,
    input: *const c_char,
    output: *mut u8,
    capacity: usize,
) -> usize {
    if descriptor.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: caller supplies a fully initialized descriptor.
    let d = unsafe { descriptor.read() };
    let (Some(encode), Some(digit), Some(report)) = (d.encode, d.digit, d.report) else {
        return 0;
    };
    if d.letters.is_null() || d.spaces.is_null() {
        return 0;
    }
    let mut effects = Effects {
        context: d.context,
        encode,
        digit,
        report,
    };
    // SAFETY: caller's initialized bounded string prefixes; no loan escapes.
    let Some((input, input_length)) = (unsafe { string::<500>(input) }) else {
        effects.diagnostic(Diagnostic::Overflow, 0, b"\0");
        return 0;
    };
    // SAFETY: group has a readable NUL within 13 bytes.
    let Some((group, group_length)) = (unsafe { string::<13>(d.group) }) else {
        effects.diagnostic(Diagnostic::Overflow, 0, b"\0");
        return 0;
    };
    let mut letters = [0; 95];
    let mut spaces = [0; 256];
    // SAFETY: descriptor supplies initialized arrays of the declared extents;
    // copies occur before callbacks, without reading unrelated context fields.
    unsafe {
        std::ptr::copy_nonoverlapping(d.letters, letters.as_mut_ptr(), letters.len());
        std::ptr::copy_nonoverlapping(d.spaces, spaces.as_mut_ptr(), spaces.len());
    }
    let env = Environment {
        group: &group[..group_length],
        letters: &letters,
        spaces: &spaces,
        line: d.line,
        debug: d.debug != 0,
        signed_char: d.signed_char != 0,
    };
    let mut result = [0; rule_compile::OUTPUT_BYTES];
    let length = rule_compile::compile(
        &mut effects,
        &env,
        &input[..input_length],
        &mut result[..capacity.min(rule_compile::OUTPUT_BYTES)],
    );
    if length != 0 {
        // SAFETY: caller's writable output extent admits this completed record;
        // owned scratch has no outstanding loan or callback.
        unsafe { std::ptr::copy_nonoverlapping(result.as_ptr(), output, length) };
    }
    length
}
