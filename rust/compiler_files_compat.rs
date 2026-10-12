//! Serialized source-file controller: bounded copied root, owned stack/paths.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_files::{self, Frame, Host, PATH},
    compiler_program::Token,
};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
};
type Status = unsafe extern "C" fn(*mut c_void) -> i32;
type Named = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Descriptor {
    context: *mut c_void,
    root: *const u8,
    width: usize,
    reset: Option<Status>,
    eof: Option<Status>,
    next: Option<unsafe extern "C" fn(*mut c_void, i32, *mut Token) -> i32>,
    frame: Option<unsafe extern "C" fn(*mut c_void, *mut Frame) -> i32>,
    project: Option<unsafe extern "C" fn(*mut c_void, *const Frame) -> i32>,
    open: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize, usize) -> i32>,
    close: Option<Status>,
    tables: Option<Status>,
    end: Option<Status>,
    start: Option<Named>,
    compile: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    terminate: Option<Status>,
    error: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize)>,
}
struct Effects {
    d: Descriptor,
}
fn status(value: i32) -> Result<(), i32> {
    if value == 0 {
        Ok(())
    } else {
        Err(value)
    }
}
impl Effects {
    fn effect(&mut self, callback: Option<Status>) -> Result<(), i32> {
        // SAFETY: admitted live synchronous callback; no foreign loans.
        status(unsafe { callback.unwrap()(self.d.context) })
    }
}
impl Host for Effects {
    fn reset(&mut self) -> Result<(), i32> {
        self.effect(self.d.reset)
    }
    fn eof(&mut self) -> bool {
        // SAFETY: admitted live synchronous callback.
        unsafe { self.d.eof.unwrap()(self.d.context) != 0 }
    }
    fn next(&mut self, kind: i32) -> Result<Token, i32> {
        let mut token = Token::default();
        // SAFETY: initialized owned result scratch; admitted synchronous callback.
        status(unsafe { self.d.next.unwrap()(self.d.context, kind, &mut token) })?;
        if !token.text.contains(&0) {
            return Err(22);
        }
        Ok(token)
    }
    fn frame(&mut self) -> Result<Frame, i32> {
        let mut frame = Frame::default();
        // SAFETY: initialized owned result scratch; admitted synchronous callback.
        status(unsafe { self.d.frame.unwrap()(self.d.context, &mut frame) })?;
        if !frame.name.contains(&0) {
            return Err(22);
        }
        Ok(frame)
    }
    fn project(&mut self, frame: Frame) -> Result<(), i32> {
        // SAFETY: initialized owned copied row, callback snapshots synchronously.
        status(unsafe { self.d.project.unwrap()(self.d.context, &frame) })
    }
    fn open(&mut self, path: &[u8], depth: usize) -> Result<i32, i32> {
        // SAFETY: owned bounded path loan; admitted synchronous callback.
        let value =
            unsafe { self.d.open.unwrap()(self.d.context, path.as_ptr(), path.len(), depth) };
        if value < 0 {
            Err(value.wrapping_neg())
        } else {
            Ok(value)
        }
    }
    fn close(&mut self) -> Result<(), i32> {
        self.effect(self.d.close)
    }
    fn tables(&mut self) -> i32 {
        // SAFETY: admitted synchronous scalar snapshot.
        unsafe { self.d.tables.unwrap()(self.d.context) }
    }
    fn end_table(&mut self) -> Result<(), i32> {
        self.effect(self.d.end)
    }
    fn start_table(&mut self, name: &[u8]) -> Result<(), i32> {
        // SAFETY: owned bounded name loan; admitted synchronous callback.
        status(unsafe { self.d.start.unwrap()(self.d.context, name.as_ptr(), name.len()) })
    }
    fn compile(&mut self, phoneme: bool) -> Result<(), i32> {
        // SAFETY: admitted synchronous definition effect, separate owner.
        status(unsafe { self.d.compile.unwrap()(self.d.context, i32::from(phoneme)) })
    }
    fn terminate(&mut self) -> Result<(), i32> {
        self.effect(self.d.terminate)
    }
    fn error(&mut self, bytes: &[u8]) {
        // SAFETY: owned bounded message loan; admitted synchronous callback.
        unsafe {
            self.d.error.unwrap()(self.d.context, bytes.as_ptr(), bytes.len());
        }
    }
}
static BUSY: AtomicBool = AtomicBool::new(false);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
/// Initialized descriptor and root bytes of width (2..4096), writable result.
/// Callbacks are live/synchronous, cannot retain loans or unwind. Root and
/// descriptor copy before effects; no foreign references span callbacks.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_files(
    descriptor: *const Descriptor,
    output: *mut i32,
) -> i32 {
    if descriptor.is_null() || output.is_null() {
        return 22;
    }
    // SAFETY: initialized descriptor copied before any effects/publication.
    let d = unsafe { descriptor.read() };
    if d.root.is_null()
        || !(2..=PATH).contains(&d.width)
        || d.reset.is_none()
        || d.eof.is_none()
        || d.next.is_none()
        || d.frame.is_none()
        || d.project.is_none()
        || d.open.is_none()
        || d.close.is_none()
        || d.tables.is_none()
        || d.end.is_none()
        || d.start.is_none()
        || d.compile.is_none()
        || d.terminate.is_none()
        || d.error.is_none()
    {
        return 22;
    }
    let mut root = [0; PATH];
    // SAFETY: caller supplies initialized root extent; copied before callbacks.
    unsafe {
        std::ptr::copy_nonoverlapping(d.root, root.as_mut_ptr(), d.width);
    }
    let Some(n) = root[..d.width].iter().position(|b| *b == 0) else {
        return 22;
    };
    if BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return 11;
    }
    let _guard = Guard;
    match compiler_files::run(&root[..n], d.width, &mut Effects { d }) {
        Ok(done) => {
            // SAFETY: writable scalar, all foreign inputs copied/effects finished.
            unsafe {
                output.write(i32::from(done));
            }
            0
        }
        Err(status) => status,
    }
}
