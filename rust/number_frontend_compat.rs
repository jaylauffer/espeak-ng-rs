//! Admit serialized initialized number source, dictionary and state primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_frontend::{self, Host};
use crate::number_lookup::{Error, PHONEME_BYTES};
use std::ffi::c_void;
#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    byte: Option<unsafe extern "C" fn(*mut c_void, isize) -> u8>,
    write: Option<unsafe extern "C" fn(*mut c_void, usize, u8)>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    word: Option<unsafe extern "C" fn(*mut c_void, usize) -> u32>,
    lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    list: Option<unsafe extern "C" fn(*mut c_void, isize, *mut u8, *mut u32) -> i32>,
    text: Option<unsafe extern "C" fn(*mut c_void, u32) -> *const u8>,
    store_text: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8, usize) -> i32>,
    classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    translate: Option<unsafe extern "C" fn(*mut c_void, usize) -> u32>,
    missing: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    skip: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    phoneme_type: Option<unsafe extern "C" fn(*mut c_void, u8) -> i32>,
}
pub(super) struct Engine<'a>(&'a Callbacks);
impl<'a> Engine<'a> {
    /// SAFETY: pointer admits a live immutable table, serialized context and
    /// primitive contracts described in rust_number_frontend.h through return.
    pub(super) unsafe fn admit(table: *const Callbacks) -> Option<Self> {
        if table.is_null() {
            return None;
        }
        // SAFETY: caller admits live immutable callback storage.
        let table = unsafe { &*table };
        if table.context.is_null()
            || table.byte.is_none()
            || table.write.is_none()
            || table.value.is_none()
            || table.word.is_none()
            || table.lookup.is_none()
            || table.list.is_none()
            || table.text.is_none()
            || table.store_text.is_none()
            || table.classify.is_none()
            || table.translate.is_none()
            || table.missing.is_none()
            || table.skip.is_none()
            || table.phoneme_type.is_none()
        {
            None
        } else {
            Some(Self(table))
        }
    }
    pub(super) fn context(&self) -> *mut c_void {
        self.0.context
    }
}
impl Host for Engine<'_> {
    fn byte(&self, offset: isize) -> u8 {
        // SAFETY: admitted primitive bounds reads to the initialized source extent.
        unsafe { self.0.byte.unwrap()(self.0.context, offset) }
    }
    fn write_byte(&mut self, offset: usize, value: u8) {
        // SAFETY: admitted primitive bounds source mutation, no loan held.
        unsafe { self.0.write.unwrap()(self.0.context, offset, value) }
    }
    fn value(&self, field: u32) -> i32 {
        // SAFETY: admitted live scalar projection, known field in 0..=9.
        unsafe { self.0.value.unwrap()(self.0.context, field) }
    }
    fn word_flags(&self, index: usize) -> u32 {
        // SAFETY: primitive projects only initialized admitted word rows.
        unsafe { self.0.word.unwrap()(self.0.context, index) }
    }
    fn lookup(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
        // SAFETY: terminated key and initialized disjoint 200-byte scratch;
        // trusted dictionary writes a terminated string within this extent.
        unsafe { self.0.lookup.unwrap()(self.0.context, key.as_ptr(), out.as_mut_ptr()) }
    }
    fn list(&mut self, offset: isize, out: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2]) -> i32 {
        // SAFETY: primitive bounds original source; writable initialized scratch
        // and two flags are disjoint. No foreign source loan spans translation.
        unsafe {
            self.0.list.unwrap()(self.0.context, offset, out.as_mut_ptr(), flags.as_mut_ptr())
        }
    }
    fn text(&self, kind: u32, out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: fresh initialized prefix projection, known kind 0..=2.
        let pointer = unsafe { self.0.text.unwrap()(self.0.context, kind) };
        if pointer.is_null() {
            return if kind == 2 {
                out[0] = 0;
                Ok(())
            } else {
                Err(Error::Phonemes)
            };
        }
        let limit = if kind == 2 { 32 } else { 12 };
        for (i, b) in out[..limit].iter_mut().enumerate() {
            // SAFETY: read only initialized prefix, stopping at its terminator.
            *b = unsafe { pointer.add(i).read() };
            if *b == 0 {
                return Ok(());
            }
        }
        if kind == 2 {
            // A >=32-byte indicator cannot equal the <=29-byte suffix. Keep
            // a 31-byte unmatched prefix without examining its unused tail.
            out[31] = 0;
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn store_text(&mut self, kind: u32, text: &[u8]) -> Result<(), Error> {
        // SAFETY: validated <=12-byte initialized terminated prefix; primitive
        // stores it into the admitted ordinal slot without retaining the loan.
        let result =
            unsafe { self.0.store_text.unwrap()(self.0.context, kind, text.as_ptr(), text.len()) };
        if result == 0 {
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn classify(&self, code: u32, kind: u32) -> bool {
        // SAFETY: admitted engine/Unicode/byte classification, kind 0..=3.
        unsafe { self.0.classify.unwrap()(self.0.context, code, kind) != 0 }
    }
    fn translate(&mut self, offset: usize) -> u32 {
        // SAFETY: bounds-checked original source owner; serialized callback.
        unsafe { self.0.translate.unwrap()(self.0.context, offset) }
    }
    fn missing(&mut self, value: i32) {
        // SAFETY: admitted serialized persistent state store, no foreign loan.
        unsafe { self.0.missing.unwrap()(self.0.context, value) }
    }
    fn skip_words(&mut self, value: i32) {
        // SAFETY: admitted serialized persistent skip-word state store.
        unsafe { self.0.skip.unwrap()(self.0.context, value) }
    }
    fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
        // SAFETY: checked unsigned-code primitive returns -1 for absent slots.
        let kind = unsafe { self.0.phoneme_type.unwrap()(self.0.context, code) };
        if kind < 0 {
            Err(Error::Phonemes)
        } else {
            Ok(kind)
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_number(
    table: *const Callbacks,
    length: usize,
    remaining: i32,
    control: i32,
    output: *mut u8,
    capacity: usize,
    flags: *mut u32,
) -> i32 {
    if table.is_null()
        || output.is_null()
        || flags.is_null()
        || length == 0
        || length > 800
        || !(0..=300).contains(&remaining)
        || capacity == 0
        || capacity > PHONEME_BYTES
    {
        return -1;
    }
    // SAFETY: immutable callback table and primitive contracts admitted.
    let Some(mut engine) = (unsafe { Engine::admit(table) }) else {
        return -1;
    };
    let mut scratch = [0; PHONEME_BYTES];
    // SAFETY: two initialized caller flags admitted, disjoint from all loans.
    let mut copied_flags = unsafe { [flags.read(), flags.add(1).read()] };
    let Ok(result) = number_frontend::translate(
        &mut engine,
        remaining as usize,
        control,
        &mut scratch[..capacity],
        &mut copied_flags,
    ) else {
        return -1;
    };
    // SAFETY: publish only initialized validated output prefix and two flags.
    // No foreign unfilled tail is borrowed; rejection preserves both outputs.
    unsafe {
        if let Some(written) = result.written {
            std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, written + 1);
        }
        flags.write(copied_flags[0]);
        flags.add(1).write(copied_flags[1]);
    }
    i32::from(result.recognized)
}
