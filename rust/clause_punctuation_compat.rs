//! Serialized compatibility callbacks for native punctuation announcement.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::clause_punctuation::{self as punctuation, Error};
use std::{ffi::c_void, ptr};
#[derive(Clone, Copy)]
#[repr(C)]
struct Context {
    owner: *mut c_void,
    icon: Option<unsafe extern "C" fn(i32) -> i32>,
    name: Option<unsafe extern "C" fn(*mut c_void, i32, u32, *mut [u8; 74]) -> i32>,
    eof: Option<unsafe extern "C" fn() -> i32>,
    read: Option<unsafe extern "C" fn() -> i32>,
    unread: Option<unsafe extern "C" fn(i32)>,
    unread_second: Option<unsafe extern "C" fn(i32)>,
    flags: *const i32,
    speed: *const i32,
}
struct Host(Context);
impl punctuation::Host for Host {
    fn soundicon(&mut self, code: i32) -> i32 {
        // SAFETY: admitted serialized owner callback; no foreign borrow held.
        unsafe { self.0.icon.expect("admitted icon")(code) }
    }
    fn name(&mut self, code: i32, period: bool) -> Result<Option<[u8; 74]>, Error> {
        let mut result = [0; 74];
        // SAFETY: retained owner and exclusive initialized local name output;
        // callback copies terminated text, retaining no Rust/foreign owner borrow.
        match unsafe {
            self.0.name.expect("admitted name")(self.0.owner, code, u32::from(period), &mut result)
        } {
            0 => Ok(Some(result)),
            1 => Ok(None),
            _ => Err(Error::Backend),
        }
    }
    fn eof(&self) -> bool {
        // SAFETY: pure serialized source predicate; no input/output borrow held.
        unsafe { self.0.eof.expect("admitted EOF")() != 0 }
    }
    fn read(&mut self) -> i32 {
        // SAFETY: serialized source callback owns its decoder/counter fields.
        unsafe { self.0.read.expect("admitted read")() }
    }
    fn unread(&mut self, code: i32) {
        // SAFETY: serialized source callback owns its initialized replay scalar.
        unsafe { self.0.unread.expect("admitted unread")(code) }
    }
    fn unread_second(&mut self, code: i32) {
        // SAFETY: serialized source callback owns its initialized second replay.
        unsafe { self.0.unread_second.expect("admitted second unread")(code) }
    }
    fn flags(&self) -> i32 {
        // SAFETY: initialized retained flag scalar, snapshotted after callbacks.
        unsafe { *self.0.flags }
    }
    fn speed(&self) -> i32 {
        // SAFETY: initialized retained speed scalar, snapshotted after callbacks.
        unsafe { *self.0.speed }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_announce(
    context: *const Context,
    code: i32,
    next: *mut i32,
    output: *mut u8,
    capacity: usize,
    offset: *mut i32,
    end_clause: u32,
) -> i32 {
    if context.is_null()
        || next.is_null()
        || output.is_null()
        || offset.is_null()
        || end_clause > 1
        || capacity > i32::MAX as usize
    {
        return -1;
    }
    // SAFETY: initialized context and exclusive copied scalar snapshots. Mutable
    // context/source/output fields are disjoint, live and serialized through all
    // callbacks. No callback may invalidate/reenter this output/announcement.
    let (context, next_value, offset_value) = unsafe { (*context, *next, *offset) };
    if context.icon.is_none()
        || context.name.is_none()
        || context.eof.is_none()
        || context.read.is_none()
        || context.unread.is_none()
        || context.unread_second.is_none()
        || context.flags.is_null()
        || context.speed.is_null()
    {
        return -1;
    }
    let Ok(position) = usize::try_from(offset_value) else {
        return -1;
    };
    let result = punctuation::announce(
        punctuation::Request {
            code,
            next: next_value,
            offset: position,
            end_clause: end_clause != 0,
            capacity,
        },
        &mut Host(context),
    );
    let Ok(result) = result else { return -1 };
    if result.write {
        // SAFETY: complete prefix+NUL admitted by native plan; copied scratch
        // source disjoint from caller capacity. No mutable slice borrows unused
        // foreign bytes, and no foreign reference survives a callback.
        unsafe {
            ptr::copy_nonoverlapping(
                result.bytes.as_ptr(),
                output.add(position),
                result.length + 1,
            );
            *offset = (position + result.length) as i32;
            *next = result.next;
        }
    }
    result.terminator
}
