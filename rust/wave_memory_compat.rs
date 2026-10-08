//! The process's command queue and echo ring (`espeak_rs_wave_memory`),
//! output buffer (`espeak_rs_output`) and frame pool (`espeak_rs_frame_pool`),
//! and the C entry points that use them. C code not yet ported (Klatt's and
//! speechPlayer's look-ahead and output, MBROLA and sonic output, events)
//! still reads and advances them in place.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::events::{Event, EventList, EventSettings};
use crate::formant::Frame;
use crate::output::{FramePool, Output};
use crate::wave_memory::WaveMemory;
use crate::wavegen::{EMBEDDED_DEFAULTS, N_EMBEDDED_VALUES};
use std::ffi::{c_char, c_long, c_void};
use std::mem::size_of;
use std::ptr;

// Matches RustOutput, RustFramePool, espeak_EVENT, RustEventList and
// RustEventSettings.
const _: () = assert!(
    size_of::<Output>() == 40
        && size_of::<FramePool>() == 170 * 64 + 4
        && size_of::<Event>() == 40
        && size_of::<EventList>() == 16
        && size_of::<EventSettings>() == 40
);

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

/// The process's event list.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut espeak_rs_events: EventList = EventList::new();

/// The embedded command values (pitch, speed, volume, ...), under C's name:
/// the C writers and readers that remain address them in place.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut embedded_value: [i32; N_EMBEDDED_VALUES] = [0; N_EMBEDDED_VALUES];

/// Their defaults, under C's name.
#[no_mangle]
#[allow(non_upper_case_globals)]
static embedded_default: [i32; N_EMBEDDED_VALUES] = EMBEDDED_DEFAULTS;

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

/// Runs `body` on an event list, or returns `invalid` for null.
///
/// # Safety
/// `list` is null or a live `RustEventList` whose buffer this module
/// allocated; access is serialized.
unsafe fn on_events<R>(
    list: *mut EventList,
    invalid: R,
    body: impl FnOnce(&mut EventList) -> R,
) -> R {
    if list.is_null() {
        return invalid;
    }
    // SAFETY: caller contract.
    body(unsafe { &mut *list })
}

/// Resizes the event list; 0, or -1 when the allocation fails (the old list
/// is kept).
///
/// # Safety
/// As for `on_events`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_events_reserve(list: *mut EventList, capacity: i32) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe { on_events(list, -1, |l| if l.reserve(capacity) { 0 } else { -1 }) }
}

/// Frees the event list.
///
/// # Safety
/// As for `on_events`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_events_release(list: *mut EventList) {
    // SAFETY: forwarded caller contract.
    unsafe { on_events(list, (), EventList::release) }
}

/// `MarkerEvent`, `offset` output bytes into the current buffer; 1 when
/// recorded.
///
/// # Safety
/// As for `on_events`; `settings` is readable.
#[no_mangle]
unsafe extern "C" fn espeak_rs_event_marker(
    list: *mut EventList,
    settings: *const EventSettings,
    kind: i32,
    char_position: u32,
    value: i32,
    value2: i32,
    offset: isize,
) -> i32 {
    if settings.is_null() {
        return 0;
    }
    // SAFETY: caller contract.
    let settings = unsafe { *settings };
    // SAFETY: forwarded caller contract.
    unsafe {
        on_events(list, 0, |l| {
            i32::from(l.marker(&settings, kind, char_position, value, value2, offset))
        })
    }
}

/// Terminates the events at `index`.
///
/// # Safety
/// As for `on_events`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_events_terminate(
    list: *mut EventList,
    index: i32,
    unique_identifier: u32,
    user_data: *mut c_void,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        on_events(list, (), |l| {
            l.terminate(index, unique_identifier, user_data as usize)
        })
    }
}

/// A message's end: the message and list terminators.
///
/// # Safety
/// As for `on_events`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_events_terminated_message(
    list: *mut EventList,
    unique_identifier: u32,
    user_data: *mut c_void,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        on_events(list, (), |l| {
            l.terminated_message(unique_identifier, user_data as usize)
        })
    }
}

/// `RescaleEventSamples`.
///
/// # Safety
/// As for `on_events`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_events_rescale(
    list: *mut EventList,
    length_pre: i32,
    length_post: i32,
    count_samples: c_long,
    mbrola_delay: i32,
    samplerate: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        on_events(list, (), |l| {
            l.rescale(
                length_pre,
                length_post,
                count_samples,
                mbrola_delay,
                samplerate,
            )
        })
    }
}
