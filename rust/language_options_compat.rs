//! `LoadLanguageOptions` against the engine's tunes, with C's diagnostics.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{crt, intonation::TUNE_SIZE, language_options};
use std::ffi::{c_char, c_int, CStr};

/// C's stderr, as the original's `fprintf(stderr, ...)`.
fn report(parts: &[&[u8]]) {
    let stream = crt::standard_error();
    for part in parts {
        // SAFETY: the C runtime's live standard error stream.
        unsafe { crt::write(stream, part) };
    }
}
/// The loaded tunes' names (`tunes[i].name`, NUL-padded to 12 bytes).
struct Engine;
impl language_options::Environment for Engine {
    fn tune(&self, name: &[u8]) -> Option<i32> {
        // SAFETY: serialized engine: the loaded intonation table holds n_tunes
        // records of TUNE_SIZE bytes, unchanged during the call.
        let (count, table) = unsafe {
            (
                super::synthdata_compat::n_tunes,
                super::synthdata_compat::tunes,
            )
        };
        if table.is_null() {
            return None;
        }
        (0..count.max(0) as usize)
            .find(|&index| {
                // SAFETY: index < n_tunes; the name is the record's first 12 bytes.
                let record =
                    unsafe { std::slice::from_raw_parts(table.add(index * TUNE_SIZE), 12) };
                let length = record.iter().position(|&b| b == 0).unwrap_or(12);
                &record[..length] == name
            })
            .map(|index| index as i32)
    }
    fn bad_ordinal(&mut self, key: u32, number: i32) {
        report(&[
            language_options::name(key),
            b": Bad option number ",
            number.to_string().as_bytes(),
            b"\n",
        ]);
    }
    fn unknown_tune(&mut self, name: &[u8]) {
        report(&[b"Unknown tune '", name, b"'\n"]);
    }
}
/// The checks and messages of `LoadLanguageOptions` around one option: with
/// no translator (`options` null), "Cannot set ..." and 1; for a value that
/// does not parse, "Invalid language option: ..." and 2, `options` unchanged;
/// otherwise 0, `options` updated.
///
/// # Safety
/// `options` is null or an exclusive initialized snapshot; `value` is a
/// terminated string; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_language_option_engine(
    options: *mut language_options::Options,
    key: u32,
    value: *const c_char,
) -> c_int {
    let name = language_options::name(key);
    if options.is_null() {
        report(&[
            b"Cannot set ",
            name,
            b": language not set, or is invalid.\n",
        ]);
        return 1;
    }
    if value.is_null() {
        return 2;
    }
    // SAFETY: caller contract.
    let (options, value) = unsafe { (&mut *options, CStr::from_ptr(value).to_bytes()) };
    if options.apply(key, value, &mut Engine).is_err() {
        report(&[b"Invalid language option: ", name, b"\n"]);
        return 2;
    }
    0
}

unsafe extern "C" {
    static mut tone_points: [c_int; 12];
    fn ConfigureSoundIcons(points: *mut c_int);
}
/// `LoadConfig`: the configuration file's tone points and sound icons.
///
/// # Safety
/// Serialized engine initialization.
#[no_mangle]
unsafe extern "C" fn LoadConfig() {
    // SAFETY: caller contract; the engine's tone point table.
    unsafe { ConfigureSoundIcons(std::ptr::addr_of_mut!(tone_points).cast()) }
}
