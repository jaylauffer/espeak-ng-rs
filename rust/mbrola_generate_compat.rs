//! Serialized legacy shell for native MBROLA generation.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::generate::{Entry, FmtParams, MAX_ENTRIES};
use crate::mbrola::Selection;
use crate::mbrola_generate::{Admission, Error, Generator, Host, Settings, Text, TEXT_CAPACITY};
use crate::phoneme_program::PhonemeData;
use std::{ffi::c_void, ptr, slice};

#[repr(C)]
pub struct Effect {
    op: i32,
    index: i32,
    a: i32,
    b: i32,
    c: i32,
    text: *mut u8,
    capacity: usize,
    settings: *mut Settings,
    selection: *mut Selection,
    data: *mut PhonemeData,
    fmt: *mut FmtParams,
    cursor: *mut i32,
}
type Callback = unsafe extern "C" fn(*mut c_void, *mut Effect) -> i32;
struct Callbacks {
    context: *mut c_void,
    callback: Callback,
}
impl Callbacks {
    fn effect(op: i32, index: usize, a: i32, b: i32, c: i32) -> Effect {
        Effect {
            op,
            index: index as i32,
            a,
            b,
            c,
            text: ptr::null_mut(),
            capacity: 0,
            settings: ptr::null_mut(),
            selection: ptr::null_mut(),
            data: ptr::null_mut(),
            fmt: ptr::null_mut(),
            cursor: ptr::null_mut(),
        }
    }
    fn run(&mut self, mut effect: Effect) -> i32 {
        // SAFETY: serialized callback; every effect pointer is disjoint local
        // storage retained for this synchronous call. It may not reenter us.
        unsafe { (self.callback)(self.context, &mut effect) }
    }
    fn call(&mut self, op: i32, ix: usize, a: i32, b: i32, c: i32) -> i32 {
        self.run(Self::effect(op, ix, a, b, c))
    }
}
fn admission(result: i32) -> Admission {
    match result {
        n if n > 0 => Admission::Accepted,
        0 => Admission::Pending,
        _ => Admission::Failed,
    }
}
impl Host for Callbacks {
    fn free(&mut self) -> i32 {
        self.call(0, 0, 0, 0, 0)
    }
    fn settings(&mut self) -> Settings {
        let mut settings = Settings::default();
        let mut e = Self::effect(1, 0, 0, 0, 0);
        e.settings = &mut settings;
        self.run(e);
        settings
    }
    fn embedded(&mut self, cursor: &mut i32, source: i32) {
        let mut e = Self::effect(2, 0, source, 0, 0);
        e.cursor = cursor;
        self.run(e);
    }
    fn marker(&mut self, kind: i32, position: i32, length: i32, value: i32) {
        self.call(3, kind as usize, position, length, value);
    }
    fn phoneme_marker(&mut self, ix: usize, ipa: bool, position: i32) {
        self.call(4, ix, i32::from(ipa), position, 0);
    }
    fn select(&mut self, ix: usize) -> Selection {
        let mut selection = Selection::default();
        let mut e = Self::effect(5, ix, 0, 0, 0);
        e.selection = &mut selection;
        self.run(e);
        selection
    }
    fn pitch(&mut self, ix: usize, split: i32, final_only: bool) -> Result<Text, Error> {
        let mut output = [0; TEXT_CAPACITY];
        let mut e = Self::effect(6, ix, split, i32::from(final_only), 0);
        e.text = output.as_mut_ptr();
        e.capacity = output.len();
        let n = self.run(e);
        let n = usize::try_from(n).map_err(|_| Error::Host)?;
        if n >= output.len() {
            return Err(Error::Capacity);
        }
        let mut text = Text::default();
        text.append(&output[..n])?;
        Ok(text)
    }
    fn set_flags(&mut self, ix: usize, flags: u16) {
        self.call(7, ix, i32::from(flags), 0, 0);
    }
    fn interpret(&mut self, ix: usize) -> PhonemeData {
        let mut data = PhonemeData::default();
        let mut e = Self::effect(8, ix, 0, 0, 0);
        e.data = &mut data;
        self.run(e);
        data
    }
    fn sample(&mut self, data: &mut PhonemeData, length: i32) -> i32 {
        let mut e = Self::effect(9, 0, length, 0, 0);
        e.data = data;
        self.run(e)
    }
    fn spect(&mut self, ix: usize, fmt: &mut FmtParams) -> i32 {
        let mut e = Self::effect(10, ix, 0, 0, 0);
        e.fmt = fmt;
        self.run(e)
    }
    fn pause_length(&mut self, length: i32, control: i32) -> i32 {
        self.call(11, 0, length, control, 0)
    }
    fn submit(&mut self, text: &Text, offset: usize, file: bool) -> Admission {
        let mut e = Self::effect(12, 0, i32::from(file), 0, 0);
        // Callback may read this NUL-terminated text but never write it.
        e.text = text.terminated()[offset..].as_ptr().cast_mut();
        let remaining = text.bytes().len() - offset;
        e.capacity = remaining;
        match self.run(e) {
            n if n > 0 && n as usize == remaining => Admission::Accepted,
            n if n > 0 => Admission::Progress(n as usize),
            0 => Admission::Pending,
            _ => Admission::Failed,
        }
    }
    fn queue_audio(&mut self, milliseconds: i32) {
        self.call(13, 0, milliseconds, 0, 0);
    }
    fn drain_audio(&mut self) {
        self.call(15, 0, 0, 0, 0);
    }
    fn flush(&mut self) -> Admission {
        admission(self.call(14, 0, 0, 0, 0))
    }
}

#[no_mangle]
extern "C" fn espeak_rs_mbrola_generator_create() -> *mut Generator {
    Box::into_raw(Box::default())
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_generator_destroy(owner: *mut Generator) {
    if !owner.is_null() {
        // SAFETY: unique live allocation from create; no call/read is active.
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_generate(
    owner: *mut Generator,
    entries: *mut Entry,
    length: usize,
    count: usize,
    resume: u32,
    file: u32,
    context: *mut c_void,
    callback: Option<Callback>,
) -> i32 {
    let Some(callback) = callback else {
        return 2;
    };
    if owner.is_null()
        || entries.is_null()
        || length > MAX_ENTRIES
        || count > length
        || resume > 1
        || file > 1
    {
        return 2;
    }
    // SAFETY: unique live generator and initialized disjoint snapshot entries.
    // Callback retains the original C list, never this snapshot/owner, and
    // may not reenter the generator or invalidate its borrowed storage.
    let (owner, entries) = unsafe { (&mut *owner, slice::from_raw_parts_mut(entries, length)) };
    let mut host = Callbacks { context, callback };
    match owner.translate(entries, count, resume != 0, file != 0, &mut host) {
        Ok(false) => 0,
        Ok(true) => 1,
        Err(e) => 3 + e as i32,
    }
}
