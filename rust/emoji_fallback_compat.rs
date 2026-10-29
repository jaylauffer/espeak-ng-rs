//! Serialized numeric source and live replacement ABI for emoji fallback.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    emoji_fallback::{self, Host, INPUT},
    translate_word::{Error, PHONEMES, WORD},
};
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) byte: Option<unsafe extern "C" fn(*mut c_void, usize) -> i32>,
    pub(super) list:
        Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8, *mut u32, *mut u8) -> i32>,
    pub(super) write: Option<unsafe extern "C" fn(*mut c_void, usize, *const u8, usize) -> i32>,
    pub(super) tracing: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) snapshot: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    pub(super) trace: Option<unsafe extern "C" fn(*mut c_void, *const u8, *const u8)>,
    pub(super) finish: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
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
    fn byte(&self, offset: usize) -> Option<u8> {
        // SAFETY: bounded live owner-byte projection at a numeric offset.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, offset) }).ok()
    }
    fn list(
        &mut self,
        input: &mut [u8; INPUT],
        phonemes: &mut [u8; PHONEMES],
        flags: &mut [u32; 2],
        text: &mut [u8; INPUT],
    ) -> Result<(), Error> {
        // SAFETY: disjoint owned initialized buffers (162/200/2 u32/162).
        // The primitive copies selected text before returning and keeps no loan.
        status(unsafe {
            self.0.list.unwrap()(
                self.0.context,
                input.as_mut_ptr(),
                phonemes.as_mut_ptr(),
                flags.as_mut_ptr(),
                text.as_mut_ptr(),
            )
        })
    }
    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        // SAFETY: owned byte block scoped to a checked in-place owner write.
        status(unsafe {
            self.0.write.unwrap()(self.0.context, offset, bytes.as_ptr(), bytes.len())
        })
    }
    fn tracing(&self) -> bool {
        // SAFETY: fresh scalar tracing projection after nested lookup effects.
        unsafe { self.0.tracing.unwrap()(self.0.context) != 0 }
    }
    fn snapshot(&self, replacement: &mut [u8; WORD]) -> Result<(), Error> {
        // SAFETY: copies initialized current owner into disjoint owned scratch.
        status(unsafe { self.0.snapshot.unwrap()(self.0.context, replacement.as_mut_ptr()) })
    }
    fn trace(&mut self, source: &[u8; WORD], replacement: &[u8; WORD]) {
        // SAFETY: owned terminated source/current replacement; no pointer kept.
        unsafe { self.0.trace.unwrap()(self.0.context, source.as_ptr(), replacement.as_ptr()) }
    }
    fn finish(&mut self) -> Result<(), Error> {
        // SAFETY: serialized source publication refers to retained live owner.
        status(unsafe { self.0.finish.unwrap()(self.0.context) })
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_lookup_emoji(
    table: *const Callbacks,
    flags: *mut u32,
) -> i32 {
    if table.is_null() || flags.is_null() {
        return -1;
    }
    // SAFETY: caller retains aligned immutable table and disjoint flag slot.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.byte.is_none()
        || t.list.is_none()
        || t.write.is_none()
        || t.tracing.is_none()
        || t.snapshot.is_none()
        || t.trace.is_none()
        || t.finish.is_none()
    {
        return -1;
    }
    match emoji_fallback::lookup(&mut Engine(t)) {
        Ok(false) => 0,
        Ok(true) => {
            // SAFETY: do not retain a flag loan across nested primitives. OR
            // into the fresh live slot only after successful source publication.
            unsafe {
                flags.write(flags.read() | 0x2000_0000);
            }
            1
        }
        Err(_) => -1,
    }
}
