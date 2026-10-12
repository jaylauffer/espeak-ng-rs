//! Copied NextItem inputs, short source borrow, then synchronous effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::compiler_source_compat::{self, Session};
use crate::{
    compiler_items::{self, Host},
    compiler_program::Token,
};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Descriptor {
    source: *mut Session,
    slot: i32,
    kind: i32,
    line: i32,
    terminator: i32,
    previous: *const u8,
    context: *mut c_void,
    lookup: Option<unsafe extern "C" fn(*mut c_void, i32, *const u8, usize) -> i32>,
    error: Option<unsafe extern "C" fn(*mut c_void, i32, *const u8, usize)>,
}
#[repr(C)]
pub struct Outcome {
    token: Token,
    line: i32,
}
struct Effects {
    d: Descriptor,
}
impl Host for Effects {
    fn error(&mut self, line: i32, bytes: &[u8]) {
        // SAFETY: admitted live synchronous callback; owned bounded message.
        unsafe {
            (self.d.error.unwrap())(self.d.context, line, bytes.as_ptr(), bytes.len());
        }
    }
    fn lookup(&mut self, line: i32, name: &[u8]) -> i32 {
        // SAFETY: admitted live synchronous callback; owned bounded name.
        unsafe { (self.d.lookup.unwrap())(self.d.context, line, name.as_ptr(), name.len()) }
    }
}
static BUSY: AtomicBool = AtomicBool::new(false);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
/// Caller owns the live source session on this thread, 256 initialized previous
/// bytes, writable result and synchronous callbacks. All inputs copy before
/// effects; source borrows end before callbacks and overlapping publication.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_item(
    descriptor: *const Descriptor,
    output: *mut Outcome,
) -> i32 {
    if descriptor.is_null() || output.is_null() {
        return 22;
    }
    // SAFETY: initialized descriptor copied before any effects.
    let d = unsafe { descriptor.read() };
    if d.previous.is_null() || d.lookup.is_none() || d.error.is_none() {
        return 22;
    }
    let mut previous = Token {
        terminator: d.terminator,
        ..Token::default()
    };
    // SAFETY: initialized declared previous-token extent, copied before callbacks.
    unsafe {
        std::ptr::copy_nonoverlapping(d.previous, previous.text.as_mut_ptr(), 256);
    }
    if BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return 11;
    }
    let _guard = Guard;
    let mut raw = std::mem::MaybeUninit::uninit();
    // SAFETY: caller's live serialized source session; scanner publishes a full
    // token on success and retains no foreign loans or source borrow afterward.
    let read = unsafe {
        compiler_source_compat::espeak_rs_compiler_source_next(
            d.source,
            d.slot,
            d.kind,
            d.line,
            raw.as_mut_ptr(),
        )
    };
    let source = if read == 0 {
        None
    } else {
        // SAFETY: successful scanner initialized the entire owned result.
        Some(unsafe { raw.assume_init() })
    };
    match compiler_items::complete(source, d.kind, previous, d.line, &mut Effects { d }) {
        Ok((token, line)) => {
            // SAFETY: writable output, all source borrows and effects finished.
            unsafe {
                output.write(Outcome { token, line });
            }
            0
        }
        Err(status) => status,
    }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_keyword(
    kind: i32,
    name: *const u8,
    length: usize,
    output: *mut i32,
) -> i32 {
    if output.is_null() || length > 255 || (length != 0 && name.is_null()) {
        return 22;
    }
    let mut bytes = [0; 256];
    if length != 0 {
        // SAFETY: declared readable bytes copied before overlapping publication.
        unsafe {
            std::ptr::copy_nonoverlapping(name, bytes.as_mut_ptr(), length);
        }
    }
    let (category, value) = compiler_items::keyword(kind, &bytes[..length]);
    // SAFETY: caller provides two writable i32 elements; inputs already copied.
    unsafe {
        output.write(category);
        output.add(1).write(value);
    }
    0
}
