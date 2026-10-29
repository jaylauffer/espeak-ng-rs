//! Numeric-source and fresh owner-effect ABI for spelling/plural control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    translate_word::{Error, Source},
    word_helpers::{self, Host},
};
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) byte: Option<unsafe extern "C" fn(*mut c_void, Source) -> i32>,
    pub(super) letter: Option<unsafe extern "C" fn(*mut c_void, Source, u32, *mut usize) -> i32>,
    pub(super) leading: Option<unsafe extern "C" fn(*mut c_void) -> u8>,
    pub(super) copy_switch: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) stress: Option<unsafe extern "C" fn(*mut c_void, i32, i32) -> i32>,
    pub(super) plural: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
fn status(value: i32) -> Result<(), Error> {
    if value == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
impl Host for Engine<'_> {
    fn byte(&self, source: Source) -> Option<u8> {
        // SAFETY: checked serialized owner-byte projection, no foreign loan.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, source) }).ok()
    }
    fn letter(&mut self, source: Source, control: u32) -> Result<usize, Error> {
        let mut count = 0;
        // SAFETY: numeric source and disjoint consumed-count slot. The primitive
        // operates on current retained pronunciation, keeping no Rust loan.
        status(unsafe { self.0.letter.unwrap()(self.0.context, source, control, &mut count) })?;
        Ok(count)
    }
    fn leading(&self) -> u8 {
        // SAFETY: fresh scalar projection after nested pronunciation effects.
        unsafe { self.0.leading.unwrap()(self.0.context) }
    }
    fn copy_switch(&mut self) -> Result<(), Error> {
        // SAFETY: serialized bounded owner-to-owner output copy.
        status(unsafe { self.0.copy_switch.unwrap()(self.0.context) })
    }
    fn stress(&mut self, mode: i32, count: i32) -> Result<(), Error> {
        // SAFETY: serialized stress primitive operates on fresh live output.
        status(unsafe { self.0.stress.unwrap()(self.0.context, mode, count) })
    }
    fn plural(&mut self, source: &mut [u8; 5]) -> Result<(), Error> {
        // SAFETY: owned initialized five-byte source scoped to this call; the
        // primitive keeps no pointer, using fresh disjoint pronunciation.
        status(unsafe { self.0.plural.unwrap()(self.0.context, source.as_mut_ptr()) })
    }
}
unsafe fn callbacks<'a>(table: *const Callbacks) -> Option<&'a Callbacks> {
    if table.is_null() {
        return None;
    }
    // SAFETY: immutable aligned callback table retained by the caller.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.byte.is_none()
        || t.letter.is_none()
        || t.leading.is_none()
        || t.copy_switch.is_none()
        || t.stress.is_none()
        || t.plural.is_none()
    {
        None
    } else {
        Some(t)
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_word_spell(
    table: *const Callbacks,
    mode: i32,
    cursor: *mut Source,
    switched: *mut i32,
) -> i32 {
    if cursor.is_null() || switched.is_null() {
        return -1;
    }
    // SAFETY: retained table and serialized live owners, result slots disjoint.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    let Ok(result) = word_helpers::spell(&mut Engine(t), mode) else {
        return -1;
    };
    // SAFETY: caller retains aligned disjoint result slots. Publish on success.
    unsafe {
        if let Some(source) = result {
            cursor.write(source);
        }
        switched.write(i32::from(result.is_none()));
    }
    0
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_word_plural(
    table: *const Callbacks,
    flags: u32,
    last: u8,
) -> i32 {
    // SAFETY: retained table and serialized live source/state/output owners.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    if word_helpers::plural(&mut Engine(t), flags, last).is_ok() {
        0
    } else {
        -1
    }
}
