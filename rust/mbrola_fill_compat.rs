//! Serialized C-reader adapter for the native MBROLA sample cursor.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::mbrola_fill::{Fill, Read, Status};
use std::ffi::c_void;
use std::slice;

type Reader = unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32;

#[no_mangle]
extern "C" fn espeak_rs_mbrola_fill_create() -> *mut Fill {
    Box::into_raw(Box::default())
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_fill_destroy(owner: *mut Fill) {
    if !owner.is_null() {
        // SAFETY: unique live allocation from create; no call remains active.
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_fill(
    owner: *mut Fill,
    output: *mut u8,
    length: usize,
    written: *mut usize,
    rate: i32,
    milliseconds: i32,
    resume: u32,
    amplitude: i32,
    context: *mut c_void,
    reader: Option<Reader>,
) -> i32 {
    let Some(reader) = reader else {
        return -1;
    };
    if owner.is_null()
        || output.is_null()
        || written.is_null()
        || length > i32::MAX as usize
        || length % 2 != 0
        || resume > 1
        || output as usize % 2 != 0
    {
        return -1;
    }
    // SAFETY: owner, output, written and reader context are live, exclusive
    // and disjoint. Reader initializes at most the requested number of sample
    // pairs, never retains this pointer or reenters/invalidates the owner.
    // Alignment above also permits the compatibility reader's short* cast.
    let (owner, output) = unsafe { (&mut *owner, slice::from_raw_parts_mut(output, length)) };
    let outcome = owner.fill(
        output,
        rate,
        milliseconds,
        resume != 0,
        amplitude,
        |bytes| {
            // SAFETY: initialized bounded exclusive output; synchronous reader
            // borrows it only during the callback under the contract above.
            match unsafe { reader(context, bytes.as_mut_ptr(), (bytes.len() / 2) as i32) } {
                n if n > 0 => Read::Samples(n as usize),
                0 => Read::End,
                -2 => Read::Pending,
                _ => Read::Failed,
            }
        },
    );
    match outcome {
        Ok(result) => {
            // SAFETY: exclusive initialized size_t output, disjoint from PCM.
            unsafe {
                *written = result.bytes;
            }
            match result.status {
                Status::Complete => 0,
                Status::More => 1,
                Status::Pending => 2,
                Status::End => 3,
            }
        }
        Err(_) => -1,
    }
}
