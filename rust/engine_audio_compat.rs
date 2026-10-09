//! Serialized resource projections for the native audio dispatcher.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::engine_audio::{self as audio, Host};
use crate::events::{Event, EventList};
use std::ffi::{c_long, c_void};
use std::ptr;
type Callback = unsafe extern "C" fn(*mut i16, i32, *mut Event) -> i32;
#[repr(C)]
pub struct Callbacks {
    capabilities: u32,
    format: i32,
    value: unsafe extern "C" fn(u32) -> i32,
    store: unsafe extern "C" fn(u32, i32),
    audio: unsafe extern "C" fn() -> *mut c_void,
    enabled: Option<unsafe extern "C" fn() -> i32>,
    close: Option<unsafe extern "C" fn(*mut c_void)>,
    open: Option<unsafe extern "C" fn(*mut c_void, i32, i32, i32) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> i32>,
    diagnostic: Option<unsafe extern "C" fn(u32, i32)>,
    event_init: Option<unsafe extern "C" fn()>,
    latency: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    declare: Option<unsafe extern "C" fn(*const Event, i32) -> u32>,
    samples: *const c_long,
    mbrola_delay: *const i32,
    callback: *const Option<Callback>,
    events: *const EventList,
}
struct Engine<'a>(&'a Callbacks, *mut Event);
unsafe fn admit<'a>(table: *const Callbacks) -> Option<Engine<'a>> {
    if table.is_null() {
        return None;
    }
    // SAFETY: caller admits a live immutable table and valid primitive funcs.
    let cb = unsafe { &*table };
    if cb.samples.is_null()
        || cb.callback.is_null()
        || cb.events.is_null()
        || (cb.capabilities & audio::ASYNC != 0
            && (cb.enabled.is_none() || cb.event_init.is_none() || cb.declare.is_none()))
        || (cb.capabilities & audio::AUDIO != 0
            && (cb.close.is_none()
                || cb.open.is_none()
                || cb.write.is_none()
                || cb.diagnostic.is_none()))
        || (cb.capabilities & audio::LATENCY != 0 && cb.latency.is_none())
    {
        return None;
    }
    Some(Engine(cb, ptr::null_mut()))
}
impl Host for Engine<'_> {
    type Samples = *mut i16;
    type Event = *mut Event;
    fn capabilities(&self) -> u32 {
        self.0.capabilities
    }
    fn mode(&self) -> i32 {
        // SAFETY: admitted atomic state primitive; no resource borrow held.
        unsafe { (self.0.value)(0) }
    }
    fn command_enabled(&mut self) -> i32 {
        // SAFETY: async primitives are admitted together.
        unsafe { self.0.enabled.unwrap()() }
    }
    fn voice_rate(&self) -> i32 {
        // SAFETY: admitted state primitive, read freshly after callbacks.
        unsafe { (self.0.value)(2) }
    }
    fn output_rate(&self) -> i32 {
        // SAFETY: admitted state primitive.
        unsafe { (self.0.value)(1) }
    }
    fn set_voice_rate(&mut self, rate: i32) {
        // SAFETY: atomic state store; no foreign borrow held.
        unsafe {
            (self.0.store)(2, rate);
        }
    }
    fn set_output_rate(&mut self, rate: i32) {
        // SAFETY: atomic state store.
        unsafe {
            (self.0.store)(1, rate);
        }
    }
    fn set_error(&mut self, status: i32) {
        // SAFETY: atomic state store.
        unsafe {
            (self.0.store)(3, status);
        }
    }
    fn close(&mut self) {
        // SAFETY: admitted audio owner/primitive remains live through return.
        unsafe {
            self.0.close.unwrap()((self.0.audio)());
        }
    }
    fn open(&mut self) -> i32 {
        // SAFETY: owner/primitive admission, fresh device and rate projections.
        unsafe { self.0.open.unwrap()((self.0.audio)(), self.0.format, self.voice_rate(), 1) }
    }
    fn diagnostic(&mut self, operation: u32, error: i32) {
        // SAFETY: bounded operation 0..=2 and admitted platform diagnostic.
        unsafe {
            self.0.diagnostic.unwrap()(operation, error);
        }
    }
    fn event_init(&mut self) {
        // SAFETY: admitted async initialization primitive.
        unsafe {
            self.0.event_init.unwrap()();
        }
    }
    fn has_samples(&self, samples: Self::Samples) -> bool {
        !samples.is_null()
    }
    fn write(&mut self, samples: Self::Samples, bytes: usize) -> i32 {
        // SAFETY: caller admits the initialized PCM extent through this call;
        // the existing device operation fences its completion before returning.
        unsafe { self.0.write.unwrap()((self.0.audio)(), samples.cast(), bytes) }
    }
    fn callback(&mut self, samples: Self::Samples, length: i32, event: Option<Self::Event>) {
        // SAFETY: initialized optional slot and admitted PCM/event extent. No
        // mutable Rust resource or callback borrow survives caller execution.
        unsafe {
            if let Some(callback) = self.0.callback.read() {
                callback(samples, length, event.unwrap_or(ptr::null_mut()));
            }
        }
    }
    fn kind(&self, event: Self::Event) -> i32 {
        // SAFETY: admitted initialized event; scalar read without borrowing.
        unsafe { (*event).kind }
    }
    fn event_length(&self, event: Self::Event) -> i32 {
        // SAFETY: admitted initialized event.
        unsafe { (*event).length }
    }
    fn event_rate(&self, event: Self::Event) -> i32 {
        // SAFETY: read only the defined numeric prefix of the event union.
        unsafe { ptr::addr_of!((*event).id).cast::<i32>().read_unaligned() }
    }
    fn event_sample(&self, event: Self::Event) -> i32 {
        // SAFETY: admitted initialized event scalar.
        unsafe { (*event).sample }
    }
    fn has_audio(&self) -> bool {
        // SAFETY: admitted atomic device projection.
        unsafe { !(self.0.audio)().is_null() }
    }
    fn samples(&self) -> i128 {
        // SAFETY: initialized count and optional delay slots, copied without
        // a borrow before latency/event callbacks. Widen before adding.
        unsafe {
            i128::from(self.0.samples.read())
                + if self.0.mbrola_delay.is_null() {
                    0
                } else {
                    i128::from(self.0.mbrola_delay.read())
                }
        }
    }
    fn latency(&mut self) -> i32 {
        // SAFETY: latency primitive admitted with its capability; live owner.
        unsafe { self.0.latency.unwrap()((self.0.audio)()) }
    }
    fn admit(&mut self, event: Self::Event, delay: i32) -> u32 {
        // SAFETY: admitted event remains live through the existing proactor
        // capacity wait; that operation copies owned declaration data.
        unsafe { self.0.declare.unwrap()(event, delay) }
    }
    fn capacity(&self) -> usize {
        // SAFETY: initialized admitted list metadata, scalar projection only.
        unsafe { (*self.0.events).capacity.max(0) as usize }
    }
    fn count(&self) -> i32 {
        // SAFETY: fresh initialized count after callbacks.
        unsafe { (*self.0.events).count }
    }
    fn event_at(&self, index: usize) -> Result<Option<Self::Event>, i32> {
        // SAFETY: serialized list remains live throughout dispatch; callback
        // may change count but must keep this owner/storage admitted and filled.
        let (count, capacity) = unsafe { ((*self.0.events).count, (*self.0.events).capacity) };
        let events = self.1;
        if count < 0
            || capacity < 0
            || count > capacity
            || capacity as usize > isize::MAX as usize / std::mem::size_of::<Event>()
        {
            return Err(22);
        }
        if count == 0 && index == 0 {
            return Ok(None);
        }
        if events.is_null() || index >= count as usize {
            return Err(22);
        }
        // SAFETY: current initialized prefix contains index; no event borrow
        // survives dispatch or arbitrary caller callbacks.
        Ok(Some(unsafe { events.add(index) }))
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_audio_dispatch(
    cb: *const Callbacks,
    samples: *mut i16,
    length: i32,
    event: *mut Event,
) -> i32 {
    // SAFETY: immutable table, PCM extent and optional initialized event are
    // admitted by the serialized engine for the duration of all owner calls.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return -1;
    };
    audio::dispatch(
        &mut host,
        samples,
        length,
        (!event.is_null()).then_some(event),
    )
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_audio_events(
    cb: *const Callbacks,
    samples: *mut i16,
    length: i32,
    events: *mut Event,
) -> i32 {
    // SAFETY: same table/PCM contract, plus list's initialized prefix remains
    // admitted during each callback; no list borrow crosses one.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return -1;
    };
    host.1 = events;
    audio::create(&mut host, samples, length)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_audio_declare(cb: *const Callbacks, event: *mut Event) -> u32 {
    // SAFETY: table and initialized event remain admitted through the wait.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    if event.is_null() || host.capabilities() & audio::ASYNC == 0 {
        return 22;
    }
    audio::declare(&mut host, event)
}
