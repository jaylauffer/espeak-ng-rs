//! Serialized compatibility effects for character and special-name lookup.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::clause_names::{self as names, Data, Error};
use std::{
    ffi::{c_char, c_void, CStr},
    ptr,
};
#[repr(C)]
struct Command {
    kind: u32,
    secondary: u32,
    found: i32,
    data: Data,
    text: [u8; 74],
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Context {
    owner: *mut c_void,
    query: Option<unsafe extern "C" fn(*mut c_void, *mut Command) -> i32>,
    language: *const i32,
}
struct Host {
    context: Context,
    output: *mut u8,
    capacity: usize,
}
impl Host {
    fn query(&mut self, kind: u32, secondary: bool, data: &mut Data) -> Result<Command, Error> {
        let mut command = Command {
            kind,
            secondary: u32::from(secondary),
            found: 0,
            data: *data,
            text: [0; 74],
        };
        // SAFETY: retained serialized owner and initialized exclusive local
        // copied command. Callback cannot retain/invalidate/reenter its storage;
        // no foreign Rust owner/input/output borrow crosses the backend call.
        if unsafe { self.context.query.expect("admitted query")(self.context.owner, &mut command) }
            != 0
            || !command.data.valid()
        {
            return Err(Error::Backend);
        }
        *data = command.data;
        Ok(command)
    }
}
impl names::Host for Host {
    fn dictionary(&mut self, data: &mut Data, secondary: bool) -> Result<bool, Error> {
        match self.query(1, secondary, data)?.found {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Backend),
        }
    }
    fn rules(&mut self, data: &mut Data) -> Result<(), Error> {
        self.query(2, false, data).map(|_| ())
    }
    fn fallback(&mut self) -> Result<(), Error> {
        self.query(3, true, &mut Data::empty()).map(|_| ())
    }
    fn format(&mut self, data: &mut Data, secondary: bool) -> Result<[u8; 74], Error> {
        let text = self.query(4, secondary, data)?.text;
        let length = text
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::Bounds)?;
        if length >= self.capacity {
            return Err(Error::Bounds);
        }
        // SAFETY: admit/publish copied format output before table restoration,
        // as the legacy callback order requires. Source is initialized local
        // storage, disjoint from retained exclusive writable caller capacity.
        unsafe { ptr::copy_nonoverlapping(text.as_ptr(), self.output, length + 1) };
        Ok(text)
    }
    fn restore_table(&mut self) -> Result<(), Error> {
        self.query(5, false, &mut Data::empty()).map(|_| ())
    }
    fn language(&self) -> u32 {
        // SAFETY: initialized live owner metadata scalar read after callbacks;
        // no reference is retained across any subsequent backend mutation.
        unsafe { *self.context.language as u32 }
    }
}
unsafe fn host(context: *const Context, output: *mut u8, capacity: usize) -> Option<Host> {
    if context.is_null() {
        return None;
    }
    // SAFETY: initialized immutable context copied before backend callbacks.
    let context = unsafe { *context };
    (context.query.is_some() && !context.language.is_null()).then_some(Host {
        context,
        output,
        capacity,
    })
}
unsafe fn publish(text: &names::Text, output: *mut u8, capacity: usize) -> i32 {
    if text.length >= capacity {
        return -2;
    }
    // SAFETY: complete prefix+NUL admitted, local source disjoint from caller's
    // exclusive writable output. No possibly undefined unused tail is borrowed.
    unsafe { ptr::copy_nonoverlapping(text.bytes.as_ptr(), output, text.length + 1) };
    text.length as i32
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_character_name(
    context: *const Context,
    code: i32,
    only: u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if output.is_null() || only > 1 || capacity == 0 {
        return -2;
    }
    // SAFETY: caller retains serialized context/owner/resources and disjoint
    // output through every synchronous copied backend effect.
    let Some(mut host) = (unsafe { host(context, output, capacity) }) else {
        return -2;
    };
    // SAFETY: admit/publish the legacy initial empty string before any backend
    // effect. Later rejection can retain this earlier one-byte output effect.
    unsafe { *output = 0 };
    let Ok(text) = names::character(code, only != 0, &mut host) else {
        return -2;
    };
    // SAFETY: same retained exclusive output after all callbacks return.
    unsafe { publish(&text, output, capacity) }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_special(
    context: *const Context,
    word: *const c_char,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if word.is_null() || output.is_null() {
        return -2;
    }
    // SAFETY: immutable initialized terminated special name. Copy into owned
    // command before any backend effect; do not retain a foreign string borrow.
    let mut copied = [0; 160];
    let length = {
        // SAFETY: same initialized source prefix, used only before callbacks.
        let word = unsafe { CStr::from_ptr(word) }.to_bytes();
        if word.is_empty() || word.len() >= copied.len() {
            return -2;
        }
        copied[..word.len()].copy_from_slice(word);
        word.len()
    };
    // SAFETY: initialized serialized owner context retained through callbacks.
    let Some(mut host) = (unsafe { host(context, output, capacity) }) else {
        return -2;
    };
    match names::special(&copied[..length], &mut host) {
        Ok(Some(text)) => {
            // SAFETY: same disjoint exclusive output after callbacks return.
            unsafe { publish(&text, output, capacity) }
        }
        Ok(None) => -1,
        Err(_) => -2,
    }
}
