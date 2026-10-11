//! Serialized C compiler source bridge; all data and scanner state are owned.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_source::{Classes, Sources, Token},
    voice_reader::TextMode,
};
use std::{
    ffi::{c_char, c_int, CStr},
    io,
    sync::{Mutex, MutexGuard, OnceLock},
};

pub struct Session {
    sources: MutexGuard<'static, Sources>,
}
fn pool() -> &'static Mutex<Sources> {
    static POOL: OnceLock<Mutex<Sources>> = OnceLock::new();
    POOL.get_or_init(|| {
        Mutex::new(Sources::new(Classes {
            spaces: [0; 256],
            digits: [0; 256],
        }))
    })
}
fn errno(error: &io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        return code;
    }
    match error.kind() {
        io::ErrorKind::NotFound => 2,
        io::ErrorKind::PermissionDenied => 13,
        io::ErrorKind::IsADirectory => 21,
        io::ErrorKind::NotADirectory => 20,
        io::ErrorKind::OutOfMemory => 12,
        io::ErrorKind::WouldBlock => 11,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => 22,
        _ => 5,
    }
}
/// Initialized 256-byte classifiers and disjoint writable errno result. A
/// session is exclusive, serialized, non-reentrant, used on the creating
/// thread and ends exactly once on that thread.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_sources_begin(
    spaces: *const u8,
    digits: *const u8,
    error: *mut c_int,
) -> *mut Session {
    if spaces.is_null() || digits.is_null() || error.is_null() {
        return std::ptr::null_mut();
    }
    let mut classes = Classes {
        spaces: [0; 256],
        digits: [0; 256],
    };
    // SAFETY: initialized classifiers are copied before any I/O or loan.
    unsafe {
        std::ptr::copy_nonoverlapping(spaces, classes.spaces.as_mut_ptr(), 256);
        std::ptr::copy_nonoverlapping(digits, classes.digits.as_mut_ptr(), 256);
    }
    let Ok(mut sources) = pool().try_lock() else {
        // SAFETY: disjoint writable scalar.
        unsafe {
            error.write(11);
        }
        return std::ptr::null_mut();
    };
    sources.clear();
    sources.classes = classes;
    // SAFETY: disjoint writable scalar.
    unsafe {
        error.write(0);
    }
    Box::into_raw(Box::new(Session { sources }))
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_sources_end(session: *mut Session) {
    if !session.is_null() {
        // SAFETY: the exclusive live session is consumed once; no C loans remain.
        let mut session = unsafe { Box::from_raw(session) };
        session.sources.clear();
    }
}
/// Path is initialized through NUL; error is disjoint; no borrowed path or
/// source bytes survive the synchronous call. Returns depth+1, or zero.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_open(
    session: *mut Session,
    path: *const c_char,
    depth: usize,
    windows: c_int,
    error: *mut c_int,
) -> c_int {
    if session.is_null() || path.is_null() || error.is_null() {
        return 0;
    }
    // SAFETY: terminated native path, copied before driving the engine port.
    let path = crate::voice_storage::compat_path(unsafe { CStr::from_ptr(path) }.to_bytes());
    let result = path.and_then(|path| {
        let mode = if windows == 0 {
            TextMode::Binary
        } else {
            TextMode::Windows
        };
        // SAFETY: serialized exclusive session token.
        unsafe { &mut *session }.sources.open(depth, &path, mode)
    });
    let status = result.as_ref().err().map_or(0, errno);
    // SAFETY: disjoint writable errno result.
    unsafe {
        error.write(status);
    }
    if result.is_ok() {
        (depth + 1) as i32
    } else {
        0
    }
}
unsafe fn source<'a>(
    session: *mut Session,
    slot: c_int,
) -> Option<&'a mut crate::compiler_source::Source> {
    if session.is_null() || slot <= 0 {
        return None;
    }
    // SAFETY: live session; reference used only within the serialized ABI call.
    unsafe { &mut *session }.sources.source(slot as usize - 1)
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_eof(
    session: *mut Session,
    slot: c_int,
) -> c_int {
    // SAFETY: caller's exclusive live session, no retained reference.
    i32::from(unsafe { source(session, slot) }.is_none_or(|s| s.ended()))
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_rewind(session: *mut Session, slot: c_int) {
    // SAFETY: caller's exclusive live session.
    if let Some(s) = unsafe { source(session, slot) } {
        s.rewind();
    }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_unget(
    session: *mut Session,
    slot: c_int,
) -> c_int {
    // SAFETY: caller's exclusive live session.
    unsafe { source(session, slot) }.map_or(1, |s| s.unget_item())
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_check(
    session: *mut Session,
    slot: c_int,
    line: *mut c_int,
) -> c_int {
    if line.is_null() {
        return -1;
    }
    // SAFETY: caller's exclusive live session and initialized disjoint line.
    let Some(s) = (unsafe { source(session, slot) }) else {
        return -1;
    };
    // SAFETY: initialized disjoint scalar, borrowed only in this call.
    let (c, updated) = s.check_next(unsafe { *line });
    // SAFETY: disjoint writable line.
    unsafe {
        line.write(updated);
    }
    c
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_next(
    session: *mut Session,
    slot: c_int,
    kind: c_int,
    line: c_int,
    output: *mut Token,
) -> c_int {
    if session.is_null() || slot <= 0 || output.is_null() || !(1..=10).contains(&kind) {
        return 0;
    }
    // SAFETY: exclusive session; output is disjoint and writable.
    let Some(token) = unsafe { &mut *session }
        .sources
        .next(slot as usize - 1, kind, line)
    else {
        return 0;
    };
    // SAFETY: disjoint writable record; publish only the initialized result.
    unsafe {
        output.write(token);
    }
    1
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_source_line(
    session: *mut Session,
    slot: c_int,
    output: *mut u8,
    width: usize,
) -> c_int {
    if output.is_null() || !(2..=4096).contains(&width) {
        return 0;
    }
    // SAFETY: exclusive session. Foreign output need not be initialized.
    let Some(s) = (unsafe { source(session, slot) }) else {
        return 0;
    };
    let mut scratch = [0; 4096];
    let Some(n) = s.chunk(&mut scratch[..width]) else {
        return 0;
    };
    // SAFETY: disjoint writable output of declared width, initialized prefix only.
    unsafe {
        std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, n + 1);
    }
    1
}
