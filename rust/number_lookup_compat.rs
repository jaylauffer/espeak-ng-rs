//! Initialized scratch spans for serialized number dictionary primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_lookup::{self as number, Host, PHONEME_BYTES};
use std::ffi::c_void;

#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    missing: Option<unsafe extern "C" fn(*mut c_void, i32)>,
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn lookup(&mut self, key: &[u8], phonemes: &mut [u8; PHONEME_BYTES]) -> i32 {
        // SAFETY: key is terminated, scratch is fully initialized/writable;
        // the admitted primitive writes at most PHONEME_BYTES, including NUL.
        unsafe { self.0.lookup.unwrap()(self.0.context, key.as_ptr(), phonemes.as_mut_ptr()) }
    }
    fn numbers(&self) -> i32 {
        self.value(0)
    }
    fn variants(&self) -> i32 {
        self.value(1)
    }
    fn control(&self) -> i32 {
        self.value(2)
    }
    fn missing(&mut self, value: i32) {
        // SAFETY: admitted serialized state store; no foreign borrow held.
        unsafe { self.0.missing.unwrap()(self.0.context, value) }
    }
}
impl Engine<'_> {
    fn value(&self, field: u32) -> i32 {
        // SAFETY: admitted fresh state projection, field in 0..=2.
        unsafe { self.0.value.unwrap()(self.0.context, field) }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_thousands(
    table: *const Callbacks,
    value: i32,
    plex: i32,
    exact: i32,
    output: *mut u8,
    capacity: usize,
    found: *mut i32,
) -> i32 {
    if table.is_null()
        || output.is_null()
        || found.is_null()
        || capacity == 0
        || capacity > PHONEME_BYTES
    {
        return -1;
    }
    // SAFETY: caller admits an immutable live callback table through return.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.lookup.is_none()
        || table.value.is_none()
        || table.missing.is_none()
    {
        return -1;
    }
    let mut scratch = [0; PHONEME_BYTES];
    let Ok(result) = number::thousands(
        &mut Engine(table),
        value,
        plex,
        exact,
        &mut scratch[..capacity],
    ) else {
        return -1;
    };
    let written = scratch.iter().position(|byte| *byte == 0).unwrap();
    // SAFETY: disjoint writable scalar and capacity-sized output admitted by
    // caller. Publish only initialized prefix; never borrow foreign output tail.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, written + 1);
        found.write(result);
    }
    0
}
