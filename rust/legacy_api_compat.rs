//! C engine callbacks for the native legacy API control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::legacy_api::{self, Host};
use std::ffi::{c_char, c_void};
use std::ptr;
#[repr(C)]
pub struct Callbacks {
    path: unsafe extern "C" fn(*const c_char),
    initialize: unsafe extern "C" fn(*mut *mut c_void) -> u32,
    output: unsafe extern "C" fn(i32, i32, *const c_char) -> u32,
    rate: unsafe extern "C" fn() -> i32,
    print: unsafe extern "C" fn(u32, *mut c_void, *mut c_void),
    clear: unsafe extern "C" fn(*mut *mut c_void),
    compile: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        *mut c_void,
        i32,
        *mut *mut c_void,
    ) -> u32,
    exit: unsafe extern "C" fn(i32),
}
struct Engine<'a> {
    cb: &'a Callbacks,
    path: *const c_char,
    dictionary: *const c_char,
    log: *mut c_void,
    errors: *mut c_void,
    events: *mut i32,
    flags: i32,
    context: *mut c_void,
}
impl Host for Engine<'_> {
    fn path(&mut self) {
        // SAFETY: admitted table/path, borrowed for the engine initialization.
        unsafe {
            (self.cb.path)(self.path);
        }
    }
    fn initialize(&mut self) -> u32 {
        // SAFETY: exclusive local context slot, callback stores its owned
        // context there and cannot retain this stack address.
        unsafe { (self.cb.initialize)(&mut self.context) }
    }
    fn diagnose(&mut self, status: u32) {
        // SAFETY: live stream and context through the callback.
        unsafe {
            (self.cb.print)(status, self.errors, self.context);
        }
    }
    fn clear(&mut self) {
        // SAFETY: local owned context returned exactly once to its engine.
        unsafe {
            (self.cb.clear)(&mut self.context);
        }
    }
    fn output(&mut self, mode: i32, length: i32) {
        // SAFETY: scalar options, default null device, serialized owner.
        unsafe {
            (self.cb.output)(mode, length, ptr::null());
        }
    }
    fn events(&mut self, flags: i32) {
        // SAFETY: caller supplies unique live engine-global event flags.
        unsafe {
            *self.events = flags;
        }
    }
    fn rate(&mut self) -> i32 {
        // SAFETY: live serialized engine callback table.
        unsafe { (self.cb.rate)() }
    }
    fn compile(&mut self) -> u32 {
        // SAFETY: admitted path/dictionary/stream and exclusive context slot;
        // the compiler borrows them for this call without retaining the slot.
        unsafe {
            (self.cb.compile)(
                self.path,
                self.dictionary,
                self.log,
                self.flags,
                &mut self.context,
            )
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_legacy_initialize(
    cb: *const Callbacks,
    errors: *mut c_void,
    events: *mut i32,
    output: i32,
    length: i32,
    path: *const c_char,
    options: i32,
) -> i32 {
    if cb.is_null() || events.is_null() {
        return -1;
    }
    // SAFETY: admitted immutable table, alive throughout all callbacks.
    let cb = unsafe { &*cb };
    let mut host = Engine {
        cb,
        path,
        dictionary: ptr::null(),
        log: ptr::null_mut(),
        errors,
        events,
        flags: 0,
        context: ptr::null_mut(),
    };
    match legacy_api::initialize(&mut host, output, length, options) {
        Ok(rate) => rate,
        Err(code) => {
            // SAFETY: callback preserves the legacy non-returning exit(code).
            // If a test host returns, refuse further initialization work.
            unsafe {
                (cb.exit)(code);
            }
            -1
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_legacy_compile(
    cb: *const Callbacks,
    errors: *mut c_void,
    path: *const c_char,
    dictionary: *const c_char,
    log: *mut c_void,
    flags: i32,
) {
    if cb.is_null() {
        return;
    }
    // SAFETY: admitted immutable table, borrowed for the complete call.
    let cb = unsafe { &*cb };
    let mut host = Engine {
        cb,
        path,
        dictionary,
        log,
        errors,
        events: ptr::null_mut(),
        flags,
        context: ptr::null_mut(),
    };
    legacy_api::compile(&mut host);
}
