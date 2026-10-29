//! The C runtime the C ABI's callers share: its standard error stream, stream
//! locking and writes, errno text, and binary standard output on Windows.
//! Rust calls them where C wrappers used to.
// SPDX-License-Identifier: GPL-3.0-or-later
use libc::FILE;
use std::ffi::{c_char, c_int, c_void};

#[cfg(unix)]
unsafe extern "C" {
    fn flockfile(file: *mut FILE);
    fn funlockfile(file: *mut FILE);
}
#[cfg(windows)]
unsafe extern "C" {
    fn __acrt_iob_func(index: u32) -> *mut FILE;
    fn _lock_file(file: *mut FILE);
    fn _unlock_file(file: *mut FILE);
    fn strerror_s(buffer: *mut c_char, length: usize, code: c_int) -> c_int;
    fn _setmode(descriptor: c_int, mode: c_int) -> c_int;
    fn _fileno(file: *mut FILE) -> c_int;
    fn _errno() -> *mut c_int;
}
// The UCRT defines sscanf only inline in its headers; this is what it calls.
#[cfg(all(espeak_native_host, windows))]
unsafe extern "C" {
    fn __stdio_common_vsscanf(
        options: u64,
        buffer: *const c_char,
        count: usize,
        format: *const c_char,
        locale: *mut c_void,
        arguments: *const *mut c_void,
    ) -> c_int;
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
unsafe extern "C" {
    static mut __stderrp: *mut FILE;
}
#[cfg(all(espeak_native_host, any(target_os = "macos", target_os = "ios")))]
unsafe extern "C" {
    static mut __stdoutp: *mut FILE;
}
// Bionic exports `stderr` only from API 23; before that (minSdk 21) the
// streams are the array `__sF`, whose elements are struct __sFILE: 152 bytes
// on LP64, 84 on ILP32 (NDK bits/struct_file.h).
#[cfg(target_os = "android")]
unsafe extern "C" {
    static mut __sF: [u8; 0];
}
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
unsafe extern "C" {
    static mut stderr: *mut FILE;
}
#[cfg(all(
    espeak_native_host,
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
unsafe extern "C" {
    static mut stdout: *mut FILE;
}

/// `N_PATH_BUF` (speech.h): `_MAX_PATH` on Windows, else `PATH_MAX`.
#[cfg(all(espeak_native_host, windows))]
pub(crate) const PATH_BUFFER: usize = 260;
#[cfg(all(espeak_native_host, not(windows)))]
pub(crate) const PATH_BUFFER: usize = libc::PATH_MAX as usize;
/// `PATHSEP` (speech.h).
#[cfg(espeak_native_host)]
pub(crate) const PATH_SEPARATOR: u8 = if cfg!(windows) { b'\\' } else { b'/' };

/// C's `stderr`.
pub(crate) fn standard_error() -> *mut FILE {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    // SAFETY: the C runtime's initialized stream pointer, only read.
    unsafe {
        __stderrp
    }
    #[cfg(target_os = "android")]
    // SAFETY: element 2 of bionic's initialized stream array.
    unsafe {
        const FILE_BYTES: usize = if cfg!(target_pointer_width = "64") {
            152
        } else {
            84
        };
        std::ptr::addr_of_mut!(__sF)
            .cast::<u8>()
            .add(2 * FILE_BYTES)
            .cast()
    }
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    // SAFETY: the C runtime's initialized stream pointer, only read.
    unsafe {
        stderr
    }
    #[cfg(windows)]
    // SAFETY: the UCRT's stream table; index 2 is standard error.
    unsafe {
        __acrt_iob_func(2)
    }
}

#[cfg(espeak_native_host)]
/// C's `stdout`.
pub(crate) fn standard_output() -> *mut FILE {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    // SAFETY: the C runtime's initialized stream pointer, only read.
    unsafe {
        __stdoutp
    }
    #[cfg(target_os = "android")]
    // SAFETY: element 1 of bionic's initialized stream array (see stderr).
    unsafe {
        const FILE_BYTES: usize = if cfg!(target_pointer_width = "64") {
            152
        } else {
            84
        };
        std::ptr::addr_of_mut!(__sF)
            .cast::<u8>()
            .add(FILE_BYTES)
            .cast()
    }
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    // SAFETY: the C runtime's initialized stream pointer, only read.
    unsafe {
        stdout
    }
    #[cfg(windows)]
    // SAFETY: the UCRT's stream table; index 1 is standard output.
    unsafe {
        __acrt_iob_func(1)
    }
}

#[cfg(espeak_native_host)]
/// Sets this thread's C `errno`.
pub(crate) fn set_errno(value: c_int) {
    #[cfg(any(target_os = "linux", target_os = "emscripten"))]
    // SAFETY: the C runtime's thread-local errno.
    unsafe {
        *libc::__errno_location() = value
    };
    #[cfg(target_os = "android")]
    // SAFETY: as above.
    unsafe {
        *libc::__errno() = value
    };
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    // SAFETY: as above.
    unsafe {
        *libc::__error() = value
    };
    #[cfg(windows)]
    // SAFETY: as above.
    unsafe {
        *_errno() = value
    };
}

/// `flockfile` (`_lock_file`).
///
/// # Safety
/// `file` is a live C stream.
pub(crate) unsafe fn lock(file: *mut FILE) {
    #[cfg(unix)]
    // SAFETY: caller contract.
    unsafe {
        flockfile(file)
    };
    #[cfg(windows)]
    // SAFETY: caller contract.
    unsafe {
        _lock_file(file)
    };
}

/// `funlockfile` (`_unlock_file`).
///
/// # Safety
/// `file` is a live C stream this thread locked.
pub(crate) unsafe fn unlock(file: *mut FILE) {
    #[cfg(unix)]
    // SAFETY: caller contract.
    unsafe {
        funlockfile(file)
    };
    #[cfg(windows)]
    // SAFETY: caller contract.
    unsafe {
        _unlock_file(file)
    };
}

/// `fwrite(bytes, 1, length, file)`.
///
/// # Safety
/// `file` is a live C stream.
pub(crate) unsafe fn write(file: *mut FILE, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    // SAFETY: caller's stream; initialized bytes borrowed for the call.
    unsafe { libc::fwrite(bytes.as_ptr().cast::<c_void>(), 1, bytes.len(), file) == bytes.len() }
}

/// The C runtime's text for `code`: POSIX `strerror_r` (glibc's
/// `__xpg_strerror_r`), or `strerror_s` on Windows, with the same zero and
/// short lengths.
///
/// # Safety
/// `output` has `length` writable bytes.
pub(crate) unsafe fn strerror(code: c_int, output: *mut c_char, length: usize) {
    #[cfg(unix)]
    // SAFETY: caller contract.
    unsafe {
        libc::strerror_r(code, output, length)
    };
    #[cfg(windows)]
    // SAFETY: caller contract.
    unsafe {
        strerror_s(output, length, code)
    };
}

#[cfg(espeak_native_host)]
/// Standard output in binary mode (Windows `_setmode`; nothing elsewhere).
///
/// # Safety
/// `file` is a live C stream.
pub(crate) unsafe fn binary_output(file: *mut FILE) {
    #[cfg(windows)]
    // SAFETY: caller contract; 0x8000 is _O_BINARY.
    unsafe {
        _setmode(_fileno(file), 0x8000)
    };
    #[cfg(not(windows))]
    let _ = file;
}

#[cfg(espeak_native_host)]
/// `sscanf(text, format, outputs[0], outputs[1], ...)` for one or two
/// outputs, with the C runtime's own parsing. On Windows it calls
/// `__stdio_common_vsscanf` as the UCRT's inline `sscanf` does: default
/// options, no locale, and a `va_list` that is the array of arguments.
///
/// # Safety
/// `text` is terminated, and each output matches its conversion.
pub(crate) unsafe fn scan(
    text: *const c_char,
    format: &std::ffi::CStr,
    outputs: &[*mut c_void],
) -> c_int {
    #[cfg(windows)]
    // SAFETY: caller contract; the argument array outlives the call.
    unsafe {
        __stdio_common_vsscanf(
            0,
            text,
            usize::MAX,
            format.as_ptr(),
            std::ptr::null_mut(),
            outputs.as_ptr(),
        )
    }
    #[cfg(not(windows))]
    // SAFETY: caller contract.
    unsafe {
        match *outputs {
            [first] => libc::sscanf(text, format.as_ptr(), first),
            [first, second] => libc::sscanf(text, format.as_ptr(), first, second),
            _ => -1,
        }
    }
}
