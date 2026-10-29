//! Serialized numeric source/state ABI for word admission policy.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    translate_word::{Error, Source, PHONEMES},
    word_admission::{self, Field, Host},
};
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) byte: Option<unsafe extern "C" fn(*mut c_void, Source) -> i32>,
    pub(super) write: Option<unsafe extern "C" fn(*mut c_void, Source, u8) -> i32>,
    pub(super) value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) alpha: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) vowel: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) probe: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut i32) -> i32>,
    pub(super) skip: Option<unsafe extern "C" fn(*mut c_void, i32)>,
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn byte(&self, s: Source) -> Option<u8> {
        // SAFETY: retained numeric source projection; no foreign span loan.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, s) }).ok()
    }
    fn write(&mut self, s: Source, b: u8) -> Result<(), Error> {
        // SAFETY: checked serialized owner-byte store; no retained Rust loan.
        if unsafe { self.0.write.unwrap()(self.0.context, s, b) } == 0 {
            Ok(())
        } else {
            Err(Error::Source)
        }
    }
    fn value(&self, f: Field) -> i32 {
        // SAFETY: fresh scalar projection from retained live language state.
        unsafe { self.0.value.unwrap()(self.0.context, f as u32) }
    }
    fn locale_alpha(&self, c: u32) -> bool {
        // SAFETY: raw CRT classification of one scalar character.
        unsafe { self.0.alpha.unwrap()(self.0.context, c) != 0 }
    }
    fn vowel(&mut self, c: u32) -> bool {
        // SAFETY: serialized vowel primitive, retaining no foreign loan.
        unsafe { self.0.vowel.unwrap()(self.0.context, c) != 0 }
    }
    fn probe(&mut self, scratch: &mut [u8; PHONEMES]) -> Result<i32, Error> {
        let mut flags = 0;
        // SAFETY: disjoint owned initialized pronunciation/result; the owner
        // resolves the admitted source and retains all nested shared effects.
        if unsafe { self.0.probe.unwrap()(self.0.context, scratch.as_mut_ptr(), &mut flags) } == 0 {
            Ok(flags)
        } else {
            Err(Error::State)
        }
    }
    fn skip_words(&mut self, n: i32) {
        // SAFETY: serialized scalar skip-word publication in the live owner.
        unsafe { self.0.skip.unwrap()(self.0.context, n) }
    }
}
unsafe fn callbacks<'a>(table: *const Callbacks) -> Option<&'a Callbacks> {
    if table.is_null() {
        return None;
    }
    // SAFETY: caller retains the immutable aligned table through this call.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.byte.is_none()
        || t.write.is_none()
        || t.value.is_none()
        || t.alpha.is_none()
        || t.vowel.is_none()
        || t.probe.is_none()
        || t.skip.is_none()
    {
        None
    } else {
        Some(t)
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_word_dotted(
    table: *const Callbacks,
    result: *mut i32,
) -> i32 {
    if result.is_null() {
        return -1;
    }
    // SAFETY: retained table/serialized live owners, result disjoint from them.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    let Ok(count) = word_admission::dotted(&mut Engine(t)) else {
        return -1;
    };
    // SAFETY: caller admits an aligned result slot; publish only after success.
    unsafe {
        result.write(count);
    }
    0
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_word_unpronounceable(
    table: *const Callbacks,
    position: i32,
    result: *mut i32,
) -> i32 {
    if result.is_null() {
        return -1;
    }
    // SAFETY: retained table/serialized live owners, result disjoint from them.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    let Ok(spell) = word_admission::unpronounceable(&mut Engine(t), position) else {
        return -1;
    };
    // SAFETY: caller admits an aligned result slot; publish only after success.
    unsafe {
        result.write(i32::from(spell));
    }
    0
}
