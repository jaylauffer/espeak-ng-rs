//! Compatibility random state and admitted raw byte/std-stream operations.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::common_primitives::{self as primitives, Random};
use std::{
    ffi::c_long,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};
static RANDOM: AtomicU32 = AtomicU32::new(0);
// C long is32 bits on Windows and some Unix targets,64 on the current host.
#[allow(clippy::unnecessary_cast)]
fn long_value(value: c_long) -> i64 {
    value as i64
}
#[no_mangle]
extern "C" fn espeak_rs_srand(seed: c_long) {
    let mut random = Random::default();
    random.seed(long_value(seed));
    RANDOM.store(random.state(), Ordering::Relaxed);
}
#[no_mangle]
extern "C" fn espeak_rs_rand(min: c_long, max: c_long) -> c_long {
    // Validate in the host's long width before the owned i64 calculation.
    if max
        .checked_sub(min)
        .and_then(|v| v.checked_add(1))
        .is_none_or(|v| v == 0)
    {
        return 0;
    }
    // Explicit loop: `fetch_update` is deprecated on current stable and its
    // replacement `try_update` is newer than the crate's rust-version.
    let mut state = RANDOM.load(Ordering::Relaxed);
    loop {
        let mut random = Random::from_state(state);
        let Some(result) = random
            .next(long_value(min), long_value(max))
            .ok()
            .and_then(|value| c_long::try_from(value).ok())
        else {
            return 0;
        };
        match RANDOM.compare_exchange_weak(
            state,
            random.state(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return result,
            Err(current) => state = current,
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_copy0(output: *mut u8, input: *const u8, capacity: usize) -> i32 {
    if output.is_null() || input.is_null() || capacity == 0 || capacity > isize::MAX as usize {
        return -1;
    }
    let mut length = 0;
    while length < capacity - 1 {
        // SAFETY: immutable caller source remains initialized/readable through
        // NUL or capacity-1; output is disjoint and not yet touched.
        if unsafe { *input.add(length) } == 0 {
            break;
        }
        length += 1;
    }
    // SAFETY: complete destination capacity admitted; copied initialized source
    // prefix is disjoint. Sparse writes never borrow undefined output tails.
    unsafe {
        ptr::copy_nonoverlapping(input, output, length);
        ptr::write_bytes(output.add(length), 0, capacity - length);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_read4(stream: *mut libc::FILE) -> i32 {
    if stream.is_null() {
        return -1;
    }
    primitives::read4(|| {
        // SAFETY: retained serialized live stdio stream, with no Rust engine or
        // buffer borrow across this possibly blocking OS/CRT read. Caller owns
        // execution and must use its worker for substantial/blocking I/O.
        unsafe { libc::fgetc(stream) }
    })
}
