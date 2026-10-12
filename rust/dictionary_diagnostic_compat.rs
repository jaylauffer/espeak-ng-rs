//! Copied dictionary diagnostic loans and serialized borrowed FILE publication.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::dictionary_diagnostic::{self, Family, Route};
use std::{
    ffi::{c_char, c_int, c_void},
    sync::atomic::{AtomicBool, Ordering},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub(super) log: *mut c_void,
    pub(super) error: *mut c_void,
    pub(super) line: *mut c_int,
    pub(super) count: *mut c_int,
    pub(super) group: *mut u8,
    pub(super) width: usize,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 48);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 24);
unsafe fn string(pointer: *const c_char) -> Option<([u8; 4096], usize)> {
    let mut bytes = [0; 4096];
    if pointer.is_null() {
        return None;
    }
    for (i, byte) in bytes.iter_mut().enumerate() {
        // SAFETY: initialized string through NUL, full 4096 bytes on rejection.
        *byte = unsafe { pointer.add(i).read() as u8 };
        if *byte == 0 {
            return Some((bytes, i));
        }
    }
    None
}
static BUSY: AtomicBool = AtomicBool::new(false);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
/// Descriptor/selected strings copy before publication; raw native fields and
/// borrowed FILEs remain live/exclusive. File family supplies width group bytes.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_diagnostic(
    pointer: *const Descriptor,
    family: c_int,
    line: c_int,
    kind: c_int,
    value: c_int,
    first: *const c_char,
    second: *const c_char,
    group: *const u8,
) -> c_int {
    if pointer.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: initialized aligned descriptor copied before any publication.
    let d = unsafe { pointer.read() };
    let family = match family {
        0 => Family::List,
        1 => Family::Rule,
        2 => Family::File,
        3 => Family::Driver,
        _ => return libc::EINVAL,
    };
    let group_needed =
        matches!(family, Family::File) || matches!(family, Family::Rule) && kind == 8;
    if (!matches!(family, Family::Driver) && d.count.is_null())
        || (matches!(family, Family::Rule | Family::File) && d.line.is_null())
        || (group_needed && (d.group.is_null() || !(2..=256).contains(&d.width)))
    {
        return libc::EINVAL;
    }
    let first_needed = match family {
        Family::List => matches!(kind, 1 | 5 | 9),
        Family::Rule => matches!(kind, 7 | 8),
        Family::File => matches!(kind, 7 | 8 | 18),
        Family::Driver => matches!(kind, 1 | 2),
    };
    let first = if first_needed {
        // SAFETY: selected initialized string copied before metadata writes.
        match unsafe { string(first) } {
            Some(s) => s,
            None => return libc::EINVAL,
        }
    } else {
        ([0; 4096], 0)
    };
    let second = if matches!(family, Family::List) && kind == 5 {
        // SAFETY: selected initialized second string copied before publication.
        match unsafe { string(second) } {
            Some(s) => s,
            None => return libc::EINVAL,
        }
    } else {
        ([0; 4096], 0)
    };
    let mut selected_group = [0; 256];
    let group_length = if group_needed {
        let input = if matches!(family, Family::File) {
            group
        } else {
            d.group
        };
        if input.is_null() {
            return libc::EINVAL;
        }
        // SAFETY: initialized width group bytes; copied before aliased writes.
        unsafe { std::ptr::copy_nonoverlapping(input, selected_group.as_mut_ptr(), d.width) };
        match selected_group[..d.width].iter().position(|b| *b == 0) {
            Some(n) => n,
            None => return libc::EINVAL,
        }
    } else {
        0
    };
    // SAFETY: live quiescent native line; file/list have explicit incoming line.
    let selected_line = if matches!(family, Family::Rule) {
        // SAFETY: admitted live initialized rule line field.
        unsafe { d.line.read() }
    } else {
        line
    };
    let Some(record) = dictionary_diagnostic::format(
        family,
        selected_line,
        kind,
        value,
        &first.0[..first.1],
        &second.0[..second.1],
        &selected_group[..group_length],
    ) else {
        return libc::EINVAL;
    };
    let file = if record.route == Route::Error {
        d.error
    } else {
        d.log
    };
    if file.is_null() && (record.route == Route::Flush || !record.bytes().is_empty()) {
        return libc::EINVAL;
    }
    // SAFETY: live disjoint count field; guard legacy undefined signed overflow.
    if record.increment && unsafe { d.count.read() } == c_int::MAX {
        return libc::EINVAL;
    }
    if BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return libc::EAGAIN;
    }
    let _guard = Guard;
    if matches!(family, Family::File) {
        // SAFETY: all inputs copied; disjoint initialized native output fields.
        unsafe {
            d.line.write(line);
            std::ptr::copy_nonoverlapping(selected_group.as_ptr(), d.group, d.width);
        }
    }
    // No native reference spans FILE effects. Preserve ignored short writes,
    // flush policy and a fresh count read after any borrowed stream callback.
    if record.route == Route::Flush {
        // SAFETY: live exclusive borrowed FILE, synchronous C-runtime primitive.
        unsafe { libc::fflush(file.cast()) };
    } else if !record.bytes().is_empty() {
        // SAFETY: owned initialized bounded bytes and live borrowed FILE.
        unsafe {
            libc::fwrite(
                record.bytes().as_ptr().cast(),
                1,
                record.bytes().len(),
                file.cast(),
            )
        };
    }
    if record.increment {
        // SAFETY: fresh native count after FILE effects, no reference retained.
        let Some(count) = (unsafe { d.count.read() }).checked_add(1) else {
            return libc::EINVAL;
        };
        // SAFETY: disjoint live counter, effects complete before publication.
        unsafe { d.count.write(count) };
    }
    0
}
