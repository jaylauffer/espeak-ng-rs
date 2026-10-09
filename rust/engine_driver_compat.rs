//! Typed projections of serialized C resources for native synthesis control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::engine_driver::{self as driver, Host, Pass};
use crate::events::{Event, EventList};
use crate::output::Output;
use std::ffi::{c_long, c_void};
use std::ptr;
type Callback = unsafe extern "C" fn(*mut i16, i32, *mut Event) -> i32;
type Step = unsafe extern "C" fn(*mut c_void) -> i32;
#[repr(C)]
pub struct Callbacks {
    output: *mut Output,
    events: *mut EventList,
    samples: *mut c_long,
    options: [*mut i32; 3],
    translator: *mut *mut c_void,
    decoder: *mut *mut c_void,
    encoding: unsafe extern "C" fn(*mut c_void) -> u32,
    voice: unsafe extern "C" fn() -> u32,
    create_decoder: unsafe extern "C" fn() -> *mut c_void,
    decode: unsafe extern "C" fn(*mut c_void, *const c_void, u32, i32) -> u32,
    begin: unsafe extern "C" fn(*mut Output),
    fill: unsafe extern "C" fn() -> i32,
    terminate_events: unsafe extern "C" fn(*mut EventList, i32, u32, *mut c_void),
    identifier: *mut u32,
    user: *mut *mut c_void,
    value: unsafe extern "C" fn(u32) -> i32,
    callback: *mut Option<Callback>,
    play: Callback,
    dispatch: Callback,
    generate: unsafe extern "C" fn() -> i32,
    queued: unsafe extern "C" fn() -> i32,
    clause: unsafe extern "C" fn(i32) -> i32,
    run: Option<unsafe extern "C" fn(Step, *mut c_void) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
/// Every slot/record is initialized, live and serialized for the call. Only
/// the active output prefix is readable; unfilled PCM/event tails are not
/// borrowed. Host calls must preserve these owners and the immutable table.
unsafe fn admit<'a>(cb: *const Callbacks) -> Option<Engine<'a>> {
    if cb.is_null() {
        return None;
    }
    // SAFETY: caller admits a live immutable table with valid primitive funcs.
    let cb = unsafe { &*cb };
    if cb.output.is_null()
        || cb.events.is_null()
        || cb.samples.is_null()
        || cb.options.iter().any(|p| p.is_null())
        || cb.translator.is_null()
        || cb.decoder.is_null()
        || cb.identifier.is_null()
        || cb.user.is_null()
        || cb.callback.is_null()
        || cb.run.is_none()
    {
        return None;
    }
    Some(Engine(cb))
}
struct RunContext {
    callbacks: *const Callbacks,
    id: u32,
    status: u32,
}
unsafe extern "C" fn pass(context: *mut c_void) -> i32 {
    let context = context.cast::<RunContext>();
    // SAFETY: run admits this initialized stack context until its synchronous
    // completion/fence. Copy pointers/scalars without retaining a context borrow
    // across a pass, which can invoke arbitrary owner/caller callbacks.
    let (cb, id) = unsafe { ((*context).callbacks, (*context).id) };
    // SAFETY: table remains admitted; status is a disjoint writable scalar.
    unsafe { espeak_rs_driver_step(cb, id, ptr::addr_of_mut!((*context).status)) }
}
impl Host for Engine<'_> {
    fn buffers_ready(&self) -> bool {
        // SAFETY: initialized live records; short shared inspection ends before
        // any host call. Buffer bytes and unused event tails are never borrowed.
        unsafe { !(*self.0.output).buffer().is_null() && !(*self.0.events).events.is_null() }
    }
    fn configure(&mut self, flags: i32) {
        for (slot, mask) in self.0.options.iter().zip([0x10, 0x100, 0x1000]) {
            // SAFETY: admitted disjoint scalar options; no callback in stores.
            unsafe {
                slot.write(flags & mask);
            }
        }
        // SAFETY: admitted writable native long counter.
        unsafe {
            self.0.samples.write(0);
        }
    }
    fn has_translator(&self) -> bool {
        // SAFETY: initialized admitted pointer slot, inspected without a borrow.
        unsafe { !self.0.translator.read().is_null() }
    }
    fn default_voice(&mut self) -> u32 {
        // SAFETY: configured voice primitive installs a valid translator on
        // success; its typed resource owner remains responsible for reclamation.
        unsafe { (self.0.voice)() }
    }
    fn has_decoder(&self) -> bool {
        // SAFETY: admitted initialized decoder slot.
        unsafe { !self.0.decoder.read().is_null() }
    }
    fn create_decoder(&mut self) {
        // SAFETY: factory produces an owned decoder or null. Publish its slot
        // after return, without borrowing globals through allocation callbacks.
        unsafe {
            self.0.decoder.write((self.0.create_decoder)());
        }
    }
    fn decode(&mut self, text: *const c_void, flags: i32) -> u32 {
        // SAFETY: successful voice setup admits a live translator. Input and
        // decoder remain live through synthesis under the existing API contract;
        // the native decoder itself handles a failed/null decoder allocation.
        unsafe {
            let encoding = (self.0.encoding)(self.0.translator.read());
            (self.0.decode)(self.0.decoder.read(), text, encoding, flags)
        }
    }
    fn clause(&mut self, control: i32) -> i32 {
        // SAFETY: typed engine primitive; no foreign resource borrow held.
        unsafe { (self.0.clause)(control) }
    }
    fn run(&mut self, id: u32) -> Result<u32, u32> {
        let mut context = RunContext {
            callbacks: self.0,
            id,
            status: 0,
        };
        // SAFETY: admitted completion runner invokes passes only while this
        // stack context is live, and fences all queued work before returning.
        let result = unsafe { self.0.run.unwrap()(pass, ptr::addr_of_mut!(context).cast()) };
        if result < 0 {
            Err(22)
        } else {
            Ok(context.status)
        }
    }
    fn begin_buffer(&mut self) {
        // SAFETY: begin mutates only the output cursor. Its internal exclusive
        // borrow ends on return; event count is a separate admitted scalar.
        unsafe {
            (self.0.begin)(self.0.output);
            (*self.0.events).count = 0;
        }
    }
    fn fill_buffer(&mut self) {
        // SAFETY: admitted bounded generator; foreign buffers are not borrowed
        // in Rust while generation/marker/output callbacks populate them.
        unsafe {
            (self.0.fill)();
        }
    }
    fn collect_buffer(&mut self, id: u32) -> Result<i32, u32> {
        // SAFETY: admitted live cursor/counter. Copy scalars only; no borrow
        // survives event termination. Defined C lengths truncate odd bytes.
        let (length, count) = unsafe {
            let buffer = (*self.0.output).buffer() as usize;
            let written = ((*self.0.output).ptr as usize)
                .checked_sub(buffer)
                .ok_or(22u32)?;
            if written > (*self.0.output).size() {
                return Err(22);
            }
            let length = i32::try_from(written / 2).map_err(|_| 22u32)?;
            let count = self
                .0
                .samples
                .read()
                .checked_add(c_long::from(length))
                .ok_or(22u32)?;
            (length, count)
        };
        // SAFETY: admitted scalar and current event count/user. Native event
        // termination writes its bounded prefix, not an undefined output tail.
        unsafe {
            self.0.samples.write(count);
            (self.0.terminate_events)(
                self.0.events,
                (*self.0.events).count,
                id,
                self.0.user.read(),
            );
        }
        Ok(length)
    }
    fn playback(&self) -> bool {
        // SAFETY: atomic mode getter; read freshly after each owner callback.
        unsafe { (self.0.value)(0) & 2 != 0 }
    }
    fn play(&mut self, length: i32, end: bool) -> i32 {
        // SAFETY: admitted output/event pointers through this primitive call.
        // End dispatch uses the API's null samples/event marker convention.
        unsafe {
            if end {
                (self.0.dispatch)(ptr::null_mut(), 0, ptr::null_mut())
            } else {
                (self.0.play)(
                    (*self.0.output).buffer().cast(),
                    length,
                    (*self.0.events).events,
                )
            }
        }
    }
    fn callback(&mut self, length: i32, end: bool) -> i32 {
        // SAFETY: initialized optional callback slot, read freshly. No mutable
        // Rust borrow of callback, PCM, event list or shared resources persists.
        unsafe {
            self.0.callback.read().map_or(0, |callback| {
                let samples = if end {
                    ptr::null_mut()
                } else {
                    (*self.0.output).buffer().cast()
                };
                callback(samples, length, (*self.0.events).events)
            })
        }
    }
    fn generate(&mut self) -> i32 {
        // SAFETY: admitted primitive takes the current clause/list state.
        unsafe { (self.0.generate)() }
    }
    fn queued(&self) -> i32 {
        // SAFETY: admitted native queue length primitive.
        unsafe { (self.0.queued)() }
    }
    fn terminate_current_events(&mut self) {
        // SAFETY: admitted event list and identity slots, read freshly after
        // the delivery/generation callbacks without borrowing them through calls.
        unsafe {
            (self.0.terminate_events)(
                self.0.events,
                0,
                self.0.identifier.read(),
                self.0.user.read(),
            );
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_driver_step(cb: *const Callbacks, id: u32, status: *mut u32) -> i32 {
    // SAFETY: immutable callback/serialized resource admission described above.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return -1;
    };
    if status.is_null() {
        return -1;
    }
    match driver::step(&mut host, id) {
        Pass::Continue => 0,
        Pass::Done(value) => {
            // SAFETY: disjoint caller-owned initialized writable status slot;
            // publish only after callbacks and leave it unchanged on Continue.
            unsafe {
                status.write(value);
            }
            1
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_driver_synthesize(
    cb: *const Callbacks,
    id: u32,
    text: *const c_void,
    flags: i32,
) -> u32 {
    // SAFETY: immutable callback/serialized resource admission described above.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    driver::synthesize(&mut host, id, text, flags)
}
