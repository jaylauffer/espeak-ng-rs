//! Serialized text-control ABI; callbacks retain live owners, never Rust loans.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    text_control::{self, Field, Host, ResultValue},
    translate_word::Error,
};
use std::ffi::c_void;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) store: Option<unsafe extern "C" fn(*mut c_void, u32, i32) -> i32>,
    pub(super) byte: Option<unsafe extern "C" fn(*mut c_void, u32, usize) -> i32>,
    pub(super) write: Option<unsafe extern "C" fn(*mut c_void, u32, usize, u8) -> i32>,
    pub(super) command: Option<unsafe extern "C" fn(*mut c_void, i32, *mut u32) -> i32>,
    pub(super) put_command: Option<unsafe extern "C" fn(*mut c_void, i32, u32) -> i32>,
    pub(super) default: Option<unsafe extern "C" fn(*mut c_void, usize, *mut i32) -> i32>,
    pub(super) alpha: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) lower: Option<unsafe extern "C" fn(*mut c_void, u8) -> u8>,
    pub(super) row: Option<unsafe extern "C" fn(*mut c_void, usize, *mut u32) -> i32>,
    pub(super) put_row: Option<unsafe extern "C" fn(*mut c_void, usize, u32) -> i32>,
    pub(super) translate:
        Option<unsafe extern "C" fn(*mut c_void, u32, usize, usize, usize, *mut u32) -> i32>,
    pub(super) vowel: Option<unsafe extern "C" fn(*mut c_void, u8, *mut i32) -> i32>,
    pub(super) lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut i32) -> i32>,
    pub(super) select: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    pub(super) init: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
}
fn status(v: i32) -> Result<(), Error> {
    if v == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
struct Engine<'a>(&'a Callbacks);
impl Host for Engine<'_> {
    fn value(&self, f: Field) -> i32 {
        // SAFETY: validated serialized callback projects a scalar, no loan.
        unsafe { self.0.value.unwrap()(self.0.context, f as u32) }
    }
    fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
        // SAFETY: checked scalar write to a retained serialized owner.
        status(unsafe { self.0.store.unwrap()(self.0.context, f as u32, v) })
    }
    fn byte(&self, k: u32, i: usize) -> Option<u8> {
        // SAFETY: numeric callback checks the current source owner extent.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, k, i) }).ok()
    }
    fn write(&mut self, k: u32, i: usize, v: u8) -> Result<(), Error> {
        // SAFETY: checked byte write; no foreign source loan crosses effects.
        status(unsafe { self.0.write.unwrap()(self.0.context, k, i, v) })
    }
    fn command(&self, i: i32) -> Result<u32, Error> {
        let mut v = 0;
        // SAFETY: checked command index and disjoint initialized result slot.
        status(unsafe { self.0.command.unwrap()(self.0.context, i, &mut v) })?;
        Ok(v)
    }
    fn put_command(&mut self, i: i32, v: u32) -> Result<(), Error> {
        // SAFETY: numeric index and scalar value, checked by retained owner.
        status(unsafe { self.0.put_command.unwrap()(self.0.context, i, v) })
    }
    fn default(&self, i: usize) -> Result<i32, Error> {
        let mut v = 0;
        // SAFETY: bounded command-default lookup and initialized result.
        status(unsafe { self.0.default.unwrap()(self.0.context, i, &mut v) })?;
        Ok(v)
    }
    fn alpha(&self, c: u32) -> bool {
        // SAFETY: serialized scalar locale classification retains no loan.
        unsafe { self.0.alpha.unwrap()(self.0.context, c) != 0 }
    }
    fn lower(&self, c: u8) -> u8 {
        // SAFETY: locale byte-lower primitive receives initialized 0..255.
        unsafe { self.0.lower.unwrap()(self.0.context, c) }
    }
    fn row(&self, i: usize) -> Result<u32, Error> {
        let mut v = 0;
        // SAFETY: checked live row index and initialized disjoint result.
        status(unsafe { self.0.row.unwrap()(self.0.context, i, &mut v) })?;
        Ok(v)
    }
    fn put_row(&mut self, i: usize, v: u32) -> Result<(), Error> {
        // SAFETY: checked row index and scalar flag write.
        status(unsafe { self.0.put_row.unwrap()(self.0.context, i, v) })
    }
    fn translate(
        &mut self,
        main: bool,
        offset: usize,
        row: usize,
        remaining: usize,
    ) -> Result<u32, Error> {
        let mut v = 0;
        // SAFETY: checked numeric offsets/rows into retained owners; primitive
        // scopes and restores nested rule contexts and retains no Rust scratch.
        status(unsafe {
            self.0.translate.unwrap()(self.0.context, main as u32, offset, row, remaining, &mut v)
        })?;
        Ok(v)
    }
    fn vowel(&self, c: u8) -> Result<bool, Error> {
        let mut v = 0;
        // SAFETY: live bounded phoneme-table lookup, initialized scalar result.
        status(unsafe { self.0.vowel.unwrap()(self.0.context, c, &mut v) })?;
        Ok(v != 0)
    }
    fn lookup(&mut self, name: &[u8; 12]) -> Result<i32, Error> {
        let mut v = 0;
        // SAFETY: initialized terminated owned name and disjoint result;
        // lookup retains no pointer into Rust scratch after returning.
        status(unsafe { self.0.lookup.unwrap()(self.0.context, name.as_ptr(), &mut v) })?;
        Ok(v)
    }
    fn select(&mut self, v: i32) -> Result<(), Error> {
        // SAFETY: serialized scalar table-selection primitive.
        status(unsafe { self.0.select.unwrap()(self.0.context, v) })
    }
    fn init(&mut self, k: u32) -> Result<(), Error> {
        // SAFETY: serialized initialization primitive, no foreign loan.
        status(unsafe { self.0.init.unwrap()(self.0.context, k) })
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_text_control(
    table: *const Callbacks,
    operation: u32,
    argument: usize,
    out: *mut ResultValue,
) -> i32 {
    if table.is_null() || out.is_null() {
        return -1;
    }
    // SAFETY: aligned immutable table retained and disjoint from mutable owners,
    // live flag publication slots and output for this serialized entry point.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.value.is_none()
        || t.store.is_none()
        || t.byte.is_none()
        || t.write.is_none()
        || t.command.is_none()
        || t.put_command.is_none()
        || t.default.is_none()
        || t.alpha.is_none()
        || t.lower.is_none()
        || t.row.is_none()
        || t.put_row.is_none()
        || t.translate.is_none()
        || t.vowel.is_none()
        || t.lookup.is_none()
        || t.select.is_none()
        || t.init.is_none()
    {
        return -1;
    }
    let mut h = Engine(t);
    let result = match operation {
        0 => text_control::embedded(&mut h, argument),
        1 => text_control::word_embedded(&mut h).map(|()| ResultValue { value: 0, index: 0 }),
        2 => text_control::combine(&mut h).map(|()| ResultValue {
            value: h.value(Field::CurrentFlags) as u32,
            index: 0,
        }),
        3 => text_control::switch_language(&mut h).map(|()| ResultValue { value: 0, index: 0 }),
        4 => u32::try_from(argument)
            .map_err(|_| Error::State)
            .and_then(|v| text_control::init_text(&mut h, v))
            .map(|()| ResultValue { value: 0, index: 0 }),
        5 => text_control::syllables(&h).map(|value| ResultValue { value, index: 0 }),
        _ => Err(Error::State),
    };
    match result {
        Ok(value) => {
            // SAFETY: aligned disjoint output stays retained by the caller.
            unsafe { out.write(value) };
            0
        }
        Err(_) => -1,
    }
}
