//! Thin serialized C owner adapter for the native Klatt synthesizer.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    formant::Frame,
    klatt::{Host, Klatt},
    output::Output,
    voice::Voice,
    wave_memory::{WaveMemory, N_WCMDQ},
    wavegen::WgenData,
};
use std::{ptr, slice};

#[repr(C)]
pub struct Shared {
    memory: *mut WaveMemory,
    output: *mut Output,
    random: Option<unsafe extern "C" fn() -> i32>,
    reset: Option<unsafe extern "C" fn()>,
}
struct Memory<'a>(&'a Shared);
impl Host for Memory<'_> {
    fn room(&self) -> isize {
        // SAFETY: admitted live output, serialized by the engine.
        unsafe { (*self.0.output).room() }
    }
    fn emit(&mut self, sample: i32) {
        // SAFETY: shared memory and output are disjoint and owner-serialized;
        // the kernel checked room before every write.
        unsafe {
            let memory = &mut *self.0.memory;
            let value = sample.wrapping_add(memory.echo() >> 8).clamp(-32768, 32767);
            (*self.0.output).write(value);
            memory.echo_put(value);
        }
    }
    fn random(&mut self) -> i32 {
        // SAFETY: the owner supplies its serialized random stream.
        self.0.random.map_or(0, |random| unsafe { random() })
    }
    fn byte(&self, address: usize, offset: i32) -> u8 {
        if address == 0 {
            return 0;
        }
        // SAFETY: caller's admitted resident envelope or sample span.
        unsafe { *(address as *const u8).offset(offset as isize) }
    }
    fn next_spectrum(&self) -> Option<Frame> {
        // SAFETY: admitted live, serialized queue; borrow ends before callbacks.
        let memory = unsafe { &*self.0.memory };
        if !(0..N_WCMDQ).contains(&memory.head) || !(0..N_WCMDQ).contains(&memory.tail) {
            return None;
        }
        let mut index = (memory.head + 1) % N_WCMDQ;
        for _ in 0..N_WCMDQ {
            if index == memory.tail {
                break;
            }
            let q = memory.entry(index);
            if q[0] == 1 {
                // SAFETY: the command retains its spectrum bytes until consumed.
                return unsafe { read_frame(q[2] as *const Frame) };
            }
            if q[0] == 5 || q[0] == 6 {
                break;
            }
            index = (index + 1) % N_WCMDQ;
        }
        None
    }
    fn reset_speechplayer(&mut self) {
        if let Some(reset) = self.0.reset {
            // SAFETY: owner callback affects only the separate speechPlayer.
            unsafe { reset() }
        }
    }
}

/// Reads exactly 44 ordinary or 64 extended initialized frame bytes, never
/// the optional tail of a short ordinary record at the end of phondata.
unsafe fn read_frame(frame: *const Frame) -> Option<Frame> {
    if frame.is_null() {
        return None;
    }
    // SAFETY: caller retains at least the flags of an admitted frame.
    let flags = unsafe { ptr::read_unaligned(frame.cast::<i16>()) };
    let length = if flags & 1 == 0 { 44 } else { 64 };
    // SAFETY: admitted resident frame with its flagged initialized extent.
    Frame::decode(unsafe { slice::from_raw_parts(frame.cast::<u8>(), length) }).ok()
}

#[no_mangle]
extern "C" fn espeak_rs_klatt_new() -> *mut Klatt {
    Box::into_raw(Box::new(Klatt::new()))
}

/// # Safety
/// `state` is null or an exclusively owned pointer from `new`, freed once.
#[no_mangle]
unsafe extern "C" fn espeak_rs_klatt_free(state: *mut Klatt) {
    if !state.is_null() {
        // SAFETY: caller transfers ownership once.
        drop(unsafe { Box::from_raw(state) });
    }
}

/// # Safety
/// `state` is live, exclusive and serialized for this operation.
#[no_mangle]
unsafe extern "C" fn espeak_rs_klatt_init(state: *mut Klatt) {
    if !state.is_null() {
        // SAFETY: caller contract.
        unsafe { (*state).initialize() }
    }
}

/// # Safety
/// As for `init`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_klatt_reset(state: *mut Klatt, control: i32) {
    if !state.is_null() {
        // SAFETY: caller contract.
        unsafe { (*state).reset(control) }
    }
}

/// # Safety
/// All pointers refer to disjoint initialized owner storage, exclusive where
/// mutated. Frames, pitch envelope, queued lookahead and mixed samples stay
/// resident throughout; callbacks cannot reenter the Klatt state. The engine
/// serializes this state exactly as it did the legacy C statics.
#[no_mangle]
unsafe extern "C" fn espeak_rs_klatt_fill(
    state: *mut Klatt,
    shared: *const Shared,
    length: i32,
    resume: i32,
    first: *const Frame,
    last: *const Frame,
    data: *mut WgenData,
    voice: *const Voice,
) -> i32 {
    if state.is_null() || shared.is_null() || data.is_null() || voice.is_null() {
        return 0;
    }
    // SAFETY: caller contract.
    let shared = unsafe { &*shared };
    if shared.memory.is_null() || shared.output.is_null() {
        return 0;
    }
    // SAFETY: caller retains both flagged frame extents.
    let (Some(first), Some(last)) = (unsafe { read_frame(first) }, unsafe { read_frame(last) })
    else {
        return 0;
    };
    // SAFETY: caller contract; snapshots keep foreign borrows out of callbacks.
    let (mut snapshot, voice) = unsafe { (ptr::read(data), ptr::read(voice)) };
    // SAFETY: exclusive, serialized kernel state; callbacks touch disjoint owners.
    let full = unsafe {
        (*state).fill(
            &mut Memory(shared),
            length,
            resume != 0,
            &first,
            &last,
            &mut snapshot,
            &voice,
        )
    };
    // SAFETY: return the updated pitch/mix cursors to the owner's live WGEN_DATA.
    unsafe { ptr::write(data, snapshot) };
    i32::from(full)
}
