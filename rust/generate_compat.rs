//! Compatibility `Generate` with one ordered effect callback for the host.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::generate::{self, Entry, Envelope, Error, FmtParams, Host, Settings, State};
use crate::phoneme_program::PhonemeData;
use std::{ffi::c_void, mem::size_of, ptr, slice};

// Matches RustGenerateEntry, FMT_PARAMS and RustGenerateEffect.
const _: () = assert!(size_of::<Entry>() == 36 && size_of::<FmtParams>() == 48);

const FREE: i32 = 0;
const RESET: i32 = 1;
const PITCH_STARTED: i32 = 2;
const ALIGNMENT: i32 = 3;
const EMBEDDED: i32 = 4;
const BREAK_FRAME: i32 = 5;
const MARKER: i32 = 6;
const PHONEME_MARKER: i32 = 7;
const END_AMPLITUDE: i32 = 8;
const END_PITCH: i32 = 9;
const PAUSE: i32 = 10;
const AMPLITUDE: i32 = 11;
const PITCH: i32 = 12;
const START_SYLLABLE: i32 = 13;
const INTERPRET: i32 = 14;
const TONE: i32 = 15;
const SPECT: i32 = 16;
const SAMPLE: i32 = 17;
const SET_SYNTHFLAGS: i32 = 18;
const SET_STD_LENGTH: i32 = 19;

/// One host effect. `envelope` is 0 none, 1 table, 2 data address.
#[repr(C)]
pub struct Effect {
    op: i32,
    index: i32,
    a: i32,
    b: i32,
    c: i32,
    envelope: i32,
    envelope_value: i32,
    fmt: *mut FmtParams,
    data: *mut PhonemeData,
    embedded_ix: *mut i32,
}

type Callback = unsafe extern "C" fn(*mut c_void, *mut Effect) -> i32;

struct Callbacks {
    context: *mut c_void,
    callback: Callback,
}

impl Callbacks {
    fn call(&mut self, op: i32, index: usize, args: [i32; 3]) -> i32 {
        self.effect(Effect {
            op,
            index: index as i32,
            a: args[0],
            b: args[1],
            c: args[2],
            envelope: 0,
            envelope_value: 0,
            fmt: ptr::null_mut(),
            data: ptr::null_mut(),
            embedded_ix: ptr::null_mut(),
        })
    }
    fn effect(&mut self, mut effect: Effect) -> i32 {
        // SAFETY: serialized owner callback; pointers in the effect refer to
        // exclusive locals that outlive the call.
        unsafe { (self.callback)(self.context, &mut effect) }
    }
    fn with_envelope(&mut self, op: i32, envelope: Option<Envelope>, args: [i32; 3]) {
        let (kind, value) = match envelope {
            None => (0, 0),
            Some(Envelope::Table(n)) => (1, n),
            Some(Envelope::Data(address)) => (2, address),
        };
        self.effect(Effect {
            op,
            index: 0,
            a: args[0],
            b: args[1],
            c: args[2],
            envelope: kind,
            envelope_value: value,
            fmt: ptr::null_mut(),
            data: ptr::null_mut(),
            embedded_ix: ptr::null_mut(),
        });
    }
    fn with_data(&mut self, op: i32, index: usize, args: [i32; 3], data: &mut PhonemeData) -> i32 {
        self.effect(Effect {
            op,
            index: index as i32,
            a: args[0],
            b: args[1],
            c: args[2],
            envelope: 0,
            envelope_value: 0,
            fmt: ptr::null_mut(),
            data,
            embedded_ix: ptr::null_mut(),
        })
    }
}

impl Host for Callbacks {
    fn free(&mut self) -> i32 {
        self.call(FREE, 0, [0; 3])
    }
    fn reset(&mut self) {
        self.call(RESET, 0, [0; 3]);
    }
    fn pitch_started(&mut self) -> bool {
        self.call(PITCH_STARTED, 0, [0; 3]) != 0
    }
    fn alignment(&mut self, index: usize) {
        self.call(ALIGNMENT, index, [0; 3]);
    }
    fn embedded(&mut self, embedded_ix: &mut i32, source: i32) {
        self.effect(Effect {
            op: EMBEDDED,
            index: 0,
            a: source,
            b: 0,
            c: 0,
            envelope: 0,
            envelope_value: 0,
            fmt: ptr::null_mut(),
            data: ptr::null_mut(),
            embedded_ix,
        });
    }
    fn break_frame(&mut self) {
        self.call(BREAK_FRAME, 0, [0; 3]);
    }
    fn marker(&mut self, kind: i32, position: i32, length: i32, value: i32) {
        self.call(MARKER, kind as usize, [position, length, value]);
    }
    fn phoneme_marker(&mut self, index: usize, ipa: bool, position: i32) {
        self.call(PHONEME_MARKER, index, [i32::from(ipa), position, 0]);
    }
    fn end_amplitude(&mut self) {
        self.call(END_AMPLITUDE, 0, [0; 3]);
    }
    fn end_pitch(&mut self, voice_break: bool) {
        self.call(END_PITCH, 0, [i32::from(voice_break), 0, 0]);
    }
    fn pause(&mut self, length: i32, control: i32) {
        self.call(PAUSE, 0, [length, control, 0]);
    }
    fn amplitude(&mut self, amp: i32, envelope: Option<Envelope>) {
        self.with_envelope(AMPLITUDE, envelope, [amp, 0, 0]);
    }
    fn pitch(&mut self, envelope: Envelope, pitch1: i32, pitch2: i32) {
        self.with_envelope(PITCH, Some(envelope), [pitch1, pitch2, 0]);
    }
    fn start_syllable(&mut self) {
        self.call(START_SYLLABLE, 0, [0; 3]);
    }
    fn interpret(&mut self, index: usize, control: i32, word: bool) -> PhonemeData {
        let mut data = PhonemeData::default();
        self.with_data(INTERPRET, index, [control, i32::from(word), 0], &mut data);
        data
    }
    fn tone(&mut self, index: usize) -> Result<PhonemeData, Error> {
        let mut data = PhonemeData::default();
        match self.with_data(TONE, index, [0; 3], &mut data) {
            0 => Ok(data),
            _ => Err(Error::Host),
        }
    }
    fn spect(&mut self, index: usize, which: i32, fmt: &mut FmtParams, modulation: i32) {
        self.effect(Effect {
            op: SPECT,
            index: index as i32,
            a: which,
            b: modulation,
            c: 0,
            envelope: 0,
            envelope_value: 0,
            fmt,
            data: ptr::null_mut(),
            embedded_ix: ptr::null_mut(),
        });
    }
    fn sample(&mut self, data: &mut PhonemeData, length_mod: i32, amp: i32) {
        self.with_data(SAMPLE, 0, [length_mod, amp, 0], data);
    }
    fn set_synthflags(&mut self, index: usize, flags: u16) {
        self.call(SET_SYNTHFLAGS, index, [i32::from(flags), 0, 0]);
    }
    fn set_std_length(&mut self, index: usize, value: u8) {
        self.call(SET_STD_LENGTH, index, [i32::from(value), 0, 0]);
    }
}

/// Returns 0 when the clause is finished (`*count` is then 0), 1 to wait for
/// queue space, 2 for invalid admission, or 3 + the native error.
#[no_mangle]
unsafe extern "C" fn espeak_rs_generate(
    entries: *mut Entry,
    length: usize,
    count: *mut usize,
    resume: u32,
    state: *mut State,
    settings: *const Settings,
    context: *mut c_void,
    callback: Option<Callback>,
) -> i32 {
    let Some(callback) = callback else {
        return 2;
    };
    if entries.is_null()
        || length == 0
        || length > generate::MAX_ENTRIES
        || count.is_null()
        || state.is_null()
        || settings.is_null()
    {
        return 2;
    }
    // SAFETY: exclusive initialized entry span, count and state, and an
    // initialized settings copy, all disjoint; the callback never touches
    // them (it updates the engine's own list).
    let (entries, count, state, settings) = unsafe {
        (
            slice::from_raw_parts_mut(entries, length),
            &mut *count,
            &mut *state,
            &*settings,
        )
    };
    let mut host = Callbacks { context, callback };
    match generate::generate(entries, count, resume != 0, state, settings, &mut host) {
        Ok(false) => 0,
        Ok(true) => 1,
        Err(error) => 3 + error as i32,
    }
}
