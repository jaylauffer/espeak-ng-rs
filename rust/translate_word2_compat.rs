//! Bounded scalar/entry projection adapter for word-to-list control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme_list::Source as Entry,
    translate_word::Error,
    translate_word2::{self, Field, Host, Phoneme, REPLACEMENT},
};
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    store: Option<unsafe extern "C" fn(*mut c_void, u32, i32) -> i32>,
    byte: Option<unsafe extern "C" fn(*mut c_void, usize, u32) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, usize, u8, u32) -> i32>,
    entry: Option<unsafe extern "C" fn(*mut c_void, i32, *mut Entry) -> i32>,
    put: Option<unsafe extern "C" fn(*mut c_void, i32, *const Entry) -> i32>,
    command: Option<unsafe extern "C" fn(*mut c_void, i32, *mut u32) -> i32>,
    put_command: Option<unsafe extern "C" fn(*mut c_void, i32, u32) -> i32>,
    embedded: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    encode: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    translate: Option<unsafe extern "C" fn(*mut c_void, u32, u32, *mut u8, *mut u32) -> i32>,
    combine: Option<unsafe extern "C" fn(*mut c_void, *mut u32) -> i32>,
    dictionary: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    restore_dictionary: Option<unsafe extern "C" fn(*mut c_void, *const u8) -> i32>,
    alternate: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut i32) -> i32>,
    select: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    phoneme: Option<unsafe extern "C" fn(*mut c_void, u8, *mut Phoneme) -> i32>,
    alpha: Option<unsafe extern "C" fn(*mut c_void, u8) -> i32>,
    invalid: Option<unsafe extern "C" fn(*mut c_void, u8)>,
}
struct Engine<'a>(&'a Callbacks);
fn status(value: i32) -> Result<(), Error> {
    if value == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
// SAFETY for calls below: the entry point validates the immutable retained
// table. Callbacks serialize live C owners; pointers are disjoint initialized
// owned scratch scoped to one callback, with no pointer or foreign loan kept.
impl Host for Engine<'_> {
    fn value(&self, f: Field) -> i32 {
        // SAFETY: validated callback returns one live scalar, with no loan.
        unsafe { self.0.value.unwrap()(self.0.context, f as u32) }
    }
    fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
        // SAFETY: validated callback stores a checked live scalar.
        status(unsafe { self.0.store.unwrap()(self.0.context, f as u32, v) })
    }
    fn byte(&self, i: usize, p: bool) -> Option<u8> {
        // SAFETY: byte callback checks numeric offsets before dereference.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, i, p as u32) }).ok()
    }
    fn write(&mut self, i: usize, b: u8, p: bool) -> Result<(), Error> {
        // SAFETY: write callback checks numeric live-owner offsets.
        status(unsafe { self.0.write.unwrap()(self.0.context, i, b, p as u32) })
    }
    fn entry(&self, i: i32) -> Result<Entry, Error> {
        let mut e = Entry::default();
        // SAFETY: disjoint initialized eight-byte entry; checked index.
        status(unsafe { self.0.entry.unwrap()(self.0.context, i, &mut e) })?;
        Ok(e)
    }
    fn put(&mut self, i: i32, e: Entry) -> Result<(), Error> {
        // SAFETY: owned entry scoped to checked owner copy, no pointer kept.
        status(unsafe { self.0.put.unwrap()(self.0.context, i, &e) })
    }
    fn command(&self, i: i32) -> Result<u32, Error> {
        let mut v = 0;
        // SAFETY: initialized disjoint command scalar; checked index.
        status(unsafe { self.0.command.unwrap()(self.0.context, i, &mut v) })?;
        Ok(v)
    }
    fn put_command(&mut self, i: i32, v: u32) -> Result<(), Error> {
        // SAFETY: numeric command write checks its retained owner index.
        status(unsafe { self.0.put_command.unwrap()(self.0.context, i, v) })
    }
    fn embedded(&mut self) -> Result<(), Error> {
        // SAFETY: serialized primitive, no foreign Rust loan is retained.
        status(unsafe { self.0.embedded.unwrap()(self.0.context) })
    }
    fn encode(&mut self, l: bool) -> Result<(), Error> {
        // SAFETY: serialized mnemonic primitive owns its source and output.
        status(unsafe { self.0.encode.unwrap()(self.0.context, l as u32) })
    }
    fn translate(&mut self, a: bool, m: bool, r: &mut [u8; REPLACEMENT]) -> Result<u32, Error> {
        let mut f = 0;
        // SAFETY: initialized owned 163-byte replacement and disjoint flags;
        // primitive scopes its rule context and retains no scratch pointer.
        status(unsafe {
            self.0.translate.unwrap()(self.0.context, a as u32, m as u32, r.as_mut_ptr(), &mut f)
        })?;
        Ok(f)
    }
    fn combine(&mut self, mut f: u32) -> Result<u32, Error> {
        // SAFETY: disjoint owned flags scoped to serialized combine primitive.
        status(unsafe { self.0.combine.unwrap()(self.0.context, &mut f) })?;
        Ok(f)
    }
    fn dictionary(&self, out: &mut [u8; 40]) -> Result<(), Error> {
        // SAFETY: terminated initialized dictionary prefix copied into scratch.
        status(unsafe { self.0.dictionary.unwrap()(self.0.context, out.as_mut_ptr()) })
    }
    fn restore_dictionary(&mut self, v: &[u8; 40]) -> Result<(), Error> {
        // SAFETY: terminated owned scratch; checked copy keeps no pointer.
        status(unsafe { self.0.restore_dictionary.unwrap()(self.0.context, v.as_ptr()) })
    }
    fn alternate(&mut self, l: &[u8; 200]) -> Result<i32, Error> {
        let mut table = 0;
        // SAFETY: terminated owned 200-byte name and disjoint result scalar;
        // alternate setup retains no pointer into scratch.
        status(unsafe { self.0.alternate.unwrap()(self.0.context, l.as_ptr(), &mut table) })?;
        Ok(table)
    }
    fn select(&mut self, t: i32) -> Result<(), Error> {
        // SAFETY: serialized table selection, no borrowed foreign state.
        status(unsafe { self.0.select.unwrap()(self.0.context, t) })
    }
    fn phoneme(&self, c: u8) -> Option<Phoneme> {
        let mut p = Phoneme::default();
        // SAFETY: initialized owned three-scalar projection, checked code.
        if unsafe { self.0.phoneme.unwrap()(self.0.context, c, &mut p) } == 0 {
            Some(p)
        } else {
            None
        }
    }
    fn alpha(&self, c: u8) -> bool {
        // SAFETY: scalar classification through a validated callback.
        unsafe { self.0.alpha.unwrap()(self.0.context, c) != 0 }
    }
    fn invalid(&mut self, c: u8) {
        // SAFETY: serialized diagnostic takes only the code scalar.
        unsafe { self.0.invalid.unwrap()(self.0.context, c) }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_word2(
    table: *const Callbacks,
    pause: i32,
    flags: *mut u32,
) -> i32 {
    if table.is_null() || flags.is_null() {
        return -1;
    }
    // SAFETY: aligned immutable table is retained and disjoint from live owners.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.value.is_none()
        || t.store.is_none()
        || t.byte.is_none()
        || t.write.is_none()
        || t.entry.is_none()
        || t.put.is_none()
        || t.command.is_none()
        || t.put_command.is_none()
        || t.embedded.is_none()
        || t.encode.is_none()
        || t.translate.is_none()
        || t.combine.is_none()
        || t.dictionary.is_none()
        || t.restore_dictionary.is_none()
        || t.alternate.is_none()
        || t.select.is_none()
        || t.phoneme.is_none()
        || t.alpha.is_none()
        || t.invalid.is_none()
    {
        return -1;
    }
    match translate_word2::translate(&mut Engine(t), pause) {
        Ok(f) => {
            // SAFETY: aligned disjoint result slot is retained by the caller.
            unsafe { flags.write(f) };
            0
        }
        Err(_) => -1,
    }
}
