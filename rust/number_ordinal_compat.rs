//! Scalar source/state projections for ordinal context; no foreign slice loan.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_ordinal::{self, Host};
use std::ffi::c_void;

#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    byte: Option<unsafe extern "C" fn(*mut c_void, isize) -> u8>,
    space: Option<unsafe extern "C" fn(*mut c_void, usize)>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> u32>,
    classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    translate: Option<unsafe extern "C" fn(*mut c_void, usize) -> u32>,
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn byte(&self, offset: isize) -> u8 {
        // SAFETY: admitted scalar callback bounds initialized source reads.
        unsafe { self.0.byte.unwrap()(self.0.context, offset) }
    }
    fn space(&mut self, offset: usize) {
        // SAFETY: admitted source owner bounds mutation; no source borrow held.
        unsafe { self.0.space.unwrap()(self.0.context, offset) }
    }
    fn value(&self, field: u32) -> u32 {
        // SAFETY: admitted live projection, known field in 0..=4.
        unsafe { self.0.value.unwrap()(self.0.context, field) }
    }
    fn alpha(&self, c: u32) -> bool {
        // SAFETY: admitted engine classification of a decoded character code.
        unsafe { self.0.classify.unwrap()(self.0.context, c, 0) != 0 }
    }
    fn digit(&self, c: u32) -> bool {
        // SAFETY: admitted Unicode digit classification primitive.
        unsafe { self.0.classify.unwrap()(self.0.context, c, 1) != 0 }
    }
    fn translate(&mut self, offset: usize) -> u32 {
        // SAFETY: admitted serialized translation, scoped to original owner.
        unsafe { self.0.translate.unwrap()(self.0.context, offset) }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_number_dot(
    table: *const Callbacks,
    length: usize,
    end: usize,
    roman: i32,
) -> i32 {
    if table.is_null() || length == 0 || length > 800 || end >= length {
        return -1;
    }
    // SAFETY: caller admits immutable live callback table through return,
    // disjoint from mutable source/state reached only through its context.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.byte.is_none()
        || table.space.is_none()
        || table.value.is_none()
        || table.classify.is_none()
        || table.translate.is_none()
    {
        return -1;
    }
    number_ordinal::dot(&mut Engine(table), end, roman != 0)
}
