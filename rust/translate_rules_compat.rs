//! Scoped numeric source/match projections for the native rule driver.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    number_lookup::PHONEME_BYTES,
    translate_rules::{self, Error, Field, Host, Match, Store},
};
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    byte: Option<unsafe extern "C" fn(*mut c_void, isize) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, isize, u8) -> i32>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    store: Option<unsafe extern "C" fn(*mut c_void, u32, i32)>,
    locale: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    group: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> usize>,
    matched: Option<unsafe extern "C" fn(*mut c_void, usize, usize, u32, u32, *mut Match) -> i32>,
    symbol: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    letter: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u8) -> i32>,
    publish: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8) -> i32>,
    has_ending: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    append: Option<unsafe extern "C" fn(*mut c_void, *const u8) -> i32>,
    trace: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8)>,
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn byte(&self, position: isize) -> Option<u8> {
        // SAFETY: serialized checked source projection, no retained loan.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, position) }).ok()
    }
    fn write(&mut self, position: isize, byte: u8) -> Result<(), Error> {
        // SAFETY: checked exclusive byte store into the current source owner.
        if unsafe { self.0.write.unwrap()(self.0.context, position, byte) } == 0 {
            Ok(())
        } else {
            Err(Error::Source)
        }
    }
    fn value(&self, field: Field, index: u32) -> i32 {
        // SAFETY: fresh scalar/index projection, no ownership transfer.
        unsafe { self.0.value.unwrap()(self.0.context, field as u32, index) }
    }
    fn store(&mut self, field: Store, value: i32) {
        // SAFETY: serialized scalar store; no callback loan survives it.
        unsafe { self.0.store.unwrap()(self.0.context, field as u32, value) }
    }
    fn locale(&self, code: u32, digit: bool) -> bool {
        // SAFETY: host CRT wide-character classification.
        unsafe { self.0.locale.unwrap()(self.0.context, code, u32::from(digit)) != 0 }
    }
    fn group(&self, kind: u32, index: u32) -> Result<Option<usize>, Error> {
        // SAFETY: numeric dictionary-relative identifier or explicit sentinel.
        match unsafe { self.0.group.unwrap()(self.0.context, kind, index) } {
            usize::MAX => Ok(None),
            value if value == usize::MAX - 1 => Err(Error::State),
            value => Ok(Some(value)),
        }
    }
    fn match_group(
        &mut self,
        group: Option<usize>,
        width: usize,
        flags: u32,
        dictionary: u32,
        matched: &mut Match,
    ) -> Result<(), Error> {
        // SAFETY: initialized owned match contains no pointers into foreign
        // source/rules/state; callback copies only a bounded pronunciation.
        if unsafe {
            self.0.matched.unwrap()(
                self.0.context,
                group.unwrap_or(usize::MAX),
                width,
                flags,
                dictionary,
                matched,
            )
        } == 0
        {
            Ok(())
        } else {
            Err(Error::State)
        }
    }
    fn symbol(&mut self, key: &[u8; 8], output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: owned terminated key and initialized disjoint 200-byte scratch.
        if unsafe { self.0.symbol.unwrap()(self.0.context, key.as_ptr(), output.as_mut_ptr()) } == 0
        {
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn letter(&mut self, code: u32, output: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: initialized disjoint owned scratch, primitive retains no loan.
        if unsafe { self.0.letter.unwrap()(self.0.context, code, output.as_mut_ptr()) } == 0 {
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn publish(&mut self, kind: u32, output: &[u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: bounded initialized prefix published to the serialized owner.
        if unsafe { self.0.publish.unwrap()(self.0.context, kind, output.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(Error::Capacity)
        }
    }
    fn has_ending(&self) -> bool {
        // SAFETY: current ending-output presence, no foreign output borrowed.
        unsafe { self.0.has_ending.unwrap()(self.0.context) != 0 }
    }
    fn append(&mut self, addition: &[u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: owned pronunciation; native leaf uses fresh output/table/counts
        // after nested translation, retaining no borrowed state through return.
        if unsafe { self.0.append.unwrap()(self.0.context, addition.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn trace(&mut self, kind: u32, word: &[u8; 120]) {
        // SAFETY: initialized bounded terminated header, borrowed through return.
        unsafe { self.0.trace.unwrap()(self.0.context, kind, word.as_ptr()) }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_rules(
    table: *const Callbacks,
    flags: u32,
    output: *mut i32,
) -> i32 {
    if table.is_null() || output.is_null() {
        return -1;
    }
    // SAFETY: immutable table and retained serialized owner through return;
    // result is disjoint from table/owner/source and callback-owned publications.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.byte.is_none()
        || table.write.is_none()
        || table.value.is_none()
        || table.store.is_none()
        || table.locale.is_none()
        || table.group.is_none()
        || table.matched.is_none()
        || table.symbol.is_none()
        || table.letter.is_none()
        || table.publish.is_none()
        || table.has_ending.is_none()
        || table.append.is_none()
        || table.trace.is_none()
    {
        return -1;
    }
    let Ok(result) = translate_rules::translate(&mut Engine(table), flags) else {
        return -1;
    };
    // SAFETY: publish the initialized signed result after successful control.
    unsafe {
        output.write(result);
    }
    0
}
