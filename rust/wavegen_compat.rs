//! Compatibility wave generator over the engine's shared memory: the queue,
//! echo ring, output buffer and embedded values are read and written in
//! place, as C did; everything else goes through one host callback.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::formant::Frame;
use crate::voice::Voice;
use crate::wavegen::{
    Hook, Host, Options, Peak, Wavegen, WgenData, MAX_HARMONIC, N_ECHO_BUF, N_EMBEDDED_VALUES,
    N_PEAKS, N_WCMDQ,
};
use std::{ffi::c_void, mem::size_of, ptr, slice};

// Matches RustWavegenShared, RustWavegenEffect, RustWavegenOptions,
// wavegen_peaks_t, WGEN_DATA and voice_t.
const _: () = assert!(
    size_of::<Shared>() == 88
        && size_of::<Effect>() == 56
        && size_of::<FfiOptions>() == 20
        && size_of::<Peak>() == 80
        && size_of::<WgenData>() == 80
        && size_of::<Voice>() == 1344
);

const ADVANCE: i32 = 0;
const HOOK: i32 = 1;
const MARKER: i32 = 2;
const ALIGNMENT: i32 = 3;
const SAMPLERATE: i32 = 4;
const SONIC: i32 = 5;
const RANDOM: i32 = 6;
const FREE_VOICE: i32 = 7;
const KLATT_RESET: i32 = 8;
const KLATT: i32 = 9;
const MBROLA: i32 = 10;

/// The frame bytes the generator reads: up to and including `fright`, so a
/// short frame at the end of its data is never read past.
const FRAME_PREFIX: usize = 35;

/// The engine's shared wave memory.
#[repr(C)]
pub struct Shared {
    samplerate: *mut i32,
    embedded: *mut i32,
    queue: *mut [isize; 4],
    head: *mut i32,
    tail: *mut i32,
    out_ptr: *mut *mut u8,
    out_end: *mut *mut u8,
    echo_buf: *mut i16,
    echo_head: *mut i32,
    echo_tail: *mut i32,
    echo_amp: *mut i32,
}

/// One host operation; results come back in the return value.
#[repr(C)]
pub struct Effect {
    op: i32,
    index: i32,
    a: i32,
    b: i32,
    c: i32,
    value: usize,
    value2: usize,
    data: *mut WgenData,
    voice: *mut Voice,
}

/// Compiled-in synthesizers, the current voice's roughness and the output
/// hooks present (bit 0 voiced, 1 silence, 2 unvoiced).
#[repr(C)]
pub struct FfiOptions {
    klatt: i32,
    mbrola: i32,
    sonic: i32,
    roughness: i32,
    hooks: i32,
}

type Callback = unsafe extern "C" fn(*mut c_void, *mut Effect) -> i32;

struct Memory<'a> {
    shared: &'a Shared,
    context: *mut c_void,
    callback: Option<Callback>,
    hooks: i32,
}

impl Memory<'_> {
    fn call(&mut self, op: i32, index: i32, a: i32, b: i32) -> i32 {
        self.call_with(Effect {
            op,
            index,
            a,
            b,
            c: 0,
            value: 0,
            value2: 0,
            data: ptr::null_mut(),
            voice: ptr::null_mut(),
        })
    }

    fn call_with(&mut self, mut effect: Effect) -> i32 {
        let Some(callback) = self.callback else {
            return 0;
        };
        // SAFETY: serialized owner callback; the effect is a local and its
        // pointers are the generator's own data, unused by Rust meanwhile.
        unsafe { callback(self.context, &mut effect) }
    }
}

impl Host for Memory<'_> {
    fn samplerate(&mut self) -> i32 {
        // SAFETY: the owner's live global, as for every shared pointer.
        unsafe { *self.shared.samplerate }
    }
    fn set_samplerate(&mut self, rate: i32) {
        // SAFETY: as above.
        unsafe { *self.shared.samplerate = rate }
    }
    fn embedded(&mut self, index: usize) -> i32 {
        if index >= N_EMBEDDED_VALUES {
            return 0;
        }
        // SAFETY: in bounds of the owner's embedded values.
        unsafe { *self.shared.embedded.add(index) }
    }
    fn set_embedded(&mut self, index: usize, value: i32) {
        if index < N_EMBEDDED_VALUES {
            // SAFETY: as above.
            unsafe { *self.shared.embedded.add(index) = value }
        }
    }

    fn head(&mut self) -> i32 {
        // SAFETY: as above.
        unsafe { *self.shared.head }
    }
    fn tail(&mut self) -> i32 {
        // SAFETY: as above.
        unsafe { *self.shared.tail }
    }
    fn command(&mut self, index: i32) -> [isize; 4] {
        if !(0..N_WCMDQ).contains(&index) {
            return [0; 4];
        }
        // SAFETY: in bounds of the owner's queue.
        unsafe { *self.shared.queue.add(index as usize) }
    }
    fn advance_head(&mut self) {
        self.call(ADVANCE, 0, 0, 0);
    }

    fn room(&mut self) -> isize {
        // SAFETY: the output pointers refer to one live buffer.
        unsafe { (*self.shared.out_end).offset_from(*self.shared.out_ptr) }
    }
    fn write(&mut self, sample: i32) {
        // SAFETY: the caller checked room before writing, as C did.
        unsafe {
            let out = *self.shared.out_ptr;
            *out = sample as u8;
            *out.add(1) = (sample >> 8) as u8;
            *self.shared.out_ptr = out.add(2);
        }
    }

    fn echo_take(&mut self) -> i32 {
        // SAFETY: the tail stays inside the ring.
        unsafe {
            let tail = *self.shared.echo_tail;
            let value = if (0..N_ECHO_BUF).contains(&tail) {
                i32::from(*self.shared.echo_buf.add(tail as usize))
            } else {
                0
            };
            *self.shared.echo_tail = if tail.wrapping_add(1) >= N_ECHO_BUF {
                0
            } else {
                tail + 1
            };
            value
        }
    }
    fn echo_put(&mut self, sample: i32) {
        // SAFETY: as above; a head past the ring, from a long echo delay, is
        // not written (C wrote past the ring once).
        unsafe {
            let head = *self.shared.echo_head;
            if (0..N_ECHO_BUF).contains(&head) {
                *self.shared.echo_buf.add(head as usize) = sample as i16;
            }
            *self.shared.echo_head = if head.wrapping_add(1) >= N_ECHO_BUF {
                0
            } else {
                head + 1
            };
        }
    }
    fn echo_amp(&mut self) -> i32 {
        // SAFETY: as above.
        unsafe { *self.shared.echo_amp }
    }
    fn echo_reset(&mut self, head: i32, amp: i32) {
        // SAFETY: the owner's ring of N_ECHO_BUF samples.
        unsafe {
            ptr::write_bytes(self.shared.echo_buf, 0, N_ECHO_BUF as usize);
            *self.shared.echo_tail = 0;
            *self.shared.echo_head = head;
            *self.shared.echo_amp = amp;
        }
    }

    fn byte(&mut self, address: usize, offset: i32) -> u8 {
        if address == 0 {
            return 0;
        }
        // SAFETY: queued envelopes and sample data are resident; offsets are
        // those C read.
        unsafe { *(address as *const u8).offset(offset as isize) }
    }
    fn frame(&mut self, address: usize) -> Frame {
        let mut frame = Frame::default();
        if address != 0 {
            // SAFETY: a queued resident frame; Frame is plain integers.
            unsafe {
                ptr::copy_nonoverlapping(
                    address as *const u8,
                    ptr::addr_of_mut!(frame).cast::<u8>(),
                    FRAME_PREFIX,
                )
            };
        }
        frame
    }
    fn voice(&mut self, address: usize) -> Option<Voice> {
        if address == 0 {
            return None;
        }
        // SAFETY: a queued voice_t copy, which Voice mirrors.
        Some(unsafe { ptr::read_unaligned(address as *const Voice) })
    }
    fn free_voice(&mut self, address: usize) {
        self.call_with(Effect {
            value: address,
            ..empty(FREE_VOICE)
        });
    }

    fn hook(&mut self, hook: Hook, sample: i32) {
        let bit = match hook {
            Hook::Voiced => 1,
            Hook::Silence => 2,
            Hook::Unvoiced => 4,
        };
        if self.hooks & bit != 0 {
            self.call(HOOK, 0, bit, sample);
        }
    }
    fn marker(&mut self, index: i32) {
        self.call(MARKER, index, 0, 0);
    }
    fn alignment(&mut self, index: i32) {
        self.call(ALIGNMENT, index, 0, 0);
    }
    fn samplerate_event(&mut self, rate: i32) {
        self.call(SAMPLERATE, 0, rate, 0);
    }
    fn sonic_speed(&mut self, index: i32) {
        self.call(SONIC, index, 0, 0);
    }
    fn random(&mut self, min: i32, max: i32) -> i32 {
        self.call(RANDOM, 0, min, max)
    }

    fn klatt_reset(&mut self) {
        self.call(KLATT_RESET, 0, 0, 0);
    }
    fn klatt(
        &mut self,
        length: i32,
        resume: bool,
        fr1: usize,
        fr2: usize,
        data: *mut WgenData,
        voice: *mut Voice,
    ) -> i32 {
        self.call_with(Effect {
            a: length,
            b: i32::from(resume),
            value: fr1,
            value2: fr2,
            data,
            voice,
            ..empty(KLATT)
        })
    }
    fn mbrola(&mut self, length: i32, resume: bool, amp: i32) -> i32 {
        self.call_with(Effect {
            a: length,
            b: i32::from(resume),
            c: amp,
            ..empty(MBROLA)
        })
    }
}

fn empty(op: i32) -> Effect {
    Effect {
        op,
        index: 0,
        a: 0,
        b: 0,
        c: 0,
        value: 0,
        value2: 0,
        data: ptr::null_mut(),
        voice: ptr::null_mut(),
    }
}

/// Runs `body` over the generator and shared memory, or returns `invalid`.
///
/// # Safety
/// `wavegen` comes from `espeak_rs_wavegen_new`; `shared` points to the
/// owner's live globals; access is serialized.
unsafe fn run<R>(
    wavegen: *mut Wavegen,
    shared: *const Shared,
    context: *mut c_void,
    callback: Option<Callback>,
    hooks: i32,
    invalid: R,
    body: impl FnOnce(&mut Wavegen, &mut Memory<'_>) -> R,
) -> R {
    if wavegen.is_null() || shared.is_null() {
        return invalid;
    }
    // SAFETY: caller contract; the generator is disjoint from the shared
    // memory and the callback never touches the generator.
    let (wavegen, shared) = unsafe { (&mut *wavegen, &*shared) };
    let mut memory = Memory {
        shared,
        context,
        callback,
        hooks,
    };
    body(wavegen, &mut memory)
}

#[no_mangle]
extern "C" fn espeak_rs_wavegen_new() -> *mut Wavegen {
    Box::into_raw(Box::default())
}

/// # Safety
/// `wavegen` is null or from `espeak_rs_wavegen_new`, freed once.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_free(wavegen: *mut Wavegen) {
    if !wavegen.is_null() {
        // SAFETY: caller contract.
        drop(unsafe { Box::from_raw(wavegen) });
    }
}

/// # Safety
/// As for `run`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_init(
    wavegen: *mut Wavegen,
    shared: *const Shared,
    context: *mut c_void,
    callback: Option<Callback>,
    rate: i32,
    wavemult_fact: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(wavegen, shared, context, callback, 0, (), |w, m| {
            w.init(m, rate, wavemult_fact)
        })
    }
}

/// `GetAmplitude`.
///
/// # Safety
/// As for `run`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_amplitude(
    wavegen: *mut Wavegen,
    shared: *const Shared,
) -> i32 {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(wavegen, shared, ptr::null_mut(), None, 0, 0, |w, m| {
            w.amplitude(m)
        })
    }
}

/// `PeaksToHarmspect`; `htab` holds `MAX_HARMONIC` entries.
///
/// # Safety
/// `wavegen` as for `run`; `peaks` and `htab` are valid as described.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_harmonics(
    wavegen: *mut Wavegen,
    samplerate: i32,
    peaks: *const [Peak; N_PEAKS],
    pitch: i32,
    htab: *mut i32,
    control: i32,
) -> i32 {
    if wavegen.is_null() || peaks.is_null() || htab.is_null() {
        return 1;
    }
    // SAFETY: caller contract; the arrays are disjoint from the generator.
    let (wavegen, peaks, htab) = unsafe {
        (
            &mut *wavegen,
            &*peaks,
            slice::from_raw_parts_mut(htab, MAX_HARMONIC),
        )
    };
    wavegen.peaks_to_harmspect(samplerate, peaks, pitch, htab, control)
}

/// `InitBreath`.
///
/// # Safety
/// `wavegen` as for `run`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_init_breath(wavegen: *mut Wavegen, samplerate: i32) {
    if !wavegen.is_null() {
        // SAFETY: caller contract.
        unsafe { (*wavegen).init_breath(samplerate) }
    }
}

/// # Safety
/// As for `run`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_set_embedded(
    wavegen: *mut Wavegen,
    shared: *const Shared,
    context: *mut c_void,
    callback: Option<Callback>,
    control: i32,
    value: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(wavegen, shared, context, callback, 0, (), |w, m| {
            w.set_embedded(m, control, value)
        })
    }
}

/// `WavegenSetVoice`: copies the voice.
///
/// # Safety
/// As for `run`; `voice` is a readable voice_t.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_set_voice(
    wavegen: *mut Wavegen,
    shared: *const Shared,
    context: *mut c_void,
    callback: Option<Callback>,
    voice: *const Voice,
) {
    if voice.is_null() {
        return;
    }
    // SAFETY: caller contract.
    let voice = unsafe { ptr::read_unaligned(voice) };
    // SAFETY: forwarded caller contract.
    unsafe {
        run(wavegen, shared, context, callback, 0, (), |w, m| {
            w.set_voice(m, &voice)
        })
    }
}

/// # Safety
/// `wavegen` as for `run`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_set_const_f0(wavegen: *mut Wavegen, f0: i32) {
    if !wavegen.is_null() {
        // SAFETY: caller contract.
        unsafe { (*wavegen).set_const_f0(f0) }
    }
}

/// `WavegenFill2`: 0 when the output buffer filled, 1 when the queue emptied.
///
/// # Safety
/// As for `run`; `options` is readable and `fall` is the resident default
/// pitch envelope.
#[no_mangle]
unsafe extern "C" fn espeak_rs_wavegen_fill(
    wavegen: *mut Wavegen,
    shared: *const Shared,
    context: *mut c_void,
    callback: Option<Callback>,
    options: *const FfiOptions,
    fall: *const u8,
) -> i32 {
    if options.is_null() {
        return 1;
    }
    // SAFETY: caller contract.
    let options = unsafe { &*options };
    let parsed = Options {
        klatt: options.klatt != 0,
        mbrola: options.mbrola != 0,
        sonic: options.sonic != 0,
        roughness: options.roughness,
    };
    // SAFETY: forwarded caller contract.
    unsafe {
        run(
            wavegen,
            shared,
            context,
            callback,
            options.hooks,
            1,
            |w, m| w.fill(m, &parsed, fall as usize),
        )
    }
}
