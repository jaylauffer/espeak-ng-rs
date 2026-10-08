//! The process's command queue and echo ring (`espeak_rs_wave_memory`) and
//! the C entry points that count, write and read them. C code not yet ported
//! (Klatt's and speechPlayer's look-ahead, smoothing) still reads the queue in
//! place.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::wave_memory::WaveMemory;
use std::ffi::c_char;

/// The process's queue and echo ring.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut espeak_rs_wave_memory: WaveMemory = WaveMemory::new();

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
