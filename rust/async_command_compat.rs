//! C-prefix adapter for the owned asynchronous commands.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::async_command::{
    self as command, Command, Data, Host, Terminated, TextBuffer, View, Wide,
};
use std::ffi::{c_char, c_void, CStr};
use std::ptr;
use std::sync::atomic::AtomicU32;

static IDENTIFIER: AtomicU32 = AtomicU32::new(0);

/// Engine callbacks borrow payloads only for the call. They may reenter the
/// engine/queue, but cannot delete or mutate the command being processed.
#[repr(C)]
pub struct Callbacks {
    synth: unsafe extern "C" fn(u32, *const c_void, u32, i32, u32, u32, *mut c_void) -> i32,
    mark: unsafe extern "C" fn(u32, *const c_void, *const c_char, u32, u32, *mut c_void) -> i32,
    key: unsafe extern "C" fn(*const c_char) -> i32,
    character: unsafe extern "C" fn(Wide) -> i32,
    parameter: unsafe extern "C" fn(i32, i32, i32) -> i32,
    punctuation: unsafe extern "C" fn(*const Wide),
    voice_name: unsafe extern "C" fn(*const c_char) -> i32,
    voice: unsafe extern "C" fn(*mut command::Voice) -> i32,
    terminated: unsafe extern "C" fn(u32, *mut c_void) -> i32,
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn process(&mut self, data: &Data) {
        // SAFETY: validated callback table and a uniquely live command; each
        // buffer is owned until this call returns. No callback retains a borrow
        // or invalidates the command. The owner serializes engine globals.
        unsafe {
            match data {
                Data::Text(args, _) => {
                    (self.0.synth)(
                        args.id,
                        args.text,
                        args.position,
                        args.position_type,
                        args.end,
                        args.flags,
                        args.user,
                    );
                }
                Data::Mark(args, _, _) => {
                    (self.0.mark)(
                        args.id, args.text, args.mark, args.end, args.flags, args.user,
                    );
                }
                Data::Key(args, _) => {
                    (self.0.key)(args.name);
                }
                Data::Character(args) => {
                    (self.0.character)(args.character);
                }
                Data::Parameter(args) => {
                    (self.0.parameter)(args.parameter, args.value, args.relative);
                }
                Data::Punctuation(list) => (self.0.punctuation)(list.as_ptr()),
                Data::VoiceName(name) => {
                    (self.0.voice_name)(name.as_ptr().cast());
                }
                Data::Voice(args, _) => {
                    let mut voice = *args;
                    (self.0.voice)(&mut voice);
                }
                Data::Terminated(args) => self.terminated(*args),
            }
        }
    }
    fn terminated(&mut self, args: Terminated) {
        // SAFETY: owned scalar notification and the admitted engine callback.
        unsafe {
            (self.0.terminated)(args.id, args.user);
        }
    }
}

unsafe fn bytes(pointer: *const u8, length: usize, zeros: usize) -> Option<Vec<u8>> {
    let capacity = length.checked_add(zeros)?;
    if pointer.is_null() || capacity > isize::MAX as usize {
        return None;
    }
    let mut owned = Vec::new();
    owned.try_reserve_exact(capacity).ok()?;
    owned.resize(capacity, 0);
    // SAFETY: caller supplies length live initialized bytes, disjoint from
    // this new allocation. Initialized suffix supplies the terminator.
    unsafe {
        ptr::copy_nonoverlapping(pointer, owned.as_mut_ptr(), length);
    }
    Some(owned)
}
unsafe fn text(pointer: *const c_void, length: usize) -> Option<TextBuffer> {
    if pointer.is_null() || length == 0 || length > isize::MAX as usize {
        return None;
    }
    // SAFETY: caller supplies length initialized readable bytes for this call.
    let input = unsafe { std::slice::from_raw_parts(pointer.cast(), length) };
    TextBuffer::copy(input)
}
unsafe fn string(pointer: *const c_char) -> Option<Vec<u8>> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: caller supplies a live NUL-terminated C string.
    let input = unsafe { CStr::from_ptr(pointer) }.to_bytes_with_nul();
    // SAFETY: extent just established; copied before caller can release it.
    unsafe { bytes(input.as_ptr(), input.len(), 0) }
}
unsafe fn punctuation(pointer: *const Wide) -> Option<Vec<Wide>> {
    if pointer.is_null() {
        return None;
    }
    let mut length = 0usize;
    loop {
        length = length.checked_add(1)?;
        if length > isize::MAX as usize / size_of::<Wide>() {
            return None;
        }
        // SAFETY: caller supplies an aligned wide string through its NUL.
        if unsafe { *pointer.add(length - 1) } == 0 {
            break;
        }
    }
    let mut owned = Vec::new();
    owned.try_reserve_exact(length).ok()?;
    owned.resize(length, 0);
    // SAFETY: initialized extent through NUL established above; new buffer
    // is disjoint, properly aligned and large enough for all wide units.
    unsafe {
        ptr::copy_nonoverlapping(pointer, owned.as_mut_ptr(), length);
    }
    Some(owned)
}

unsafe fn create(view: &View, length: usize) -> Option<Box<Command>> {
    // SAFETY: the admitted kind selects its initialized C union member.
    // Pointer payloads are copied under the caller's live-buffer contract.
    let data = unsafe {
        match view.kind {
            0 => {
                if length == 0 {
                    return None;
                }
                let args = view.data.text;
                let text = text(args.text, length)?;
                Data::Text(args, text)
            }
            1 => {
                if length == 0 {
                    return None;
                }
                let args = view.data.mark;
                if args.mark.is_null() {
                    return None;
                }
                let text = text(args.text, length)?;
                let mark = string(args.mark)?;
                Data::Mark(args, text, mark)
            }
            2 => {
                let args = view.data.key;
                let name = string(args.name)?;
                Data::Key(args, name)
            }
            3 => {
                let args = view.data.character;
                Data::Character(args)
            }
            4 => Data::Parameter(view.data.parameter),
            5 => Data::Punctuation(punctuation(view.data.punctuation)?),
            6 => Data::VoiceName(string(view.data.name)?),
            7 => {
                let args = view.data.voice;
                let mut strings = [None, None, None];
                for (out, pointer) in
                    strings
                        .iter_mut()
                        .zip([args.name, args.languages, args.identifier])
                {
                    if !pointer.is_null() {
                        *out = Some(string(pointer)?);
                    }
                }
                Data::Voice(args, strings)
            }
            8 => Data::Terminated(view.data.terminated),
            _ => return None,
        }
    };
    let mut owner = Command::new(data)?;
    owner.assign_id(&IDENTIFIER);
    Some(owner)
}

/// Caller supplies a header/active union matching espeak_command.h. Pointer
/// fields are live for the call; length is the readable byte extent of text.
#[no_mangle]
unsafe extern "C" fn espeak_rs_async_command_create(
    view: *const View,
    length: usize,
) -> *mut Command {
    if view.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: admitted initialized view with kind selecting the active union.
    unsafe { create(&*view, length) }.map_or(ptr::null_mut(), Box::into_raw)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_async_command_process(
    owner: *mut Command,
    callbacks: *const Callbacks,
) {
    if owner.is_null() || callbacks.is_null() {
        return;
    }
    // SAFETY: unique live Rust command from create. Mutation finishes before
    // entering host code; only the disjoint payload remains borrowed, so C
    // can inspect the prefix. Callbacks cannot invalidate/mutate the payload.
    let data = unsafe { (&mut *owner).prepare_process() };
    // SAFETY: live admitted callback table, serialized by the engine/queue.
    Engine(unsafe { &*callbacks }).process(data);
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_async_command_delete(
    owner: *mut Command,
    callbacks: *const Callbacks,
) -> i32 {
    if owner.is_null() || callbacks.is_null() {
        return 0;
    }
    // SAFETY: unique command returned by create, relinquished exactly once.
    // Copy notification/state first, ending exclusive borrows before host
    // code inspects the prefix; reclaim the Box only after the callback.
    let completion = unsafe { (&mut *owner).take_completion() };
    if let Some(completion) = completion {
        // SAFETY: live admitted callback table and user data through disposal.
        Engine(unsafe { &*callbacks }).terminated(completion);
    }
    // SAFETY: callback could inspect the prefix but did not invalidate owner;
    // exclusive ownership is now returned to its original Rust allocation.
    unsafe {
        drop(Box::from_raw(owner));
    }
    1
}
