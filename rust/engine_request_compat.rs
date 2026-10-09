//! C resource projections for the native request controllers.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::async_command::{
    self as command, Character, Key, Mark, Parameter, Text, View, ViewData, Wide,
};
use crate::engine_request::{self as request, AdmissionHost, Request, SynthesisHost};
use std::ffi::{c_char, c_void};
use std::ptr;

/// The immutable table, slots and array extents remain live during each call.
/// The engine owner serializes resource access; callbacks may inspect/reenter
/// globals. No mutable Rust reference to those globals survives a callback.
#[repr(C)]
pub struct Callbacks {
    capabilities: u32,
    value: unsafe extern "C" fn(u32) -> i32,
    audio: unsafe extern "C" fn() -> *mut c_void,
    init_text: unsafe extern "C" fn(i32),
    synthesize: unsafe extern "C" fn(u32, *const c_void, i32) -> u32,
    key: unsafe extern "C" fn(*const c_char) -> u32,
    character: unsafe extern "C" fn(Wide) -> u32,
    parameter: unsafe extern "C" fn(i32, i32, i32) -> u32,
    punctuation: unsafe extern "C" fn(*const Wide),
    create: Option<unsafe extern "C" fn(*const View, usize) -> *mut View>,
    single: Option<unsafe extern "C" fn(*mut View) -> u32>,
    pair: Option<unsafe extern "C" fn(*mut View, *mut View) -> u32>,
    delete_command: Option<unsafe extern "C" fn(*mut View) -> i32>,
    identifier: *mut u32,
    user: *mut *mut c_void,
    current: *const i32,
    saved: *mut i32,
    skip: [*mut i32; 3],
    skipping: *mut bool,
    end: *mut i32,
    marker: *mut c_char,
    flush: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    drain: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    audio_error: Option<unsafe extern "C" fn(*mut c_void, i32) -> *const c_char>,
    diagnose: Option<unsafe extern "C" fn(*const c_char, *const c_char)>,
}
struct Engine<'a> {
    cb: &'a Callbacks,
    output_identifier: *mut u32,
}
unsafe fn admit<'a>(cb: *const Callbacks) -> Option<Engine<'a>> {
    if cb.is_null() {
        return None;
    }
    // SAFETY: caller supplies a live immutable table with valid nonoptional
    // function pointers. Slots and arrays below have their documented extents.
    let cb = unsafe { &*cb };
    if cb.identifier.is_null()
        || cb.user.is_null()
        || cb.current.is_null()
        || cb.saved.is_null()
        || cb.skip.iter().any(|p| p.is_null())
        || cb.skipping.is_null()
        || cb.end.is_null()
        || cb.marker.is_null()
        || (cb.capabilities & 1 != 0
            && (cb.create.is_none()
                || cb.single.is_none()
                || cb.pair.is_none()
                || cb.delete_command.is_none()))
        || (cb.capabilities & 2 != 0
            && (cb.flush.is_none()
                || cb.drain.is_none()
                || cb.audio_error.is_none()
                || cb.diagnose.is_none()))
    {
        return None;
    }
    Some(Engine {
        cb,
        output_identifier: ptr::null_mut(),
    })
}
fn view(request: Request) -> View {
    let (kind, data) = match request {
        Request::Text(args) => (0, ViewData { text: args }),
        Request::Mark(args) => (1, ViewData { mark: args }),
        Request::Key(args) => (2, ViewData { key: args }),
        Request::Character(args) => (3, ViewData { character: args }),
        Request::Parameter(args) => (4, ViewData { parameter: args }),
        Request::Punctuation(list) => (5, ViewData { punctuation: list }),
    };
    View {
        kind,
        state: command::UNDEFINED,
        data,
    }
}
unsafe fn input(view: &View) -> Option<Request> {
    // SAFETY: caller initializes the active member selected by kind. Only
    // that member is copied; no foreign view borrow crosses a host call.
    Some(unsafe {
        match view.kind {
            0 => Request::Text(view.data.text),
            1 => Request::Mark(view.data.mark),
            2 => Request::Key(view.data.key),
            3 => Request::Character(view.data.character),
            4 => Request::Parameter(view.data.parameter),
            5 => Request::Punctuation(view.data.punctuation),
            _ => return None,
        }
    })
}
impl AdmissionHost for Engine<'_> {
    type Command = *mut View;
    fn asynchronous(&self) -> bool {
        self.cb.capabilities & 1 != 0
    }
    fn mode(&self) -> i32 {
        // SAFETY: admitted atomic engine scalar getter; Mode = 0.
        unsafe { (self.cb.value)(0) }
    }
    fn publish_identifier(&mut self, id: u32) {
        if !self.output_identifier.is_null() {
            // SAFETY: optional caller-owned writable identifier slot. Store
            // immediately so notification/queue callbacks observe this value.
            unsafe {
                self.output_identifier.write(id);
            }
        }
    }
    fn synchronous(&mut self, input: Request) -> u32 {
        match input {
            Request::Text(args) => request::synthesize(self, args),
            Request::Mark(args) => request::synthesize_mark(self, args),
            // SAFETY: admitted engine callbacks and caller payload lifetime.
            Request::Key(Key { name, .. }) => unsafe { (self.cb.key)(name) },
            // SAFETY: admitted callback and scalar wchar_t value.
            Request::Character(Character { character, .. }) => unsafe {
                (self.cb.character)(character)
            },
            // SAFETY: admitted callback and scalar parameter triple.
            Request::Parameter(Parameter {
                parameter,
                value,
                relative,
            }) => {
                // SAFETY: admitted callback and scalar parameter triple.
                unsafe { (self.cb.parameter)(parameter, value, relative) }
            }
            Request::Punctuation(list) => {
                // SAFETY: caller's live optional wide string and engine callback.
                unsafe {
                    (self.cb.punctuation)(list);
                }
                0
            }
        }
    }
    fn create(&mut self, input: Request, size: usize) -> Option<(*mut View, u32)> {
        let view = view(input);
        // SAFETY: validated async table. Native creation copies the active
        // payload before returning; the stack prefix is never retained.
        let owner = unsafe { self.cb.create.unwrap()(&view, size) };
        if owner.is_null() {
            return None;
        }
        // SAFETY: creation returned a live command of this exact kind. Read
        // identifier once before any queue/callback can take ownership.
        let id = unsafe {
            match view.kind {
                0 => (*owner).data.text.id,
                1 => (*owner).data.mark.id,
                _ => 0,
            }
        };
        Some((owner, id))
    }
    fn terminated(&mut self, id: u32, user: *mut c_void) -> Option<*mut View> {
        let view = View {
            kind: 8,
            state: command::UNDEFINED,
            data: ViewData {
                terminated: command::Terminated { id, user },
            },
        };
        // SAFETY: validated async callback; initialized notification scalars
        // are captured before this stack prefix is released.
        let command = unsafe { self.cb.create.unwrap()(&view, 0) };
        (!command.is_null()).then_some(command)
    }
    fn enqueue_pair(
        &mut self,
        a: *mut View,
        b: *mut View,
    ) -> Result<(), (u32, *mut View, *mut View)> {
        // SAFETY: two uniquely owned commands. Queue success takes both;
        // failure takes neither. No foreign command borrow is held here.
        let status = unsafe { self.cb.pair.unwrap()(a, b) };
        if status == 0 {
            Ok(())
        } else {
            Err((status, a, b))
        }
    }
    fn enqueue(&mut self, command: Option<*mut View>) -> Result<(), (u32, Option<*mut View>)> {
        // SAFETY: uniquely owned command or legacy null rejection. Success
        // takes ownership, failure leaves it with this request controller.
        let status = unsafe { self.cb.single.unwrap()(command.unwrap_or(ptr::null_mut())) };
        if status == 0 {
            Ok(())
        } else {
            Err((status, command))
        }
    }
    fn delete(&mut self, command: Option<*mut View>) {
        // SAFETY: failed/unsubmitted command relinquished exactly once; the
        // established deletion callback accepts null and completes messages.
        unsafe {
            self.cb.delete_command.unwrap()(command.unwrap_or(ptr::null_mut()));
        }
    }
}
impl SynthesisHost for Engine<'_> {
    fn initialize_text(&mut self, flags: u32) {
        // SAFETY: admitted serialized engine callback and C int flag bits.
        unsafe {
            (self.cb.init_text)(flags as i32);
        }
    }
    fn identity(&mut self, id: u32, user: *mut c_void) {
        // SAFETY: initialized admitted scalar slots; no callback between stores.
        unsafe {
            self.cb.identifier.write(id);
            self.cb.user.write(user);
        }
    }
    fn save_parameters(&mut self) {
        for index in 0..15 {
            // SAFETY: admitted initialized 15-element arrays. Per-element raw
            // copy preserves current state without borrowing globals for calls.
            unsafe {
                self.cb
                    .saved
                    .add(index)
                    .write(self.cb.current.add(index).read());
            }
        }
    }
    fn skip(&mut self, index: usize, position: u32) {
        // SAFETY: controller selects one of three admitted scalar slots.
        unsafe {
            self.cb.skip[index].write(position as i32);
        }
    }
    fn has_skips(&self) -> bool {
        self.cb.skip.iter().any(|p| {
            // SAFETY: admitted initialized scalars; read after InitText.
            unsafe { p.read() != 0 }
        })
    }
    fn skipping(&mut self) {
        // SAFETY: admitted C bool slot with matching one-byte representation.
        unsafe {
            self.cb.skipping.write(true);
        }
    }
    fn marker(&mut self, marker: *const c_char) {
        let mut ended = false;
        for index in 0..49 {
            // SAFETY: live terminated source through its first NUL; 50-byte
            // writable destination, disjoint from source per the C API. Match
            // strncpy0's zero padding, stopping source reads at its first NUL.
            unsafe {
                let byte = if ended { 0 } else { marker.add(index).read() };
                ended |= byte == 0;
                self.cb.marker.add(index).write(byte);
            }
        }
        // SAFETY: final byte of admitted fixed 50-byte marker buffer.
        unsafe {
            self.cb.marker.add(49).write(0);
        }
    }
    fn end(&mut self, position: u32) {
        // SAFETY: admitted initialized writable end-position scalar.
        unsafe {
            self.cb.end.write(position as i32);
        }
    }
    fn synthesize(&mut self, id: u32, text: *const c_void, flags: u32) -> u32 {
        // SAFETY: serialized engine callback; text is live through completion.
        unsafe { (self.cb.synthesize)(id, text, flags as i32) }
    }
    fn playback(&self) -> bool {
        self.cb.capabilities & 2 != 0 && self.mode() & 2 != 0
    }
    fn finish_audio(&mut self, flush: bool) {
        let operation = if flush { self.cb.flush } else { self.cb.drain }.unwrap();
        // SAFETY: admitted audio bundle and current admitted engine handle.
        let status = unsafe { operation((self.cb.audio)()) };
        if status != 0 {
            // SAFETY: read current handle freshly after the operation; CRT
            // error string stays live through immediate diagnostic callback.
            unsafe {
                let message = self.cb.audio_error.unwrap()((self.cb.audio)(), status);
                let verb: &[u8] = if flush { b"flush\0" } else { b"drain\0" };
                self.cb.diagnose.unwrap()(verb.as_ptr().cast(), message);
            }
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_request_submit(
    cb: *const Callbacks,
    view: *const View,
    size: usize,
    identifier: *mut u32,
) -> u32 {
    // SAFETY: admitted table and active prefix contract described above.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    if view.is_null() {
        return 22;
    }
    // SAFETY: caller's readable initialized active prefix; copied before calls.
    let Some(input) = (unsafe { input(&*view) }) else {
        return 22;
    };
    host.output_identifier = identifier;
    request::submit(&mut host, input, size)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_request_synthesize(cb: *const Callbacks, args: *const Text) -> u32 {
    // SAFETY: admitted immutable table and initialized slots/arrays.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    if args.is_null() {
        return 22;
    }
    // SAFETY: readable argument record copied before any engine callback.
    request::synthesize(&mut host, unsafe { args.read() })
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_request_mark(cb: *const Callbacks, args: *const Mark) -> u32 {
    // SAFETY: admitted immutable table and initialized slots/arrays.
    let Some(mut host) = (unsafe { admit(cb) }) else {
        return 22;
    };
    if args.is_null() {
        return 22;
    }
    // SAFETY: readable argument record copied before any engine callback.
    request::synthesize_mark(&mut host, unsafe { args.read() })
}
