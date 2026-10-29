//! Serialized C admission to Rust-owned typed translator allocations.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    dictionary_storage::{Cache, Error},
    translator::Translator,
};
use std::{
    ffi::{c_char, CStr},
    ptr,
};
#[no_mangle]
unsafe extern "C" fn espeak_rs_translator_create(name: *const c_char) -> *mut Translator {
    if name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: terminated immutable name remains readable for construction;
    // no host callback or loan escapes the typed, process-lifetime setup.
    let name = unsafe { CStr::from_ptr(name) }.to_bytes();
    Translator::new(name).map_or(ptr::null_mut(), Box::into_raw)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translator_destroy(owner: *mut Translator) {
    if !owner.is_null() {
        // SAFETY: caller transfers exactly one live Rust allocation returned
        // by create, after serialized calls and all dictionary views drain.
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
}
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_translator_load(
    owner: *mut Translator,
    cache: *mut Cache,
    path: *const c_char,
    name: *const c_char,
) -> i32 {
    if owner.is_null() || cache.is_null() || path.is_null() || name.is_null() {
        return 2;
    }
    let mut dictionary = [0; 40];
    // SAFETY: retained terminated names/path are readable. Name can alias the
    // owner's dictionary field: copy its meaningful prefix before I/O/effects.
    let source = unsafe { CStr::from_ptr(name) }.to_bytes();
    let length = source.len().min(39);
    dictionary[..length].copy_from_slice(&source[..length]);
    // SAFETY: path is retained terminated storage, disjoint from cache and owner.
    let path = unsafe { CStr::from_ptr(path) }.to_bytes();
    let result = crate::voice_storage::compat_path(path)
        .map_err(Error::Io)
        .and_then(|path| {
            // SAFETY: cache is uniquely admitted for setup/worker I/O, with no
            // reentry. No Rust translator borrow is held while the proactor runs.
            unsafe { (&mut *cache).load(&path) }
        });
    match result {
        Ok(snapshot) => {
            // SAFETY: complete typed Rust owner exclusively admitted after I/O,
            // no callback occurs during rebinding; projections drain beforehand.
            unsafe { (&mut *owner).bind(snapshot, &dictionary[..length]) }.map_or_else(
                |e| {
                    if e.kind() == std::io::ErrorKind::OutOfMemory {
                        3
                    } else {
                        2
                    }
                },
                |()| 0,
            )
        }
        Err(Error::Empty) => 1,
        Err(Error::Invalid(_)) => 2,
        Err(Error::Io(e)) => match e.kind() {
            std::io::ErrorKind::OutOfMemory => 3,
            std::io::ErrorKind::InvalidData | std::io::ErrorKind::WouldBlock => 4,
            _ => 1,
        },
    }
}

#[no_mangle]
extern "C" fn espeak_rs_translator_layout(kind: u32, index: u32) -> usize {
    crate::translator::layout(kind, index as usize)
}
