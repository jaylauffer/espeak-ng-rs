//! The process's command queue and echo ring (`espeak_rs_wave_memory`),
//! output buffer (`espeak_rs_output`) and frame pool (`espeak_rs_frame_pool`),
//! and the C entry points that use them. C code not yet ported (Klatt's and
//! speechPlayer's look-ahead and output, MBROLA and sonic output, events)
//! still reads and advances them in place.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::formant::Frame;
use crate::output::{FramePool, Output};
use crate::wave_memory::WaveMemory;
use std::ffi::{c_char, c_void};
use std::mem::size_of;
use std::ptr;

// Matches RustOutput and RustFramePool.
const _: () = assert!(size_of::<Output>() == 40 && size_of::<FramePool>() == 170 * 64 + 4);

/// The process's queue and echo ring.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut espeak_rs_wave_memory: WaveMemory = WaveMemory::new();

/// The process's output buffer.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut espeak_rs_output: Output = Output::new();

/// The process's frame pool.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut espeak_rs_frame_pool: FramePool = FramePool::new();

/// Runs `body` on a queue and ring, or returns `invalid` for null.
///
/// # Safety
/// `memory` is null or a live `RustWaveMemory`; access is serialized.
unsafe fn on_memory<R>(
    memory: *mut WaveMemory,
    invalid: R,
    body: impl FnOnce(&mut WaveMemory) -> R,
) -> R {
    if memory.is_null() {
        return invalid;
    }
    // SAFETY: caller contract.
    body(unsafe { &mut *memory })
}

/// `WcmdqFree`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wcmdq_free(memory: *mut WaveMemory) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, 0, |m| m.free()) }
}

/// `WcmdqUsed`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wcmdq_used(memory: *mut WaveMemory) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, 0, |m| m.used()) }
}

/// `WcmdqInc`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wcmdq_inc(memory: *mut WaveMemory) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), WaveMemory::inc_tail) }
}

/// The queue part of `WcmdqStop`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wcmdq_stop(memory: *mut WaveMemory) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), WaveMemory::stop) }
}

/// The echo ring's tail sample times its amplitude, advancing the tail.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_echo(memory: *mut WaveMemory) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, 0, WaveMemory::echo) }
}

/// Stores a sample at the echo ring's head and advances it.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_echo_put(memory: *mut WaveMemory, sample: i32) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), |m| m.echo_put(sample)) }
}

/// `DoMarker`; returns whether it was queued (more than five free entries).
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_queue_marker(
    memory: *mut WaveMemory,
    kind: i32,
    char_posn: i32,
    length: i32,
    value: i32,
) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe {
        on_memory(memory, 0, |m| {
            i32::from(m.marker(kind, char_posn, length, value))
        })
    }
}

/// `DoPhonemeMarker`: 8 bytes of `name`.
///
/// # Safety
/// As for `on_memory`; `name` has 8 readable bytes.
#[no_mangle]
unsafe extern "C" fn espeak_rs_queue_phoneme_marker(
    memory: *mut WaveMemory,
    kind: i32,
    char_posn: i32,
    length: i32,
    name: *const c_char,
) -> i32 {
    if name.is_null() {
        return 0;
    }
    // SAFETY: caller contract.
    let name = unsafe { name.cast::<[u8; 8]>().read_unaligned() };
    // SAFETY: forwarded caller contract.
    unsafe {
        on_memory(memory, 0, |m| {
            i32::from(m.phoneme_marker(kind, char_posn, length, name))
        })
    }
}

/// `DoPhonemeAlignment`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_queue_phoneme_alignment(
    memory: *mut WaveMemory,
    name: *mut c_char,
    kind: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), |m| m.phoneme_alignment(name as usize, kind)) }
}

/// `DoSonicSpeed`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_queue_sonic_speed(memory: *mut WaveMemory, value: i32) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), |m| m.sonic_speed(value)) }
}

/// `DoVoiceChange`'s queue entry for an allocated voice copy.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_queue_voice(memory: *mut WaveMemory, voice: *mut u8) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), |m| m.voice_change(voice as usize)) }
}

/// MBROLA output of `length`.
///
/// # Safety
/// As for `on_memory`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_queue_mbrola(memory: *mut WaveMemory, length: i32) {
    // SAFETY: forwarded caller contract.
    unsafe { on_memory(memory, (), |m| m.mbrola_data(length)) }
}

/// Replaces the output buffer with `size` bytes; 0, or -1 when the
/// allocation fails (the old buffer is kept).
///
/// # Safety
/// `output` is null or a live `RustOutput` whose buffer this module
/// allocated; access is serialized.
#[no_mangle]
unsafe extern "C" fn espeak_rs_output_reserve(output: *mut Output, size: usize) -> i32 {
    if output.is_null() {
        return -1;
    }
    // SAFETY: caller contract.
    if unsafe { (*output).reserve(size) } {
        0
    } else {
        -1
    }
}

/// Starts a fill of the whole buffer.
///
/// # Safety
/// As for `espeak_rs_output_reserve`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_output_begin(output: *mut Output) {
    if !output.is_null() {
        // SAFETY: caller contract.
        unsafe { (*output).begin() }
    }
}

/// Frees the buffer.
///
/// # Safety
/// As for `espeak_rs_output_reserve`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_output_release(output: *mut Output) {
    if !output.is_null() {
        // SAFETY: caller contract.
        unsafe { (*output).release() }
    }
}

/// The frame storage callback over a pool (`opaque`): kind 0 takes the next
/// frame, kind 1 returns `frame` when it is one of the pool's frames (so it
/// may be modified), else null.
///
/// # Safety
/// `opaque` is null or a live `RustFramePool`; access is serialized. A
/// returned frame stays valid until the pool comes round to it again.
#[no_mangle]
unsafe extern "C" fn espeak_rs_frame_pool_storage(
    opaque: *mut c_void,
    kind: u32,
    frame: *mut c_void,
) -> *mut c_void {
    let pool = opaque.cast::<FramePool>();
    if pool.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller contract; the borrow ends before returning.
    let pool = unsafe { &mut *pool };
    match kind {
        0 => (pool.allocate() as *mut Frame).cast(),
        1 if pool.owns(frame.cast::<Frame>()) => frame,
        _ => ptr::null_mut(),
    }
}
