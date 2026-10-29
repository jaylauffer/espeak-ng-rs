//! Public native intonation host: default paths, locale maps and diagnostic FILE.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::intonation_compile_compat::{self, Descriptor as Compiler};
use crate::intonation_compile::FAILED;
use std::ffi::{c_char, c_int, c_void};

type Reload = unsafe extern "C" fn(*mut c_void);
type Error = unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub(super) source: *const c_char,
    pub(super) destination: *const c_char,
    pub(super) home: *const c_char,
    pub(super) log: *mut c_void,
    pub(super) fallback_log: *mut c_void,
    pub(super) error_context: *mut c_void,
    pub(super) width: usize,
    pub(super) reload: Option<Reload>,
    pub(super) error: Option<Error>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 72);

#[derive(Clone, Copy)]
struct Admission {
    source: *const c_char,
    destination: *const c_char,
    log: *mut c_void,
}
fn admit(d: &Descriptor) -> Option<Admission> {
    let a = Admission {
        source: if d.source.is_null() { d.home } else { d.source },
        destination: if d.destination.is_null() {
            d.home
        } else {
            d.destination
        },
        log: if d.log.is_null() {
            d.fallback_log
        } else {
            d.log
        },
    };
    ((2..=4096).contains(&d.width)
        && d.reload.is_some()
        && d.error.is_some()
        && !a.source.is_null()
        && !a.destination.is_null()
        && !a.log.is_null())
    .then_some(a)
}
#[derive(Clone, Copy)]
struct Effects {
    file: *mut libc::FILE,
    error_context: *mut c_void,
    reload: Reload,
    error: Error,
}
unsafe extern "C" fn log(context: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: synchronous generic compiler callback with initialized local state;
    // copy its handle before libc, no reference spans the foreign call.
    let file = unsafe { (*context.cast::<Effects>()).file };
    // SAFETY: live exclusively used diagnostic FILE and complete initialized
    // byte span. As in C, an incomplete diagnostic write does not change status.
    unsafe { libc::fwrite(bytes.cast(), 1, length, file) };
}
unsafe extern "C" fn close(context: *mut c_void) {
    // SAFETY: generic compiler closes its live diagnostic stream at most once,
    // and performs no further diagnostic writes after this callback.
    let file = unsafe { (*context.cast::<Effects>()).file };
    // SAFETY: admitted live FILE, transferred close policy matches retained C.
    unsafe { libc::fclose(file) };
}
unsafe extern "C" fn reload(context: *mut c_void) {
    // SAFETY: copied initialized local state; no reference spans the primitive.
    let e = unsafe { context.cast::<Effects>().read() };
    // SAFETY: synchronous serialized engine reload; opaque error context remains
    // live and callback cannot retain any borrowed compiler state or unwind.
    unsafe { (e.reload)(e.error_context) };
}
unsafe extern "C" fn error(context: *mut c_void, status: c_int, path: *const c_char) -> c_int {
    // SAFETY: initialized local effects copied before primitive dispatch.
    let e = unsafe { context.cast::<Effects>().read() };
    // SAFETY: synchronous error allocation over initialized terminated owned
    // path; neither the path nor local effects may be retained by the callback.
    unsafe { (e.error)(e.error_context, status, path) }
}

/// Descriptor initialized/aligned; selected strings initialized through NUL
/// within 4096 bytes (or all 4096 on refusal). Selected FILE is live and used
/// exclusively, with retained-C close policy: normal/semantic-error completion
/// leaves it open; source/discovery/output failure closes it. Admission refusal
/// leaves it open.
/// Whole engine/locale access serialized; primitives synchronous, no retained
/// loans or unwind. Descriptor/default strings are copied before any effects/I/O.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_intonation_native(d: *const Descriptor) -> c_int {
    if d.is_null() {
        return FAILED;
    }
    // SAFETY: fully initialized descriptor copied before foreign access/effects.
    let d = unsafe { d.read() };
    let Some(a) = admit(&d) else {
        return FAILED;
    };
    let mut spaces = [0; 256];
    let mut digits = [0; 256];
    for i in 0..256 {
        // SAFETY: C locale classifiers accept every unsigned-byte value; locale
        // is stable for this serialized admission. Preserve normalized C maps.
        unsafe {
            spaces[i] = u8::from(libc::isspace(i as c_int) != 0);
            digits[i] = u8::from(libc::isdigit(i as c_int) != 0);
        }
    }
    let mut effects = Effects {
        file: a.log.cast(),
        error_context: d.error_context,
        reload: d.reload.unwrap(),
        error: d.error.unwrap(),
    };
    let compiler = Compiler {
        context: std::ptr::addr_of_mut!(effects).cast(),
        source: a.source,
        destination: a.destination,
        spaces: spaces.as_ptr(),
        digits: digits.as_ptr(),
        width: d.width,
        windows: c_int::from(cfg!(windows)),
        log: Some(log),
        close: Some(close),
        reload: Some(reload),
        error: Some(error),
    };
    // SAFETY: initialized local descriptor/maps, selected admitted C strings and
    // FILE. Generic compiler snapshots all inputs before callbacks/proactor I/O;
    // local effects remain live and no foreign reference spans an effect.
    unsafe { intonation_compile_compat::espeak_rs_compile_intonation(&compiler) }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn reload_stub(_: *mut c_void) {}
    unsafe extern "C" fn error_stub(_: *mut c_void, _: c_int, _: *const c_char) -> c_int {
        0
    }
    fn descriptor() -> Descriptor {
        Descriptor {
            source: std::ptr::null(),
            destination: std::ptr::null(),
            home: c"home".as_ptr(),
            log: std::ptr::null_mut(),
            fallback_log: std::ptr::dangling_mut(),
            error_context: std::ptr::null_mut(),
            width: 4096,
            reload: Some(reload_stub),
            error: Some(error_stub),
        }
    }
    #[test]
    fn public_defaults_and_explicit_paths_select_only_required_inputs() {
        let mut d = descriptor();
        let a = admit(&d).unwrap();
        assert_eq!(
            (a.source, a.destination, a.log),
            (d.home, d.home, d.fallback_log)
        );
        d.source = c"source".as_ptr();
        d.destination = c"destination".as_ptr();
        d.log = std::ptr::addr_of_mut!(d).cast();
        d.home = std::ptr::null();
        d.fallback_log = std::ptr::null_mut();
        let a = admit(&d).unwrap();
        assert_eq!(
            (a.source, a.destination, a.log),
            (d.source, d.destination, d.log)
        );
    }
    #[test]
    fn incomplete_native_admission_refuses_before_file_effects() {
        let mut d = descriptor();
        d.home = std::ptr::null();
        assert!(admit(&d).is_none());
        d = descriptor();
        d.fallback_log = std::ptr::null_mut();
        assert!(admit(&d).is_none());
        d = descriptor();
        for width in [0, 1, 4097, usize::MAX] {
            d.width = width;
            assert!(admit(&d).is_none());
        }
        d = descriptor();
        d.reload = None;
        assert!(admit(&d).is_none());
        d = descriptor();
        d.error = None;
        assert!(admit(&d).is_none());
    }
}
