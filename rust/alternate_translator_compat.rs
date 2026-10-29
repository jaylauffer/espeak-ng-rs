//! Serialized lifecycle ABI: copied names and checked retained resource slots.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    alternate_translator::{self, Effect, Host},
    translate_word::Error,
};
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) present: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) cached: Option<unsafe extern "C" fn(*mut c_void, u32, usize) -> i32>,
    pub(super) cache: Option<unsafe extern "C" fn(*mut c_void, u32, usize, u8) -> i32>,
    pub(super) lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8) -> i32>,
    pub(super) create: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8) -> i32>,
    pub(super) load: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) main_table: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) effect: Option<unsafe extern "C" fn(*mut c_void, u32, u32, i32) -> i32>,
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
    fn present(&self, slot: u32) -> bool {
        // SAFETY: validated serialized callback reads a checked resource slot.
        unsafe { self.0.present.unwrap()(self.0.context, slot) != 0 }
    }
    fn cached(&self, slot: u32, index: usize) -> Option<u8> {
        // SAFETY: checked numeric index; no foreign cache loan is retained.
        u8::try_from(unsafe { self.0.cached.unwrap()(self.0.context, slot, index) }).ok()
    }
    fn cache(&mut self, slot: u32, index: usize, byte: u8) -> Result<(), Error> {
        // SAFETY: checked serialized cache write, no source loans cross it.
        status(unsafe { self.0.cache.unwrap()(self.0.context, slot, index, byte) })
    }
    fn lookup(&mut self, name: &[u8; 20]) -> Result<i32, Error> {
        // SAFETY: initialized terminated owned name; primitive retains none.
        Ok(unsafe { self.0.lookup.unwrap()(self.0.context, name.as_ptr()) })
    }
    fn create(&mut self, slot: u32, name: &[u8; 20]) -> Result<(), Error> {
        // SAFETY: checked slot and owned terminated name copied by primitive.
        status(unsafe { self.0.create.unwrap()(self.0.context, slot, name.as_ptr()) })
    }
    fn load(&mut self, slot: u32) -> Result<i32, Error> {
        // SAFETY: checked live translator slot, serialized dictionary loading.
        Ok(unsafe { self.0.load.unwrap()(self.0.context, slot) })
    }
    fn main_table(&self) -> i32 {
        // SAFETY: serialized scalar voice-resource projection.
        unsafe { self.0.main_table.unwrap()(self.0.context) }
    }
    fn effect(&mut self, slot: u32, effect: Effect, value: i32) -> Result<(), Error> {
        // SAFETY: checked numeric slot/effect, no loan survives resource effects.
        status(unsafe { self.0.effect.unwrap()(self.0.context, slot, effect as u32, value) })
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_alternate_translator(
    callbacks: *const Callbacks,
    operation: u32,
    name: *const u8,
    length: usize,
    out: *mut i32,
) -> i32 {
    if callbacks.is_null() || out.is_null() {
        return -1;
    }
    // SAFETY: aligned immutable table stays retained and disjoint from mutable
    // resource/cache owners and output throughout this serialized call.
    let t = unsafe { &*callbacks };
    if t.context.is_null()
        || t.present.is_none()
        || t.cached.is_none()
        || t.cache.is_none()
        || t.lookup.is_none()
        || t.create.is_none()
        || t.load.is_none()
        || t.main_table.is_none()
        || t.effect.is_none()
    {
        return -1;
    }
    let mut owned = [0; 20];
    if operation < 2 {
        if name.is_null() || length >= owned.len() {
            return -1;
        }
        // SAFETY: caller provides a readable initialized name of length bytes,
        // disjoint from output and table. Copy before any primitive effect;
        // no foreign name slice is held while callbacks can mutate owners.
        unsafe { std::ptr::copy_nonoverlapping(name, owned.as_mut_ptr(), length) };
    }
    let mut h = Engine(t);
    let result = match operation {
        0 | 1 => alternate_translator::set(&mut h, operation, &owned[..length]),
        2 => alternate_translator::free(&mut h).map(|()| 0),
        _ => Err(Error::State),
    };
    match result {
        Ok(value) => {
            // SAFETY: aligned disjoint caller output remains valid for the call.
            unsafe { out.write(value) };
            0
        }
        Err(_) => -1,
    }
}
