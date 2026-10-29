//! Synchronous native compiler projections; output FILE ownership is Rust.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    dictionary_compile::{self, Host},
    engine_io::Writer,
};
use std::ffi::{c_char, c_int, c_void};
type Compile = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void) -> c_int;
type Log = unsafe extern "C" fn(*mut c_void, c_int, *const c_char);
type Reload = unsafe extern "C" fn(*mut c_void, *const c_char);
type Errors = unsafe extern "C" fn(*mut c_void) -> c_int;
type Error = unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int;
#[repr(C)]
pub struct Descriptor {
    pub(super) context: *mut c_void,
    pub(super) source: *const c_char,
    pub(super) name: *const c_char,
    pub(super) home: *const c_char,
    pub(super) table: *const c_char,
    pub(super) path_width: usize,
    pub(super) separator: c_int,
    pub(super) lists: Option<Compile>,
    pub(super) rules: Option<Compile>,
    pub(super) log: Option<Log>,
    pub(super) reload: Option<Reload>,
    pub(super) errors: Option<Errors>,
    pub(super) error: Option<Error>,
}
struct Effects {
    d: Descriptor,
}
impl Host for Effects {
    fn lists(&mut self, path: &[u8], writer: &mut Writer) -> i32 {
        // SAFETY: owned terminated path and live exclusive writer, retained
        // only for this synchronous nested compiler call.
        unsafe {
            (self.d.lists.unwrap())(
                self.d.context,
                path.as_ptr().cast(),
                (writer as *mut Writer).cast(),
            )
        }
    }
    fn rules(&mut self, path: &[u8], writer: &mut Writer) -> i32 {
        // SAFETY: owned terminated path and exclusive writer; no retained loan.
        unsafe {
            (self.d.rules.unwrap())(
                self.d.context,
                path.as_ptr().cast(),
                (writer as *mut Writer).cast(),
            )
        }
    }
    fn log(&mut self, kind: i32, text: &[u8]) {
        // SAFETY: initialized terminated text for the synchronous log effect.
        unsafe { (self.d.log.unwrap())(self.d.context, kind, text.as_ptr().cast()) };
    }
    fn reload(&mut self, name: &[u8]) {
        // SAFETY: owned terminated name, disjoint from native global storage.
        unsafe { (self.d.reload.unwrap())(self.d.context, name.as_ptr().cast()) };
    }
    fn errors(&mut self) -> i32 {
        // SAFETY: live serialized caller context, scalar projection only.
        unsafe { (self.d.errors.unwrap())(self.d.context) }
    }
    fn error(&mut self, status: i32, path: &[u8]) -> i32 {
        // SAFETY: owned terminated path for the synchronous context allocator.
        unsafe { (self.d.error.unwrap())(self.d.context, status, path.as_ptr().cast()) }
    }
}
unsafe fn snapshot(pointer: *const c_char) -> Option<[u8; 4096]> {
    if pointer.is_null() {
        return Some([0; 4096]);
    }
    let mut out = [0; 4096];
    for (index, byte) in out.iter_mut().enumerate() {
        // SAFETY: caller's initialized string through NUL within 4096, or all
        // 4096 readable bytes for a rejected unterminated input.
        *byte = unsafe { pointer.add(index).cast::<u8>().read() };
        if *byte == 0 {
            return Some(out);
        }
    }
    None
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_dictionary(descriptor: *const Descriptor) -> c_int {
    if descriptor.is_null() {
        return dictionary_compile::FAILED;
    }
    // SAFETY: caller supplies a fully initialized descriptor.
    let d = unsafe { descriptor.read() };
    if d.lists.is_none()
        || d.rules.is_none()
        || d.log.is_none()
        || d.reload.is_none()
        || d.errors.is_none()
        || d.error.is_none()
    {
        return dictionary_compile::FAILED;
    }
    // SAFETY: all initialized source/name/home/table strings are copied before
    // any callback or filesystem effect. No foreign text loan survives.
    let (Some(source), Some(name), Some(home), Some(table)) = (
        unsafe { snapshot(d.source) },
        unsafe { snapshot(d.name) },
        unsafe { snapshot(d.home) },
        unsafe { snapshot(d.table) },
    ) else {
        return dictionary_compile::FAILED;
    };
    let string = |bytes: &[u8; 4096]| bytes.iter().position(|b| *b == 0).unwrap();
    let env = dictionary_compile::Environment {
        source: &source[..string(&source)],
        name: &name[..string(&name)],
        home: &home[..string(&home)],
        table: &table[..string(&table)],
        path_width: d.path_width,
        separator: d.separator as u8,
    };
    dictionary_compile::compile(&mut Effects { d }, &env)
}
/// Native compiler callbacks receive this exclusively owned live writer token.
/// Scratch is initialized, disjoint, and borrowed only for this synchronous call.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_write(
    writer: *mut Writer,
    bytes: *const u8,
    length: usize,
) -> c_int {
    if writer.is_null() || bytes.is_null() || length > crate::rule_file::MAX_BYTES {
        return 0;
    }
    // SAFETY: callback's live exclusive writer and initialized byte extent.
    let (writer, bytes) = unsafe { (&mut *writer, std::slice::from_raw_parts(bytes, length)) };
    i32::from(writer.write(bytes).is_ok())
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_tell(
    writer: *mut Writer,
    position: *mut u64,
) -> c_int {
    if writer.is_null() || position.is_null() {
        return 0;
    }
    // SAFETY: live exclusive writer token and disjoint writable scalar result.
    unsafe { *position = (*writer).position() };
    1
}
