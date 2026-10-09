//! Bounded admission for the serialized dictionary-list frontend.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::lookup_list::{self, Error, Host, WORD_BYTES};
use crate::number_lookup::PHONEME_BYTES;
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    context: *mut c_void,
    byte: Option<unsafe extern "C" fn(*mut c_void, usize) -> i32>,
    lookup: Option<
        unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut u32, *mut u8, *mut usize) -> i32,
    >,
    repeat: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    set_repeat: Option<unsafe extern "C" fn(*mut c_void, *const u8, i32)>,
    text_mode: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    skip: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    accent: Option<unsafe extern "C" fn(*mut c_void, u32, usize, *mut u8)>,
    replacement: Option<unsafe extern "C" fn(*mut c_void, *const u8)>,
    trace: Option<unsafe extern "C" fn(*mut c_void, usize)>,
}
pub(super) struct Engine<'a>(&'a Callbacks);
impl Callbacks {
    pub(super) fn engine(&self) -> Option<Engine<'_>> {
        (!self.context.is_null()
            && self.byte.is_some()
            && self.lookup.is_some()
            && self.repeat.is_some()
            && self.set_repeat.is_some()
            && self.text_mode.is_some()
            && self.skip.is_some()
            && self.accent.is_some()
            && self.replacement.is_some()
            && self.trace.is_some())
        .then_some(Engine(self))
    }
    pub(super) fn context(&self) -> *mut c_void {
        self.context
    }
}
impl Host for Engine<'_> {
    fn byte(&self, position: usize) -> Option<u8> {
        // SAFETY: serialized source projection checks its admitted extent.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, position) }).ok()
    }
    fn lookup(
        &mut self,
        key: &[u8; WORD_BYTES],
        next: usize,
        flags: &mut [u32; 2],
        phonemes: &mut [u8; PHONEME_BYTES],
    ) -> Result<Option<usize>, Error> {
        let mut matched = 0;
        // SAFETY: terminated owned key, two initialized flags and 200-byte
        // initialized scratch are disjoint; callback retains no loans.
        match unsafe {
            self.0.lookup.unwrap()(
                self.0.context,
                key.as_ptr(),
                next,
                flags.as_mut_ptr(),
                phonemes.as_mut_ptr(),
                &mut matched,
            )
        } {
            0 => Ok(None),
            1 => Ok(Some(matched)),
            _ => Err(Error::Source),
        }
    }
    fn repeat(&self, output: &mut [u8; 20]) -> i32 {
        // SAFETY: copy live repeat state into exclusive initialized scratch.
        unsafe { self.0.repeat.unwrap()(self.0.context, output.as_mut_ptr()) }
    }
    fn set_repeat(&mut self, output: &[u8; 20], count: i32) {
        // SAFETY: state store copies 20 owned bytes, retaining no pointer.
        unsafe { self.0.set_repeat.unwrap()(self.0.context, output.as_ptr(), count) }
    }
    fn text_mode(&self) -> bool {
        // SAFETY: fresh serialized scalar projection.
        unsafe { self.0.text_mode.unwrap()(self.0.context) != 0 }
    }
    fn skip_words(&mut self, count: i32) {
        // SAFETY: serialized scalar store.
        unsafe { self.0.skip.unwrap()(self.0.context, count) }
    }
    fn accent(&mut self, code: u32, capacity: usize, phonemes: &mut [u8; PHONEME_BYTES]) {
        // SAFETY: initialized 200-byte scratch; actual publication extent
        // separately supplied. Native accent child retains no output loan.
        unsafe { self.0.accent.unwrap()(self.0.context, code, capacity, phonemes.as_mut_ptr()) }
    }
    fn replacement(&mut self, text: &[u8; WORD_BYTES]) {
        // SAFETY: full initialized padded/terminated replacement copied by owner.
        unsafe { self.0.replacement.unwrap()(self.0.context, text.as_ptr()) }
    }
    fn trace_replacement(&mut self, matched: usize) {
        // SAFETY: trace primitive checks numeric original source extent.
        unsafe { self.0.trace.unwrap()(self.0.context, matched) }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_list(
    table: *const Callbacks,
    end_flags: u32,
    flags: *mut u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if table.is_null()
        || flags.is_null()
        || output.is_null()
        || capacity == 0
        || capacity > PHONEME_BYTES
    {
        return -1;
    }
    // SAFETY: immutable callback table and live serialized owner admitted through
    // return, disjoint from flags/output and all callback scratch.
    let table = unsafe { &*table };
    let Some(mut engine) = table.engine() else {
        return -1;
    };
    // SAFETY: two readable initialized flags, copied before callbacks. Never
    // inspect the caller's possibly uninitialized phoneme output tail.
    let mut selected = unsafe { [flags.read(), flags.add(1).read()] };
    let mut scratch = [0; PHONEME_BYTES];
    let Ok(found) = lookup_list::dictionary_list(
        &mut engine,
        end_flags,
        &mut selected,
        &mut scratch[..capacity],
    ) else {
        return -1;
    };
    let size = scratch.iter().position(|byte| *byte == 0).unwrap();
    // SAFETY: publish only the validated initialized prefix and two output flags.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, size + 1);
        std::ptr::copy_nonoverlapping(selected.as_ptr(), flags, 2);
    }
    i32::from(found)
}
