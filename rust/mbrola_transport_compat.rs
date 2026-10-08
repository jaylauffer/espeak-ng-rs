//! Serialized legacy owner bridge. Views remain live until the next mutation.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::mbrola_transport::{self, Transport};

#[no_mangle]
extern "C" fn espeak_rs_mbr_transport_create() -> *mut Transport {
    Box::into_raw(Box::new(
        Transport::new(mbrola_transport::COMMAND_CAPACITY).expect("fixed capacity"),
    ))
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_transport_destroy(owner: *mut Transport) {
    if !owner.is_null() {
        // SAFETY: caller returns this unique owner once, after all views expire.
        drop(unsafe { Box::from_raw(owner) });
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_queue(
    owner: *mut Transport,
    bytes: *const u8,
    length: usize,
) -> i32 {
    if owner.is_null() || bytes.is_null() || length > isize::MAX as usize {
        return -1;
    }
    // SAFETY: serialized live owner and initialized input extent supplied by C.
    let (owner, bytes) = unsafe { (&mut *owner, std::slice::from_raw_parts(bytes, length)) };
    i32::from(owner.queue(bytes).is_ok())
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_front(owner: *const Transport, bytes: *mut *const u8) -> usize {
    if owner.is_null() || bytes.is_null() {
        return 0;
    }
    // SAFETY: caller supplies a live serialized owner and writable result slot.
    unsafe {
        let front = (*owner).front();
        *bytes = front.as_ptr();
        front.len()
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_consume(owner: *mut Transport, count: usize) -> i32 {
    if owner.is_null() {
        return -1;
    }
    // SAFETY: live owner uniquely accessed by the serialized C engine.
    if unsafe { (*owner).consume(count) }.is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_clear(owner: *mut Transport, stderr: i32) {
    if !owner.is_null() {
        // SAFETY: live owner uniquely accessed by the serialized C engine.
        let owner = unsafe { &mut *owner };
        owner.clear_commands();
        if stderr != 0 {
            owner.clear_stderr();
        }
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_stderr(
    owner: *mut Transport,
    bytes: *const u8,
    length: usize,
    eof: i32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if owner.is_null()
        || bytes.is_null()
        || output.is_null()
        || capacity == 0
        || length > isize::MAX as usize
        || capacity > isize::MAX as usize
    {
        return -1;
    }
    // SAFETY: C supplies a serialized owner and disjoint initialized input and
    // writable output extents, each valid for the duration of this call.
    unsafe {
        let owner = &mut *owner;
        let messages = owner.stderr(std::slice::from_raw_parts(bytes, length), eof != 0);
        if messages != 0 {
            let error = owner.error();
            let length = error.len().min(capacity - 1);
            std::ptr::copy_nonoverlapping(error.as_ptr(), output, length);
            *output.add(length) = 0;
        }
        messages.min(i32::MAX as usize) as i32
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_mbr_sample_rate(bytes: *const u8, length: usize) -> i32 {
    if bytes.is_null() || length != 44 {
        return -1;
    }
    // SAFETY: C supplies exactly 44 initialized readable header bytes.
    mbrola_transport::sample_rate(unsafe { std::slice::from_raw_parts(bytes, length) })
        .unwrap_or(-1)
}
