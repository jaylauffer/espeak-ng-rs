//! Primitive C callbacks for native engine lifecycle control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::engine_lifecycle::{self as engine, Action, Capabilities, Field, Host, State};
use std::ffi::{c_char, c_long, c_void};
use std::ptr;
static STATE: State = State::new();
#[repr(C)]
pub struct Callbacks {
    capabilities: u32,
    actions: [unsafe extern "C" fn(); 22],
    locale: unsafe extern "C" fn(i32, *const c_char) -> *mut c_char,
    ctype: i32,
    load: unsafe extern "C" fn(*mut i32, *mut *mut c_void) -> u32,
    wave_init: unsafe extern "C" fn(i32, i32),
    defaults: *const i32,
    current: *mut i32,
    saved: *mut i32,
    capitals: *mut i32,
    punctuation: *mut i32,
    phonemes: *mut i32,
    phoneme_events: *mut i32,
    echo: *mut i32,
    parameter: unsafe extern "C" fn(i32, i32, i32) -> u32,
    clock: unsafe extern "C" fn() -> c_long,
    seed: unsafe extern "C" fn(c_long),
    create_audio:
        Option<unsafe extern "C" fn(*const c_char, *const c_char, *const c_char) -> *mut c_void>,
    close_audio: Option<unsafe extern "C" fn(*mut c_void)>,
    destroy_audio: Option<unsafe extern "C" fn(*mut c_void)>,
    flush_audio: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    output_reserve: unsafe extern "C" fn(usize) -> i32,
    events_reserve: unsafe extern "C" fn(i32) -> i32,
    synchronize: Option<unsafe extern "C" fn() -> u32>,
    translator: *mut *mut c_void,
    destroy_translator: unsafe extern "C" fn(*mut c_void),
    decoder: *mut *mut c_void,
    destroy_decoder: unsafe extern "C" fn(*mut c_void),
}
struct Engine<'a> {
    cb: &'a Callbacks,
    context: *mut *mut c_void,
    device: *const c_char,
}
unsafe fn admit<'a>(pointer: *const Callbacks) -> Option<Engine<'a>> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: caller admits an immutable live table throughout the operation.
    let cb = unsafe { &*pointer };
    if cb.defaults.is_null()
        || cb.current.is_null()
        || cb.saved.is_null()
        || cb.capitals.is_null()
        || cb.punctuation.is_null()
        || cb.phonemes.is_null()
        || cb.phoneme_events.is_null()
        || cb.echo.is_null()
        || cb.translator.is_null()
        || cb.decoder.is_null()
    {
        return None;
    }
    if cb.capabilities & 2 != 0
        && (cb.create_audio.is_none()
            || cb.close_audio.is_none()
            || cb.destroy_audio.is_none()
            || cb.flush_audio.is_none())
    {
        return None;
    }
    Some(Engine {
        cb,
        context: ptr::null_mut(),
        device: ptr::null(),
    })
}
impl Host for Engine<'_> {
    fn capabilities(&self) -> Capabilities {
        let bits = self.cb.capabilities;
        Capabilities {
            asynchronous: bits & 1 != 0,
            audio: bits & 2 != 0,
            mbrola: bits & 4 != 0,
            proactor: bits & 8 != 0,
        }
    }
    fn action(&mut self, action: Action) {
        // SAFETY: admitted function table and typed bounded operation index.
        unsafe {
            (self.cb.actions[action as usize])();
        }
    }
    fn locale(&mut self, name: &'static [u8]) -> bool {
        // SAFETY: terminated static name and matching CRT locale category.
        !unsafe { (self.cb.locale)(self.cb.ctype, name.as_ptr().cast()) }.is_null()
    }
    fn load_phonemes(&mut self, rate: &mut i32) -> u32 {
        // SAFETY: exclusive initialized rate, caller's live optional context
        // slot; neither stack address may be retained by this callback.
        unsafe { (self.cb.load)(rate, self.context) }
    }
    fn wave_init(&mut self, rate: i32) {
        // SAFETY: admitted engine callback; no global mutable borrow held.
        unsafe {
            (self.cb.wave_init)(rate, 0);
        }
    }
    fn defaults(&mut self) {
        for index in 0..15 {
            // SAFETY: initialized immutable defaults and two distinct writable
            // 15-element arrays, admitted by the serialized C engine owner.
            unsafe {
                let value = self.cb.defaults.add(index).read();
                self.cb.current.add(index).write(value);
                self.cb.saved.add(index).write(value);
            }
        }
    }
    fn option(&self, index: usize) -> i32 {
        let slot = if index == 0 {
            self.cb.capitals
        } else {
            self.cb.punctuation
        };
        // SAFETY: admitted initialized scalar; no borrow crosses callbacks.
        unsafe { slot.read() }
    }
    fn parameter(&mut self, index: usize, value: i32) {
        // SAFETY: bounded native parameter index and admitted C callback.
        unsafe {
            (self.cb.parameter)(index as i32, value, 0);
        }
    }
    fn saved(&self, index: usize) -> i32 {
        // SAFETY: admitted 15-element initialized array and native bounded
        // index. Read each value freshly, without a borrow across callbacks.
        unsafe { self.cb.saved.add(index).read() }
    }
    fn phoneme_flags(&mut self, value: i32) {
        // SAFETY: unique admitted scalar stores; no callback during mutation.
        unsafe {
            self.cb.phonemes.write(value);
            self.cb.phoneme_events.write(value);
        }
    }
    fn echo_reset(&mut self) {
        // SAFETY: unique admitted echo scalar, no borrowed view retained.
        unsafe {
            self.cb.echo.write(0);
        }
    }
    fn seed(&mut self) {
        // SAFETY: matching nonblocking platform clock and native seed callback.
        unsafe {
            (self.cb.seed)((self.cb.clock)());
        }
    }
    fn create_audio(&mut self) -> *mut c_void {
        // SAFETY: audio callbacks admitted together; device borrowed through
        // creation, static terminated application/description names.
        unsafe {
            (self.cb.create_audio.expect("admitted audio"))(
                self.device,
                c"eSpeak".as_ptr(),
                c"Text-to-Speech".as_ptr(),
            )
        }
    }
    fn close_audio(&mut self, pointer: *mut c_void) {
        // SAFETY: owned admitted handle, detached after all queue workers exit.
        unsafe {
            (self.cb.close_audio.expect("admitted audio"))(pointer);
        }
    }
    fn destroy_audio(&mut self, pointer: *mut c_void) {
        // SAFETY: detached handle relinquished exactly once to its host.
        unsafe {
            (self.cb.destroy_audio.expect("admitted audio"))(pointer);
        }
    }
    fn flush_audio(&mut self, pointer: *mut c_void) {
        // SAFETY: audio callback accepts its current optional live handle.
        unsafe {
            (self.cb.flush_audio.expect("admitted audio"))(pointer);
        }
    }
    fn output_reserve(&mut self, bytes: usize) -> bool {
        // SAFETY: checked bounded size; host mutates its owned output locally.
        unsafe { (self.cb.output_reserve)(bytes) == 0 }
    }
    fn events_reserve(&mut self, count: i32) -> bool {
        // SAFETY: checked positive count and matching event-owner callback.
        unsafe { (self.cb.events_reserve)(count) == 0 }
    }
    fn synchronize(&mut self) -> u32 {
        match self.cb.synchronize {
            // SAFETY: host completion wait; no state/global borrow or lock.
            Some(callback) => unsafe { callback() },
            None => 22,
        }
    }
    fn translator_release(&mut self) {
        // SAFETY: unique admitted slot. Relinquish before destruction; the
        // callback accepts null as the legacy DeleteTranslator does.
        unsafe {
            let pointer = ptr::replace(self.cb.translator, ptr::null_mut());
            (self.cb.destroy_translator)(pointer);
        }
    }
    fn decoder_release(&mut self) {
        // SAFETY: unique admitted slot, detached before its sole destructor.
        unsafe {
            let pointer = ptr::replace(self.cb.decoder, ptr::null_mut());
            if !pointer.is_null() {
                (self.cb.destroy_decoder)(pointer);
            }
        }
    }
}
#[no_mangle]
extern "C" fn espeak_rs_engine_value(index: u32) -> i32 {
    Field::from_index(index).map_or(0, |field| STATE.get(field))
}
#[no_mangle]
extern "C" fn espeak_rs_engine_store(index: u32, value: i32) {
    if let Some(field) = Field::from_index(index) {
        STATE.set(field, value);
    }
}
#[no_mangle]
extern "C" fn espeak_rs_engine_audio() -> *mut c_void {
    STATE.audio()
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_initialize(
    cb: *const Callbacks,
    context: *mut *mut c_void,
) -> u32 {
    // SAFETY: caller supplies the immutable table and serialized resource slots.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    host.context = context;
    engine::initialize(&mut host)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_output(
    cb: *const Callbacks,
    mode: i32,
    length: i32,
    rate: i32,
    device: *const c_char,
) -> u32 {
    // SAFETY: caller's live table, scalar options and borrowed device name.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    host.device = device;
    engine::output(&STATE, &mut host, mode, length, rate)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_cancel(cb: *const Callbacks) -> u32 {
    // SAFETY: same serialized resource/table contract as initialization.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    engine::cancel(&STATE, &mut host)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_synchronize(cb: *const Callbacks) -> u32 {
    // SAFETY: admitted table; native wait releases all state/owner locks.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    engine::synchronize(&STATE, &mut host)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_terminate(cb: *const Callbacks) -> u32 {
    // SAFETY: admitted serialized lifecycle owner; queue termination fences
    // resource users before any destructor callback can reclaim their storage.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    engine::terminate(&STATE, &mut host)
}
