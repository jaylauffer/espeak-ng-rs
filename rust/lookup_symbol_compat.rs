//! Bounded symbol publication over the native dictionary-list engine.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::lookup_list_compat::{Callbacks as ListCallbacks, Engine as ListEngine};
use crate::lookup_symbol::{self, Host, SOURCE_BYTES};
use crate::number_lookup::PHONEME_BYTES;
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    list: ListCallbacks,
    byte: Option<unsafe extern "C" fn(*mut c_void, usize) -> i32>,
    say_as: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    set_say_as: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    translate: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8) -> i32>,
}
struct Engine<'a> {
    table: &'a Callbacks,
    list: ListEngine<'a>,
}
impl<'a> Host for Engine<'a> {
    type List = ListEngine<'a>;
    fn list(&mut self) -> &mut Self::List {
        &mut self.list
    }
    fn byte(&self, position: usize) -> Option<u8> {
        // SAFETY: bounded projection of original or owned replacement bytes.
        u8::try_from(unsafe { self.table.byte.unwrap()(self.table.list.context(), position) }).ok()
    }
    fn say_as(&self) -> i32 {
        // SAFETY: fresh serialized scalar projection.
        unsafe { self.table.say_as.unwrap()(self.table.list.context()) }
    }
    fn set_say_as(&mut self, value: i32) {
        // SAFETY: serialized scalar store.
        unsafe { self.table.set_say_as.unwrap()(self.table.list.context(), value) }
    }
    fn translate(&mut self, text: &mut [u8; SOURCE_BYTES], out: &mut [u8; PHONEME_BYTES]) -> i32 {
        // SAFETY: fully initialized owned source and scratch are disjoint. The
        // child scopes/restores its source window, retaining no pointer.
        unsafe {
            self.table.translate.unwrap()(
                self.table.list.context(),
                text.as_mut_ptr(),
                out.as_mut_ptr(),
            )
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_symbol(
    table: *const Callbacks,
    output: *mut u8,
    capacity: usize,
    flags: *mut i32,
) -> i32 {
    if table.is_null()
        || output.is_null()
        || flags.is_null()
        || capacity == 0
        || capacity > PHONEME_BYTES
    {
        return -1;
    }
    // SAFETY: live immutable callback table, disjoint output/result and owner;
    // every primitive retains no loan of scratch/source through return.
    let table = unsafe { &*table };
    let Some(list) = table.list.engine() else {
        return -1;
    };
    if table.byte.is_none()
        || table.say_as.is_none()
        || table.set_say_as.is_none()
        || table.translate.is_none()
    {
        return -1;
    }
    let mut scratch = [0; PHONEME_BYTES];
    let Ok(result) = lookup_symbol::symbol(&mut Engine { table, list }, &mut scratch[..capacity])
    else {
        return -1;
    };
    let length = scratch.iter().position(|byte| *byte == 0).unwrap();
    // SAFETY: publish only the validated initialized prefix and signed flag
    // result. The status return is separate so negative flag bits stay valid.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1);
        flags.write(result);
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_flags(table: *const ListCallbacks, output: *mut u32) -> i32 {
    if table.is_null() || output.is_null() {
        return -1;
    }
    // SAFETY: immutable live table/context; output is two exclusive words
    // disjoint from table/owner/source. Never read foreign output before calls.
    let table = unsafe { &*table };
    let Some(mut engine) = table.engine() else {
        return -1;
    };
    let Ok(flags) = lookup_symbol::flags(&mut engine) else {
        return -1;
    };
    // SAFETY: publish the two validated initialized flags only after success.
    unsafe {
        std::ptr::copy_nonoverlapping(flags.as_ptr(), output, 2);
    }
    0
}
