//! Serialized bounded clause ABI; owned scratch never escapes a primitive.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme_list::Source as Entry,
    translate_clause::{self, Class, Field, Host, Read, Row, WordCall, SOURCE},
    translate_word::Error,
};
use std::ffi::c_void;
type WordPrimitive = unsafe extern "C" fn(
    *mut c_void,
    *mut u8,
    usize,
    *mut u8,
    usize,
    usize,
    *mut Row,
    usize,
    usize,
    i32,
    *mut u32,
) -> i32;
#[repr(C)]
pub(super) struct Callbacks {
    pub(super) context: *mut c_void,
    pub(super) value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    pub(super) store: Option<unsafe extern "C" fn(*mut c_void, u32, i32) -> i32>,
    pub(super) byte: Option<unsafe extern "C" fn(*mut c_void, usize) -> i32>,
    pub(super) write: Option<unsafe extern "C" fn(*mut c_void, usize, u8) -> i32>,
    pub(super) classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    pub(super) lower: Option<unsafe extern "C" fn(*mut c_void, u32) -> u32>,
    pub(super) read: Option<unsafe extern "C" fn(*mut c_void, *mut i16, *mut u8, *mut Read) -> i32>,
    pub(super) embedded_command:
        Option<unsafe extern "C" fn(*mut c_void, *mut usize, *mut i32) -> i32>,
    pub(super) translate_char: Option<
        unsafe extern "C" fn(
            *mut c_void,
            usize,
            u32,
            u32,
            u32,
            *mut u32,
            *mut u32,
            *mut u32,
        ) -> i32,
    >,
    pub(super) upper_word:
        Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, u32, *mut i32) -> i32>,
    pub(super) entry: Option<unsafe extern "C" fn(*mut c_void, i32, *mut Entry) -> i32>,
    pub(super) put: Option<unsafe extern "C" fn(*mut c_void, i32, *const Entry) -> i32>,
    pub(super) command: Option<unsafe extern "C" fn(*mut c_void, i32, *mut u32) -> i32>,
    pub(super) put_command: Option<unsafe extern "C" fn(*mut c_void, i32, u32) -> i32>,
    pub(super) translate: Option<WordPrimitive>,
    pub(super) word_embedded: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) eof: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) make_list: Option<unsafe extern "C" fn(*mut c_void, i32, i32) -> i32>,
    pub(super) output: Option<unsafe extern "C" fn(*mut c_void, usize, u32, u32) -> i32>,
    pub(super) result: Option<unsafe extern "C" fn(*mut c_void, u32, i32, *const u8) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
fn status(v: i32) -> Result<(), Error> {
    if v == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
impl Host for Engine<'_> {
    fn value(&self, f: Field) -> i32 {
        // SAFETY: validated callback projects a serialized scalar, no loan.
        unsafe { self.0.value.unwrap()(self.0.context, f as u32) }
    }
    fn store(&mut self, f: Field, v: i32) -> Result<(), Error> {
        // SAFETY: checked scalar write through the retained callback table.
        status(unsafe { self.0.store.unwrap()(self.0.context, f as u32, v) })
    }
    fn byte(&self, i: usize) -> Option<u8> {
        // SAFETY: numeric source callback checks the C owner's extent.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, i) }).ok()
    }
    fn write(&mut self, i: usize, v: u8) -> Result<(), Error> {
        // SAFETY: numeric source write checks the retained owner extent.
        status(unsafe { self.0.write.unwrap()(self.0.context, i, v) })
    }
    fn classify(&self, c: u32, k: Class) -> bool {
        // SAFETY: scalar classification retains no source/state borrow.
        unsafe { self.0.classify.unwrap()(self.0.context, c, k as u32) != 0 }
    }
    fn lower(&self, c: u32) -> u32 {
        // SAFETY: scalar case mapping through validated callback.
        unsafe { self.0.lower.unwrap()(self.0.context, c) }
    }
    fn read(&mut self, p: &mut [i16; SOURCE + 4], v: &mut [u8; 40]) -> Result<Read, Error> {
        let mut r = Read {
            terminator: 0,
            tone: 0,
            top: 0,
        };
        // SAFETY: disjoint owned initialized positions, voice and read result;
        // primitive retains none of these pointers after returning.
        status(unsafe {
            self.0.read.unwrap()(self.0.context, p.as_mut_ptr(), v.as_mut_ptr(), &mut r)
        })?;
        Ok(r)
    }
    fn embedded_command(&mut self, i: &mut usize) -> Result<i32, Error> {
        let mut n = 0;
        // SAFETY: disjoint initialized numeric offset/result; checked owner.
        status(unsafe { self.0.embedded_command.unwrap()(self.0.context, i, &mut n) })?;
        Ok(n)
    }
    fn translate_char(
        &mut self,
        i: usize,
        p: u32,
        c: u32,
        n: u32,
        insert: &mut u32,
        f: &mut u32,
    ) -> Result<u32, Error> {
        let mut result = 0;
        // SAFETY: scalar source offset and disjoint initialized in/out slots;
        // source remains C-owned, no foreign loan or pointer retained.
        status(unsafe {
            self.0.translate_char.unwrap()(self.0.context, i, p, c, n, insert, f, &mut result)
        })?;
        Ok(result)
    }
    fn upper_word(&mut self, s: &mut [u8; SOURCE], i: usize, c: u32) -> Result<bool, Error> {
        let mut r = 0;
        // SAFETY: initialized owned source, checked offset and result slot;
        // primitive scopes access to scratch and retains no pointer.
        status(unsafe {
            self.0.upper_word.unwrap()(self.0.context, s.as_mut_ptr(), i, c, &mut r)
        })?;
        Ok(r != 0)
    }
    fn entry(&self, i: i32) -> Result<Entry, Error> {
        let mut e = Entry::default();
        // SAFETY: checked index, disjoint initialized eight-byte entry.
        status(unsafe { self.0.entry.unwrap()(self.0.context, i, &mut e) })?;
        Ok(e)
    }
    fn put(&mut self, i: i32, e: Entry) -> Result<(), Error> {
        // SAFETY: initialized owned entry copied to checked live index.
        status(unsafe { self.0.put.unwrap()(self.0.context, i, &e) })
    }
    fn command(&self, i: i32) -> Result<u32, Error> {
        let mut r = 0;
        // SAFETY: checked command index and disjoint initialized result.
        status(unsafe { self.0.command.unwrap()(self.0.context, i, &mut r) })?;
        Ok(r)
    }
    fn put_command(&mut self, i: i32, v: u32) -> Result<(), Error> {
        // SAFETY: scalar command write checks the live owner index.
        status(unsafe { self.0.put_command.unwrap()(self.0.context, i, v) })
    }
    fn translate(&mut self, call: WordCall<'_>) -> Result<u32, Error> {
        let WordCall {
            clause,
            end,
            number,
            offset,
            rows,
            row,
            remaining,
            pause,
        } = call;
        let (number, length) =
            number.map_or((std::ptr::null_mut(), 0), |(n, l)| (n.as_mut_ptr(), l));
        let mut result = 0;
        // SAFETY: initialized disjoint owned source/optional number/rows and
        // result; callback checks offsets and scopes/restores both source
        // contexts. No scratch pointer or foreign Rust loan survives return.
        status(unsafe {
            self.0.translate.unwrap()(
                self.0.context,
                clause.as_mut_ptr(),
                end,
                number,
                length,
                offset,
                rows.as_mut_ptr(),
                row,
                remaining,
                pause,
                &mut result,
            )
        })?;
        Ok(result)
    }
    fn word_embedded(&mut self) -> Result<(), Error> {
        // SAFETY: serialized retained primitive owns live command state.
        status(unsafe { self.0.word_embedded.unwrap()(self.0.context) })
    }
    fn eof(&self) -> bool {
        // SAFETY: one scalar EOF query; no polling loop or retained loan.
        unsafe { self.0.eof.unwrap()(self.0.context) != 0 }
    }
    fn make_list(&mut self, p: i32, s: bool) -> Result<(), Error> {
        // SAFETY: serialized list primitive takes scalar policy results.
        status(unsafe { self.0.make_list.unwrap()(self.0.context, p, s as i32) })
    }
    fn output(&mut self, i: usize, f: u32, v: u32) -> Result<(), Error> {
        // SAFETY: numeric list field projection validates index and field.
        status(unsafe { self.0.output.unwrap()(self.0.context, i, f, v) })
    }
    fn result(&mut self, k: u32, v: i32, name: &[u8; 40]) -> Result<(), Error> {
        // SAFETY: owned initialized voice name scoped to checked publication
        // into retained C storage; callbacks retain no scratch pointer.
        status(unsafe { self.0.result.unwrap()(self.0.context, k, v, name.as_ptr()) })
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_translate_clause(table: *const Callbacks) -> i32 {
    if table.is_null() {
        return -1;
    }
    // SAFETY: aligned immutable callback table retained and disjoint from all
    // owners, output slots and owned scratch for the serialized entry point.
    let t = unsafe { &*table };
    if t.context.is_null()
        || t.value.is_none()
        || t.store.is_none()
        || t.byte.is_none()
        || t.write.is_none()
        || t.classify.is_none()
        || t.lower.is_none()
        || t.read.is_none()
        || t.embedded_command.is_none()
        || t.translate_char.is_none()
        || t.upper_word.is_none()
        || t.entry.is_none()
        || t.put.is_none()
        || t.command.is_none()
        || t.put_command.is_none()
        || t.translate.is_none()
        || t.word_embedded.is_none()
        || t.eof.is_none()
        || t.make_list.is_none()
        || t.output.is_none()
        || t.result.is_none()
    {
        return -1;
    }
    match translate_clause::translate(&mut Engine(t)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
