//! Owned scratch and scalar effects for isolated-character translation.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    number_lookup::{Error, PHONEME_BYTES},
    translate_letter::{self, Field, Host},
};
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    named:
        Option<unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, *mut i32) -> i32>,
    letter: Option<unsafe extern "C" fn(*mut c_void, u32, u32, i32, u32, usize, *mut u8) -> i32>,
    secondary: Option<unsafe extern "C" fn(*mut c_void, *const u8) -> i32>,
    restore: Option<unsafe extern "C" fn(*mut c_void)>,
    hangul: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8) -> i32>,
    encode: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    publish: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
fn status(value: i32) -> Result<(), Error> {
    if value == 0 {
        Ok(())
    } else {
        Err(Error::Phonemes)
    }
}
impl Host for Engine<'_> {
    fn value(&self, field: Field) -> i32 {
        // SAFETY: retained serialized scalar owner; no foreign loan.
        unsafe { self.0.value.unwrap()(self.0.context, field as u32) }
    }
    fn classify(&self, code: u32, kind: u32) -> bool {
        // SAFETY: host CRT classification of scalar code.
        unsafe { self.0.classify.unwrap()(self.0.context, code, kind) != 0 }
    }
    fn named(
        &mut self,
        which: u32,
        key: &[u8],
        capacity: usize,
        output: &mut [u8; PHONEME_BYTES],
    ) -> Result<i32, Error> {
        let mut flags = 0;
        // SAFETY: owned terminated key, initialized disjoint scratch and result;
        // synchronous primitive retains no pointer through return.
        status(unsafe {
            self.0.named.unwrap()(
                self.0.context,
                which,
                key.as_ptr(),
                capacity,
                output.as_mut_ptr(),
                &mut flags,
            )
        })?;
        Ok(flags)
    }
    fn letter(
        &mut self,
        which: u32,
        code: u32,
        next: i32,
        control: u32,
        capacity: usize,
        output: &mut [u8; PHONEME_BYTES],
    ) -> Result<(), Error> {
        // SAFETY: bounded initialized owned pronunciation, scalar inputs.
        status(unsafe {
            self.0.letter.unwrap()(
                self.0.context,
                which,
                code,
                next,
                control,
                capacity,
                output.as_mut_ptr(),
            )
        })
    }
    fn secondary(&mut self, name: &[u8]) -> i32 {
        // SAFETY: owned terminated name; owner performs fresh setup synchronously.
        unsafe { self.0.secondary.unwrap()(self.0.context, name.as_ptr()) }
    }
    fn restore_table(&mut self) {
        // SAFETY: owner restores the fresh voice table; no retained loan.
        unsafe { self.0.restore.unwrap()(self.0.context) }
    }
    fn hangul(
        &mut self,
        source: &mut [u8; 12],
        output: &mut [u8; PHONEME_BYTES],
    ) -> Result<(), Error> {
        // SAFETY: owned initialized source and disjoint pronunciation; callback
        // scopes/restores source context before returning and retains nothing.
        status(unsafe {
            self.0.hangul.unwrap()(self.0.context, source.as_mut_ptr(), output.as_mut_ptr())
        })
    }
    fn encode(&mut self, text: &[u8], output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: fixed host literal and initialized disjoint 200-byte scratch.
        status(unsafe {
            self.0.encode.unwrap()(self.0.context, text.as_ptr(), output.as_mut_ptr())
        })
    }
    fn publish(&mut self, replace: bool, output: &[u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: owned bounded terminated prefix, fresh serialized output owner.
        status(unsafe {
            self.0.publish.unwrap()(self.0.context, u32::from(replace), output.as_ptr())
        })
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_letter(
    table: *const Callbacks,
    code: u32,
    next: i32,
    control: u32,
    current: u32,
    consumed: i32,
    result: *mut i32,
) -> i32 {
    if table.is_null() || result.is_null() || !(1..=4).contains(&consumed) {
        return -1;
    }
    // SAFETY: immutable admitted table retained through the serialized call;
    // signed result is disjoint from table, context and callback publications.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.value.is_none()
        || table.classify.is_none()
        || table.named.is_none()
        || table.letter.is_none()
        || table.secondary.is_none()
        || table.restore.is_none()
        || table.hangul.is_none()
        || table.encode.is_none()
        || table.publish.is_none()
    {
        return -1;
    }
    let Ok(switched) = translate_letter::translate(
        &mut Engine(table),
        code,
        next,
        control,
        (current != u32::MAX).then_some(current),
    ) else {
        return -1;
    };
    // SAFETY: publish initialized consumed-byte result after successful control.
    unsafe {
        result.write(if switched { 0 } else { consumed });
    }
    0
}
