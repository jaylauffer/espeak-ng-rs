//! Initialized projections for the serialized number pronunciation owner.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_digits::{self as digits, Host};
use crate::number_lookup::{Error, Host as LookupHost, PHONEME_BYTES};
use std::ffi::c_void;

#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    missing: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    text: Option<unsafe extern "C" fn(*mut c_void, u32) -> *const u8>,
    phoneme_type: Option<unsafe extern "C" fn(*mut c_void, u8) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
impl Engine<'_> {
    fn value(&self, field: u32) -> i32 {
        // SAFETY: admitted serialized state projection, field in 0..=5.
        unsafe { self.0.value.unwrap()(self.0.context, field) }
    }
}
impl LookupHost for Engine<'_> {
    fn lookup(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) -> i32 {
        // SAFETY: admitted dictionary primitive writes at most 200 terminated
        // bytes to initialized writable scratch; key is live and terminated.
        unsafe { self.0.lookup.unwrap()(self.0.context, key.as_ptr(), output.as_mut_ptr()) }
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
        // SAFETY: admitted serialized store; no foreign borrow held.
        unsafe { self.0.missing.unwrap()(self.0.context, value) }
    }
}
impl Host for Engine<'_> {
    fn numbers2(&self) -> i32 {
        self.value(3)
    }
    fn digit_count(&self) -> i32 {
        self.value(4)
    }
    fn language(&self) -> i32 {
        self.value(5)
    }
    fn text(&self, kind: u32, output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: admitted fresh serialized pointer projection, kind 0..=2.
        let text = unsafe { self.0.text.unwrap()(self.0.context, kind) };
        if text.is_null() {
            return Err(Error::Phonemes);
        }
        let limit = if kind == 0 { 50 } else { 12 };
        for (index, byte) in output[..limit].iter_mut().enumerate() {
            // SAFETY: owner admits a live initialized text prefix through NUL
            // within the 50/12-byte source extent. Stop before its unused tail.
            *byte = unsafe { text.add(index).read() };
            if *byte == 0 {
                return Ok(());
            }
        }
        Err(Error::Phonemes)
    }
    fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
        // SAFETY: admitted checked unsigned-code projection; missing slots -1.
        let kind = unsafe { self.0.phoneme_type.unwrap()(self.0.context, code) };
        if kind < 0 {
            Err(Error::Phonemes)
        } else {
            Ok(kind)
        }
    }
}
unsafe fn admit<'a>(table: *const Callbacks) -> Option<Engine<'a>> {
    if table.is_null() {
        return None;
    }
    // SAFETY: caller admits a live immutable table, disjoint from all outputs,
    // through return; its serialized primitives uphold their span contracts.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.lookup.is_none()
        || table.value.is_none()
        || table.missing.is_none()
        || table.text.is_none()
        || table.phoneme_type.is_none()
    {
        return None;
    }
    Some(Engine(table))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_num2(
    table: *const Callbacks,
    value: i32,
    plex: i32,
    control: i32,
    output: *mut u8,
    capacity: usize,
    used_and: *mut i32,
) -> i32 {
    if output.is_null() || used_and.is_null() || capacity == 0 || capacity > PHONEME_BYTES {
        return -1;
    }
    // SAFETY: caller admits the serialized table as documented by admit.
    let Some(mut host) = (unsafe { admit(table) }) else {
        return -1;
    };
    let mut scratch = [0; PHONEME_BYTES];
    let Ok(result) = digits::two(&mut host, value, plex, control, &mut scratch[..capacity]) else {
        return -1;
    };
    let length = scratch.iter().position(|byte| *byte == 0).unwrap();
    // SAFETY: admitted disjoint writable scalar/output spans. Only initialized
    // terminated prefix is published; no foreign unfilled tail is borrowed.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1);
        used_and.write(result);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_num3(
    table: *const Callbacks,
    value: i32,
    plex: i32,
    control: i32,
    suppress: i32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if output.is_null() || capacity == 0 || capacity > PHONEME_BYTES {
        return -1;
    }
    // SAFETY: caller admits the serialized table as documented by admit.
    let Some(mut host) = (unsafe { admit(table) }) else {
        return -1;
    };
    let mut scratch = [0; PHONEME_BYTES];
    if digits::three(
        &mut host,
        value,
        plex,
        control,
        suppress != 0,
        &mut scratch[..capacity],
    )
    .is_err()
    {
        return -1;
    }
    let length = scratch.iter().position(|byte| *byte == 0).unwrap();
    // SAFETY: capacity-sized writable output admitted, disjoint from table;
    // copy only the validated initialized prefix, never project foreign tail.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1);
    }
    0
}
