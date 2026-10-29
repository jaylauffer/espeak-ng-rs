//! Public MBROLA compiler host. No foreign string loan crosses proactor work.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::mbrola_compile::Workspace;
use std::{
    ffi::{c_char, c_void},
    sync::{Mutex, OnceLock},
};

#[repr(C)]
struct Config {
    width: usize,
    name_max: usize,
}
extern "C" {
    static espeak_rs_mbrola_compile_config: Config;
    static path_home: c_char;
    fn create_file_error_context(context: *mut c_void, status: i32, filename: *const c_char)
        -> i32;
}
unsafe fn snapshot(pointer: *const c_char, limit: usize) -> Option<[u8; 4096]> {
    if pointer.is_null() || !(1..=4096).contains(&limit) {
        return None;
    }
    let mut result = [0; 4096];
    for (i, b) in result[..limit].iter_mut().enumerate() {
        // SAFETY: caller supplies initialized bytes through the terminator.
        *b = unsafe { pointer.add(i).cast::<u8>().read() };
        if *b == 0 {
            return Some(result);
        }
    }
    None
}
fn text(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())]
}

/// The existing engine serializes path_home; filepath is NUL-terminated, log
/// and error context are valid caller-owned C objects for this entire call.
#[no_mangle]
pub unsafe extern "C" fn espeak_ng_CompileMbrolaVoice(
    filepath: *const c_char,
    log: *mut libc::FILE,
    context: *mut c_void,
) -> i32 {
    // SAFETY: immutable native configuration and live engine path global.
    let config = unsafe { &espeak_rs_mbrola_compile_config };
    if !(2..=4096).contains(&config.width) {
        return libc::EINVAL;
    }
    // SAFETY: copy all foreign input before opening files or delivering work.
    let (Some(source), Some(home)) = (unsafe { snapshot(filepath, 4096) }, unsafe {
        snapshot(std::ptr::addr_of!(path_home), config.width)
    }) else {
        return libc::EINVAL;
    };
    let name = text(&source);
    let separator = name
        .iter()
        .rposition(|b| matches!(*b, b'/' | b'\\'))
        .unwrap_or(0);
    let basename = if separator == 0 {
        name
    } else {
        &name[separator + 1..]
    };
    if basename.len() >= config.name_max {
        return libc::EINVAL;
    }
    let mut destination = [0; 4096];
    let mut length = 0;
    for part in [
        text(&home),
        b"/mbrola_ph/".as_slice(),
        basename,
        b"_phtrans",
    ] {
        let n = part.len().min(config.width - 1 - length);
        destination[length..length + n].copy_from_slice(&part[..n]);
        length += n;
    }
    static WORKSPACE: OnceLock<Mutex<Workspace>> = OnceLock::new();
    let Ok(mut workspace) = WORKSPACE
        .get_or_init(|| Mutex::new(Workspace::default()))
        .try_lock()
    else {
        return libc::EAGAIN;
    };
    let result = workspace.compile(text(&source), &destination[..length], cfg!(windows));
    match result {
        Err((status, output)) => {
            let path = if output { &destination } else { &source };
            // SAFETY: owned terminated path, caller retains error context.
            unsafe { create_file_error_context(context, status, path.as_ptr().cast()) }
        }
        Ok(rows) => {
            let mut message = [0; 4192];
            let mut n = 0;
            let mut append = |part: &[u8]| {
                message[n..n + part.len()].copy_from_slice(part);
                n += part.len();
            };
            append(b"Mbrola translation file: ");
            append(&destination[..length]);
            append(b" -- ");
            let digits = [
                b'0' + (rows / 100) as u8,
                b'0' + (rows / 10 % 10) as u8,
                b'0' + (rows % 10) as u8,
            ];
            append(
                &digits[if rows >= 100 {
                    0
                } else if rows >= 10 {
                    1
                } else {
                    2
                }..],
            );
            append(b" phonemes\n");
            // SAFETY: borrowed stream stays live; fwrite preserves C text mode
            // and locking. The workspace remains admitted during callbacks.
            unsafe {
                let log = if log.is_null() {
                    crate::crt::standard_error()
                } else {
                    log
                };
                libc::fwrite(message.as_ptr().cast(), 1, n, log);
            }
            0
        }
    }
}
