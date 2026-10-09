//! Admit serialized letter lookup primitives and bounded output publication.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::letter_lookup::{self, Host};
use crate::number_lookup::PHONEME_BYTES;
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    lookup: Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, u32, *mut u8) -> i32>,
    named: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    space: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    rules: Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, usize, u32, *mut u8)>,
    select: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    stress: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u32, i32)>,
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn lookup(
        &mut self,
        source: &mut [u8; 10],
        start: usize,
        secondary: bool,
        out: &mut [u8; PHONEME_BYTES],
    ) -> i32 {
        // SAFETY: initialized owned source and 200-byte scratch disjoint; callback
        // uses only derived pointers through return and retains no source loan.
        unsafe {
            self.0.lookup.unwrap()(
                self.0.context,
                source.as_mut_ptr(),
                start,
                u32::from(secondary),
                out.as_mut_ptr(),
            )
        }
    }
    fn named(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
        // SAFETY: terminated immutable key, disjoint initialized writable scratch.
        unsafe { self.0.named.unwrap()(self.0.context, key.as_ptr(), out.as_mut_ptr()) }
    }
    fn value(&self, field: u32) -> i32 {
        // SAFETY: live serialized scalar projection, known field 0..=1.
        unsafe { self.0.value.unwrap()(self.0.context, field) }
    }
    fn space(&self, code: u32) -> bool {
        // SAFETY: platform wide-character classification primitive.
        unsafe { self.0.space.unwrap()(self.0.context, code) != 0 }
    }
    fn rules(
        &mut self,
        source: &mut [u8; 10],
        start: usize,
        capacity: usize,
        flags: u32,
        out: &mut [u8; PHONEME_BYTES],
    ) {
        // SAFETY: initialized owned source and scratch disjoint. Callback scopes
        // and restores source/rule context, retaining neither source nor output.
        unsafe {
            self.0.rules.unwrap()(
                self.0.context,
                source.as_mut_ptr(),
                start,
                capacity,
                flags,
                out.as_mut_ptr(),
            )
        }
    }
    fn select(&mut self, restore: bool) {
        // SAFETY: serialized secondary setup or fresh voice-table restoration.
        unsafe { self.0.select.unwrap()(self.0.context, u32::from(restore)) }
    }
    fn stress(&mut self, out: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2], control: i32) {
        // SAFETY: initialized terminated owned scratch and flags disjoint from
        // serialized engine state; stress primitive retains no output pointer.
        unsafe {
            self.0.stress.unwrap()(
                self.0.context,
                out.as_mut_ptr(),
                flags.as_mut_ptr(),
                control,
            )
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_letter(
    table: *const Callbacks,
    letter: u32,
    next: i32,
    control: i32,
    accent: i32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if table.is_null()
        || output.is_null()
        || capacity == 0
        || capacity > PHONEME_BYTES
        || !(0..=1).contains(&accent)
    {
        return -1;
    }
    // SAFETY: live immutable callback table and serialized primitive contracts
    // admitted through return, disjoint from all writable output/source/state.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.lookup.is_none()
        || table.named.is_none()
        || table.value.is_none()
        || table.space.is_none()
        || table.rules.is_none()
        || table.select.is_none()
        || table.stress.is_none()
    {
        return -1;
    }
    let mut scratch = [0; PHONEME_BYTES];
    let result = if accent == 1 {
        letter_lookup::accented(&mut Engine(table), letter, &mut scratch[..capacity])
    } else {
        letter_lookup::letter(
            &mut Engine(table),
            letter,
            next,
            control,
            &mut scratch[..capacity],
        )
        .map(Some)
    };
    match result {
        Ok(Some(written)) => {
            // SAFETY: publish only validated initialized terminated output prefix.
            unsafe { std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, written + 1) };
            1
        }
        Ok(None) => 0,
        Err(_) => -1,
    }
}
