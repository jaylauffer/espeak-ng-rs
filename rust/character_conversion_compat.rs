//! Serialized character conversion ABI over checked numeric resource offsets.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    character_conversion::{self, Call, Class, Field, Host, ResultCode},
    translate_word::Error,
};
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    ignore: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    byte: Option<unsafe extern "C" fn(*mut c_void, u32, usize) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, usize, u8) -> i32>,
    classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    lower: Option<unsafe extern "C" fn(*mut c_void, u32) -> u32>,
    trace: Option<unsafe extern "C" fn(*mut c_void, usize, usize) -> i32>,
}
fn status(value: i32) -> Result<(), Error> {
    if value == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn value(&self, field: Field) -> i32 {
        // SAFETY: validated callback projects live serialized scalar state.
        unsafe { self.0.value.unwrap()(self.0.context, field as u32) }
    }
    fn ignore(&mut self, count: i32) -> Result<(), Error> {
        // SAFETY: numeric state write checks its retained owner's range.
        status(unsafe { self.0.ignore.unwrap()(self.0.context, count) })
    }
    fn byte(&self, kind: u32, index: usize) -> Option<u8> {
        // SAFETY: numeric source/resource callback checks current allocation.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, kind, index) }).ok()
    }
    fn write(&mut self, index: usize, value: u8) -> Result<(), Error> {
        // SAFETY: bounded source write retains no Rust source loan.
        status(unsafe { self.0.write.unwrap()(self.0.context, index, value) })
    }
    fn classify(&self, code: u32, class: Class) -> bool {
        // SAFETY: scalar locale/resource classification, no foreign borrow.
        unsafe { self.0.classify.unwrap()(self.0.context, code, class as u32) != 0 }
    }
    fn lower(&self, code: u32) -> u32 {
        // SAFETY: serialized language case primitive takes a scalar only.
        unsafe { self.0.lower.unwrap()(self.0.context, code) }
    }
    fn trace(&mut self, from: usize, to: usize) -> Result<(), Error> {
        // SAFETY: checked numeric resource offsets; the trace callback verifies
        // terminated strings within their retained dictionary allocation.
        status(unsafe { self.0.trace.unwrap()(self.0.context, from, to) })
    }
}
unsafe fn callbacks<'a>(table: *const Callbacks) -> Option<&'a Callbacks> {
    if table.is_null() {
        return None;
    }
    // SAFETY: caller retains an aligned immutable table, disjoint from context,
    // all resource owners and public result slots, for this serialized call.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.value.is_none()
        || t.ignore.is_none()
        || t.byte.is_none()
        || t.write.is_none()
        || t.classify.is_none()
        || t.lower.is_none()
        || t.trace.is_none()
    {
        None
    } else {
        Some(t)
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_character_translate(
    table: *const Callbacks,
    call: *const Call,
    out: *mut ResultCode,
) -> i32 {
    if call.is_null() || out.is_null() {
        return -1;
    }
    // SAFETY: validated immutable callback table stays retained for this call.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    // SAFETY: aligned initialized scalar input is disjoint from mutable owners
    // and result; copying it retains no foreign loan across primitive effects.
    let call = unsafe { call.read() };
    match character_conversion::translate(&mut Engine(t), call) {
        Ok(result) => {
            // SAFETY: aligned disjoint result slot is retained by caller.
            unsafe { out.write(result) };
            0
        }
        Err(_) => -1,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_character_upper(
    table: *const Callbacks,
    before: *const u8,
    code: u32,
    out: *mut i32,
) -> i32 {
    if before.is_null() || out.is_null() {
        return -1;
    }
    // SAFETY: validated immutable callback table stays retained for this call.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    let mut copy = [0u8; 3];
    // SAFETY: three initialized predecessor bytes, disjoint from owners and
    // output. Copy into owned scratch before any classifier callback executes.
    unsafe { std::ptr::copy_nonoverlapping(before, copy.as_mut_ptr(), 3) };
    let result = character_conversion::upper_in_word(&Engine(t), &copy, code);
    // SAFETY: aligned disjoint result slot remains live until return.
    unsafe { out.write(result as i32) };
    0
}
