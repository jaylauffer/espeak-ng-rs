//! Serialized Roman primitives and owned synthetic-number source projection.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::number_frontend_compat::{Callbacks as Frontend, Engine as FrontEngine};
use crate::number_frontend::{self, Host as FrontHost};
use crate::number_lookup::{Error, PHONEME_BYTES};
use crate::number_ordinal::Host as OrdinalHost;
use crate::number_roman::{self, Host};
use std::ffi::c_void;

#[repr(C)]
struct Callbacks {
    frontend: *const Frontend,
    range: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    suffix: Option<unsafe extern "C" fn(*mut c_void) -> *const u8>,
    word: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    clear: Option<unsafe extern "C" fn(*mut c_void)>,
    list: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8, *mut u32) -> i32>,
    translate: Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, usize) -> u32>,
}
struct Engine<'a> {
    front: FrontEngine<'a>,
    table: &'a Callbacks,
    remaining: usize,
}
impl OrdinalHost for Engine<'_> {
    fn byte(&self, offset: isize) -> u8 {
        self.front.byte(offset)
    }
    fn space(&mut self, offset: usize) {
        self.front.write_byte(offset, b' ');
    }
    fn value(&self, field: u32) -> u32 {
        match field {
            0 => self.front.value(0) as u32,
            1 => self.front.value(3) as u32,
            2 => self.front.word_flags(0),
            3 if self.remaining > 1 => self.front.word_flags(1),
            3 => 0,
            _ => self.front.value(8) as u32,
        }
    }
    fn alpha(&self, code: u32) -> bool {
        self.front.classify(code, 0)
    }
    fn digit(&self, code: u32) -> bool {
        self.front.classify(code, 1)
    }
    fn translate(&mut self, offset: usize) -> u32 {
        self.front.translate(offset)
    }
}
impl Host for Engine<'_> {
    fn lookup(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) {
        self.front.lookup(key, output);
    }
    fn range(&self, maximum: bool) -> i32 {
        // SAFETY: admitted live scalar bounds projection.
        unsafe { self.table.range.unwrap()(self.front.context(), u32::from(maximum)) }
    }
    fn suffix(&self, out: &mut [u8; 160]) -> Result<(), Error> {
        // SAFETY: admitted initialized CString owner, prefix read stops at NUL.
        let pointer = unsafe { self.table.suffix.unwrap()(self.front.context()) };
        if pointer.is_null() {
            return Err(Error::Phonemes);
        }
        for (i, byte) in out.iter_mut().enumerate() {
            // SAFETY: initialized prefix through terminator or admitted 160-byte bound.
            *byte = unsafe { pointer.add(i).read() };
            if *byte == 0 {
                return Ok(());
            }
        }
        Err(Error::Phonemes)
    }
    fn store_word_flags(&mut self, flags: u32) {
        // SAFETY: admitted serialized initialized current word row.
        unsafe { self.table.word.unwrap()(self.front.context(), flags) }
    }
    fn clear_previous(&mut self) {
        // SAFETY: admitted serialized previous dictionary flags reset.
        unsafe { self.table.clear.unwrap()(self.front.context()) }
    }
    fn number(
        &mut self,
        source: &mut [u8; 160],
        initialized: usize,
        remaining: usize,
        control: i32,
        output: &mut [u8],
    ) -> Result<(), Error> {
        let mut flags = [0; 2];
        number_frontend::translate(
            &mut Synthetic {
                engine: self,
                source,
                initialized,
            },
            remaining,
            control,
            output,
            &mut flags,
        )?;
        Ok(())
    }
}
struct Synthetic<'a, 'b> {
    engine: &'a mut Engine<'b>,
    source: &'a mut [u8; 160],
    initialized: usize,
}
impl FrontHost for Synthetic<'_, '_> {
    fn byte(&self, offset: isize) -> u8 {
        offset
            .checked_add(3)
            .and_then(|i| usize::try_from(i).ok())
            .filter(|i| *i < self.initialized)
            .map_or(0, |i| self.source[i])
    }
    fn write_byte(&mut self, offset: usize, byte: u8) {
        if let Some(index) = offset.checked_add(3).filter(|i| *i < self.initialized) {
            self.source[index] = byte;
        }
    }
    fn value(&self, field: u32) -> i32 {
        self.engine.front.value(field)
    }
    fn word_flags(&self, index: usize) -> u32 {
        self.engine.front.word_flags(index)
    }
    fn lookup(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
        self.engine.front.lookup(key, out)
    }
    fn list(&mut self, offset: isize, out: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2]) -> i32 {
        let Some(index) = offset
            .checked_add(3)
            .and_then(|i| usize::try_from(i).ok())
            .filter(|i| *i < self.initialized)
        else {
            return 0;
        };
        // SAFETY: initialized owned source prefix, scratch and flags are disjoint;
        // callback reads/mutates source only through this derived raw pointer,
        // retaining no pointer or loan after return. No source read spans the call.
        unsafe {
            self.engine.table.list.unwrap()(
                self.engine.front.context(),
                self.source.as_mut_ptr().add(index),
                out.as_mut_ptr(),
                flags.as_mut_ptr(),
            )
        }
    }
    fn text(&self, kind: u32, out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        self.engine.front.text(kind, out)
    }
    fn store_text(&mut self, kind: u32, text: &[u8]) -> Result<(), Error> {
        self.engine.front.store_text(kind, text)
    }
    fn classify(&self, code: u32, kind: u32) -> bool {
        self.engine.front.classify(code, kind)
    }
    fn translate(&mut self, offset: usize) -> u32 {
        let Some(index) = offset.checked_add(3).filter(|i| *i < self.initialized) else {
            return 0;
        };
        // SAFETY: initialized owned source extent, callback scopes its rule context
        // and retains no pointer/loan. No Rust source access spans translation.
        unsafe {
            self.engine.table.translate.unwrap()(
                self.engine.front.context(),
                self.source.as_mut_ptr(),
                self.initialized,
                index,
            )
        }
    }
    fn missing(&mut self, value: i32) {
        self.engine.front.missing(value);
    }
    fn skip_words(&mut self, value: i32) {
        self.engine.front.skip_words(value);
    }
    fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
        self.engine.front.phoneme_type(code)
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_roman(
    table: *const Callbacks,
    length: usize,
    remaining: i32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if table.is_null()
        || output.is_null()
        || length == 0
        || length > 800
        || !(0..=300).contains(&remaining)
        || capacity == 0
        || capacity > PHONEME_BYTES
    {
        return -1;
    }
    // SAFETY: immutable table admitted live/disjoint through serialized invocation.
    let table = unsafe { &*table };
    if table.range.is_none()
        || table.suffix.is_none()
        || table.word.is_none()
        || table.clear.is_none()
        || table.list.is_none()
        || table.translate.is_none()
    {
        return -1;
    }
    // SAFETY: frontend table and primitive lifetime/bounds contracts admitted.
    let Some(front) = (unsafe { FrontEngine::admit(table.frontend) }) else {
        return -1;
    };
    let mut scratch = [0; PHONEME_BYTES];
    let Ok(outcome) = number_roman::translate(
        &mut Engine {
            front,
            table,
            remaining: remaining as usize,
        },
        remaining as usize,
        &mut scratch[..capacity],
    ) else {
        return -1;
    };
    // SAFETY: copy only initialized validated final terminated prefix.
    unsafe { std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, outcome.written + 1) };
    i32::from(outcome.recognized)
}
