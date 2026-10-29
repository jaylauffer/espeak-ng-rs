//! Owned recursive replacement ABI; no foreign source/state loan across calls.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    replacement::{self, Class, Field, Host, PHRASE, TEXT},
    translate_word::Error,
};
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) store: Option<unsafe extern "C" fn(*mut c_void, u32, i32) -> i32>,
    pub(super) classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    pub(super) lower: Option<unsafe extern "C" fn(*mut c_void, u32) -> u32>,
    pub(super) main:
        Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, usize, *mut u8, *mut u32) -> i32>,
    pub(super) snapshot: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    pub(super) publish: Option<unsafe extern "C" fn(*mut c_void, *const u8) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
fn status(v: i32) -> Result<(), Error> {
    if v == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
impl Host for Engine<'_> {
    fn value(&self, f: Field) -> i32 {
        // SAFETY: validated scalar callback projects serialized live state.
        unsafe { self.0.value.unwrap()(self.0.context, f as u32) }
    }
    fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
        // SAFETY: checked scalar write; no foreign Rust loan is retained.
        status(unsafe { self.0.store.unwrap()(self.0.context, f as u32, v) })
    }
    fn classify(&self, c: u32, k: Class) -> bool {
        // SAFETY: scalar classification takes no source/output buffer loan.
        unsafe { self.0.classify.unwrap()(self.0.context, c, k as u32) != 0 }
    }
    fn lower(&self, c: u32) -> u32 {
        // SAFETY: scalar case mapping through validated callback.
        unsafe { self.0.lower.unwrap()(self.0.context, c) }
    }
    fn main(
        &mut self,
        source: Option<&mut [u8; PHRASE]>,
        initialized: usize,
        offset: usize,
        text: Option<&mut [u8; TEXT]>,
    ) -> Result<u32, Error> {
        let mut flags = 0;
        // SAFETY: disjoint owned initialized source/replacement and result;
        // source context is scoped and no pointer into scratch is kept.
        status(unsafe {
            self.0.main.unwrap()(
                self.0.context,
                source.map_or(std::ptr::null_mut(), |p| p.as_mut_ptr()),
                initialized,
                offset,
                text.map_or(std::ptr::null_mut(), |p| p.as_mut_ptr()),
                &mut flags,
            )
        })?;
        Ok(flags)
    }
    fn snapshot(&self, output: &mut [u8; 200]) -> Result<(), Error> {
        // SAFETY: initialized owned snapshot of the retained 200-byte output.
        status(unsafe { self.0.snapshot.unwrap()(self.0.context, output.as_mut_ptr()) })
    }
    fn publish(&mut self, output: &[u8; 200]) -> Result<(), Error> {
        // SAFETY: terminated owned pronunciation scoped to checked copy.
        status(unsafe { self.0.publish.unwrap()(self.0.context, output.as_ptr()) })
    }
}
unsafe fn callbacks<'a>(table: *const Callbacks) -> Option<&'a Callbacks> {
    if table.is_null() {
        return None;
    }
    // SAFETY: caller retains aligned immutable table disjoint from all owners.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.value.is_none()
        || t.store.is_none()
        || t.classify.is_none()
        || t.lower.is_none()
        || t.main.is_none()
        || t.snapshot.is_none()
        || t.publish.is_none()
    {
        None
    } else {
        Some(t)
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_replacement(
    table: *const Callbacks,
    enabled: u32,
    depth: i32,
    flags: *mut u32,
) -> i32 {
    if flags.is_null() {
        return -1;
    }
    // SAFETY: immutable table is retained for this serialized entry point.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    let mut replacement = [0; TEXT];
    let result = replacement::translate(
        &mut Engine(t),
        (enabled != 0).then_some(&mut replacement),
        depth,
    );
    match result {
        Ok(value) => {
            // SAFETY: aligned disjoint result slot retained by caller.
            unsafe { flags.write(value) };
            0
        }
        Err(_) => -1,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_split_ideographs(table: *const Callbacks, c: u32, p: u32) -> i32 {
    // SAFETY: retained immutable table, used only for scalar projections.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return 0;
    };
    replacement::split(&Engine(t), c, p) as i32
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_segment_replacement(
    table: *const Callbacks,
    input: *const u8,
    length: usize,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if input.is_null()
        || output.is_null()
        || length == 0
        || length > TEXT
        || capacity == 0
        || capacity > 320
    {
        return -1;
    }
    // SAFETY: retained immutable callback table is disjoint from buffers.
    let Some(t) = (unsafe { callbacks(table) }) else {
        return -1;
    };
    let mut text = [0; TEXT];
    // SAFETY: admitted initialized input prefix, copied without a foreign view.
    unsafe { std::ptr::copy_nonoverlapping(input, text.as_mut_ptr(), length) };
    if !text[..length].contains(&0) {
        return -1;
    }
    let mut phrase = [0; 320];
    match replacement::segment(&Engine(t), &text[..length], &mut phrase[..capacity]) {
        Ok(used) => {
            // SAFETY: completed owned prefix plus NUL fits admitted output.
            unsafe { std::ptr::copy_nonoverlapping(phrase.as_ptr(), output, used + 1) };
            0
        }
        Err(_) => -1,
    }
}
