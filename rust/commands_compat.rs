//! Compatibility command writers over the engine's state, with one host
//! callback for the queue, spectrum lookup, smoothing and frames.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::commands::{
    self, Commands, FrameInfo, FrameRef, Host, Lookup, Settings, SpectPhoneme, State, N_SEQ_FRAMES,
};
use crate::generate::FmtParams;
use crate::phoneme_program::PhonemeData;
use std::{ffi::c_void, mem::size_of, ptr, slice};

// Matches RustCommandState, RustCommandBase/Settings, RustCommandLookup,
// RustCommandEffect, RustSpectPhoneme and frameref_t.
const _: () = assert!(
    size_of::<State>() == 48
        && size_of::<FrameRef>() == 16
        && size_of::<CommandLookup>() == 32
        && size_of::<Settings>() == 56
        && size_of::<CommandSettings>() == 72
        && size_of::<Effect>() == 96
        && size_of::<SpectPhoneme>() == 20
);

const PUSH: i32 = 0;
const TAIL: i32 = 1;
const WORD: i32 = 2;
const PATCH: i32 = 3;
const SMOOTH: i32 = 4;
const LOOKUP: i32 = 5;
const FRAME: i32 = 6;
const COPY_HIGH: i32 = 7;
const CLEAR_LENGTH_ADJUST: i32 = 8;

#[repr(C)]
#[derive(Default)]
pub struct CommandLookup {
    found: i32,
    count: i32,
    modulation: i32,
    n_pauses: i32,
    pauses: [i32; 4],
}

/// One host operation. Results come back in the return value, `value`,
/// `a`/`b` or `lookup`.
#[repr(C)]
pub struct Effect {
    op: i32,
    index: i32,
    slot: i32,
    a: i32,
    b: i32,
    c: i32,
    words: [isize; 4],
    count: usize,
    value: isize,
    fmt: *mut FmtParams,
    frames: *mut FrameRef,
    lookup: *mut CommandLookup,
}

/// Settings plus the phoneme sound data for one call.
#[repr(C)]
pub struct CommandSettings {
    settings: Settings,
    wave: *const u8,
    wave_length: usize,
}

type Callback = unsafe extern "C" fn(*mut c_void, *mut Effect) -> i32;

struct Callbacks {
    context: *mut c_void,
    callback: Callback,
}

impl Callbacks {
    fn effect(&mut self, op: i32) -> Effect {
        Effect {
            op,
            index: 0,
            slot: 0,
            a: 0,
            b: 0,
            c: 0,
            words: [0; 4],
            count: 0,
            value: 0,
            fmt: ptr::null_mut(),
            frames: ptr::null_mut(),
            lookup: ptr::null_mut(),
        }
    }
    fn call(&mut self, effect: &mut Effect) -> i32 {
        // SAFETY: serialized owner callback; pointers in the effect refer to
        // exclusive locals that outlive the call.
        unsafe { (self.callback)(self.context, effect) }
    }
}

impl Host for Callbacks {
    fn push(&mut self, words: [isize; 4], count: usize) -> i32 {
        let mut e = Effect {
            words,
            count,
            ..self.effect(PUSH)
        };
        self.call(&mut e)
    }
    fn tail(&mut self) -> i32 {
        let mut e = self.effect(TAIL);
        self.call(&mut e)
    }
    fn word(&mut self, index: i32, slot: usize) -> isize {
        let mut e = Effect {
            index,
            slot: slot as i32,
            ..self.effect(WORD)
        };
        self.call(&mut e);
        e.value
    }
    fn patch(&mut self, index: i32, slot: usize, value: isize) {
        let mut e = Effect {
            index,
            slot: slot as i32,
            value,
            ..self.effect(PATCH)
        };
        self.call(&mut e);
    }
    fn smooth(&mut self, start: i32, end: i32, centre: i32) -> i32 {
        let mut e = Effect {
            a: start,
            b: end,
            c: centre,
            ..self.effect(SMOOTH)
        };
        self.call(&mut e)
    }
    fn lookup(
        &mut self,
        which: i32,
        fmt: &mut FmtParams,
        frames: &mut [FrameRef; N_SEQ_FRAMES],
    ) -> Lookup {
        let mut result = CommandLookup::default();
        let mut e = Effect {
            a: which,
            fmt,
            frames: frames.as_mut_ptr(),
            lookup: &mut result,
            ..self.effect(LOOKUP)
        };
        self.call(&mut e);
        let mut lookup = Lookup {
            found: result.found != 0,
            count: usize::try_from(result.count).unwrap_or(usize::MAX),
            modulation: result.modulation,
            pauses: result.pauses,
            n_pauses: usize::try_from(result.n_pauses).unwrap_or(usize::MAX),
        };
        lookup.count = lookup.count.min(N_SEQ_FRAMES + 1);
        lookup
    }
    fn frame(&mut self, frame: usize) -> FrameInfo {
        let mut e = Effect {
            value: frame as isize,
            ..self.effect(FRAME)
        };
        self.call(&mut e);
        FrameInfo {
            length: e.a as u8,
            flags: e.b as i16,
        }
    }
    fn copy_high(&mut self, frame: usize, high: usize) -> usize {
        let mut e = Effect {
            value: frame as isize,
            words: [high as isize, 0, 0, 0],
            ..self.effect(COPY_HIGH)
        };
        self.call(&mut e);
        e.value as usize
    }
    fn clear_length_adjust(&mut self) {
        let mut e = self.effect(CLEAR_LENGTH_ADJUST);
        self.call(&mut e);
    }
}

/// Borrows the call's parts; `None` for invalid admission.
///
/// # Safety
/// Pointers must be valid as the exported functions document.
unsafe fn admit<'a>(
    state: *mut State,
    settings: *const CommandSettings,
    callback: Option<Callback>,
) -> Option<(&'a mut State, &'a Settings, &'a [u8], Callback)> {
    let callback = callback?;
    if state.is_null() || settings.is_null() {
        return None;
    }
    // SAFETY: exclusive state and an initialized settings copy, disjoint;
    // the callback never touches the state.
    let (state, settings) = unsafe { (&mut *state, &*settings) };
    if settings.wave.is_null() && settings.wave_length != 0
        || settings.wave_length > isize::MAX as usize
    {
        return None;
    }
    let wave: &[u8] = if settings.wave_length == 0 {
        &[]
    } else {
        // SAFETY: owner retains the resident phoneme sound data.
        unsafe { slice::from_raw_parts(settings.wave, settings.wave_length) }
    };
    Some((state, &settings.settings, wave, callback))
}

/// Runs `body` over the admitted call, or returns `invalid`.
///
/// # Safety
/// Pointers must be valid as the exported functions document.
unsafe fn run<R>(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    invalid: R,
    body: impl FnOnce(&mut Commands<'_, Callbacks>) -> R,
) -> R {
    // SAFETY: forwarded caller contract.
    let Some((state, settings, wave, callback)) = (unsafe { admit(state, settings, callback) })
    else {
        return invalid;
    };
    let mut host = Callbacks { context, callback };
    let mut commands = Commands {
        state,
        settings,
        wave,
        host: &mut host,
    };
    body(&mut commands)
}

#[no_mangle]
extern "C" fn espeak_rs_pause_length(
    settings: *const CommandSettings,
    pause: i32,
    control: i32,
) -> i32 {
    if settings.is_null() {
        return 0;
    }
    // SAFETY: nonnull initialized settings copy.
    commands::pause_length(unsafe { &(*settings).settings }, pause, control)
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_command_pause(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    length: i32,
    control: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, (), |c| {
            c.pause(length, control)
        })
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_command_pitch(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    envelope: *const u8,
    pitch1: i32,
    pitch2: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, (), |c| {
            c.pitch(envelope as usize, pitch1, pitch2)
        })
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_command_amplitude(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    amp: i32,
    envelope: *const u8,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, (), |c| {
            c.amplitude(amp, envelope as usize)
        })
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_command_end_pitch(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    voice_break: i32,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, (), |c| {
            c.end_pitch(voice_break != 0)
        })
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_command_end_amplitude(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, (), |c| {
            c.end_amplitude()
        })
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_command_start_syllable(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, (), |c| {
            c.start_syllable()
        })
    }
}

/// Returns the sample length, or -1 when C would read past the sound data or
/// never finish splitting the sample.
#[no_mangle]
unsafe extern "C" fn espeak_rs_command_sample(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    data: *const PhonemeData,
    length_mod: i32,
    amp: i32,
) -> i32 {
    if data.is_null() {
        return -1;
    }
    // SAFETY: nonnull initialized phoneme data, disjoint from the state.
    let data = unsafe { &*data };
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, -1, |c| {
            c.sample(data, length_mod, amp).unwrap_or(-1)
        })
    }
}

/// Returns the sequence length, or -1 as for samples or an oversized lookup.
#[no_mangle]
unsafe extern "C" fn espeak_rs_command_spect(
    state: *mut State,
    settings: *const CommandSettings,
    context: *mut c_void,
    callback: Option<Callback>,
    phoneme: *const SpectPhoneme,
    which: i32,
    fmt: *mut FmtParams,
    modulation: i32,
) -> i32 {
    if phoneme.is_null() || fmt.is_null() {
        return -1;
    }
    // SAFETY: nonnull initialized phoneme copy and exclusive format
    // parameters, disjoint from the state; the lookup callback receives the
    // same parameters back as its only access.
    let (phoneme, fmt) = unsafe { (&*phoneme, &mut *fmt) };
    // SAFETY: forwarded caller contract.
    unsafe {
        run(state, settings, context, callback, -1, |c| {
            c.spect(phoneme, which, fmt, modulation).unwrap_or(-1)
        })
    }
}
