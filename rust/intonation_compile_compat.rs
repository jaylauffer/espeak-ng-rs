//! C intonation API projections; compiler/source/output policy is owned Rust.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    compiler_source::Classes,
    intonation_compile::{Host, Paths, Workspace, FAILED},
    voice_reader::TextMode,
};
use std::{
    ffi::{c_char, c_int, c_void},
    io,
    sync::{Mutex, OnceLock},
};
type Log = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Effect = unsafe extern "C" fn(*mut c_void);
type Error = unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int;
#[repr(C)]
pub struct Descriptor {
    pub(super) context: *mut c_void,
    pub(super) source: *const c_char,
    pub(super) destination: *const c_char,
    pub(super) spaces: *const u8,
    pub(super) digits: *const u8,
    pub(super) width: usize,
    pub(super) windows: c_int,
    pub(super) log: Option<Log>,
    pub(super) close: Option<Effect>,
    pub(super) reload: Option<Effect>,
    pub(super) error: Option<Error>,
}
struct Effects {
    descriptor: Descriptor,
    paths: [[u8; 4096]; 3],
}
fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
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
        io::ErrorKind::StorageFull => 28,
        _ => 5,
    }
}
impl Host for Effects {
    fn log(&mut self, bytes: &[u8]) {
        // SAFETY: owned initialized bytes, synchronous effect, no retained loan.
        unsafe {
            (self.descriptor.log.unwrap())(self.descriptor.context, bytes.as_ptr(), bytes.len());
        }
    }
    fn close_log(&mut self) {
        // SAFETY: caller's serialized live opaque log projection.
        unsafe {
            (self.descriptor.close.unwrap())(self.descriptor.context);
        }
    }
    fn reload(&mut self) {
        // SAFETY: synchronous caller-owned reload, after output completion.
        unsafe {
            (self.descriptor.reload.unwrap())(self.descriptor.context);
        }
    }
    fn file_error(&mut self, role: usize, error: io::Error) -> i32 {
        // SAFETY: owned terminated path; allocator projection cannot retain it.
        unsafe {
            (self.descriptor.error.unwrap())(
                self.descriptor.context,
                errno(&error),
                self.paths[role].as_ptr().cast(),
            )
        }
    }
}
fn pool() -> &'static Mutex<Workspace> {
    static POOL: OnceLock<Mutex<Workspace>> = OnceLock::new();
    POOL.get_or_init(|| {
        Mutex::new(Workspace::new(Classes {
            spaces: [0; 256],
            digits: [0; 256],
        }))
    })
}
unsafe fn snapshot(pointer: *const c_char) -> Option<[u8; 4096]> {
    if pointer.is_null() {
        return None;
    }
    let mut out = [0; 4096];
    for (i, byte) in out.iter_mut().enumerate() {
        // SAFETY: initialized through NUL within 4096, or all 4096 on rejection.
        *byte = unsafe { pointer.add(i).cast::<u8>().read() };
        if *byte == 0 {
            return Some(out);
        }
    }
    None
}
fn path(prefix: &[u8], suffix: &[u8], width: usize) -> [u8; 4096] {
    let mut out = [0; 4096];
    let n = prefix.len().min(width - 1);
    out[..n].copy_from_slice(&prefix[..n]);
    let m = suffix.len().min(width - 1 - n);
    out[n..n + m].copy_from_slice(&suffix[..m]);
    out
}
/// Descriptor/text/maps are initialized, outputs disjoint; compiler calls and
/// callbacks are serialized and cannot retain loans or unwind. No descriptor
/// pointer or source string survives admission or a proactor completion.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_intonation(descriptor: *const Descriptor) -> c_int {
    if descriptor.is_null() {
        return FAILED;
    }
    // SAFETY: caller's fully initialized descriptor copied before effects.
    let d = unsafe { descriptor.read() };
    if !(2..=4096).contains(&d.width)
        || d.spaces.is_null()
        || d.digits.is_null()
        || d.log.is_none()
        || d.close.is_none()
        || d.reload.is_none()
        || d.error.is_none()
    {
        return FAILED;
    }
    // SAFETY: initialized terminated source/destination strings copied first.
    let (Some(source), Some(destination)) = (unsafe { snapshot(d.source) }, unsafe {
        snapshot(d.destination)
    }) else {
        return FAILED;
    };
    let mut classes = Classes {
        spaces: [0; 256],
        digits: [0; 256],
    };
    // SAFETY: initialized 256-byte classifiers, copied before I/O and callbacks.
    unsafe {
        std::ptr::copy_nonoverlapping(d.spaces, classes.spaces.as_mut_ptr(), 256);
        std::ptr::copy_nonoverlapping(d.digits, classes.digits.as_mut_ptr(), 256);
    }
    let paths = [
        path(string(&source), b"/../phsource/intonation.txt", d.width),
        path(string(&source), b"/../phsource/intonation", d.width),
        path(string(&destination), b"/intonations", d.width),
    ];
    let width = d.width;
    let mut effects = Effects {
        descriptor: d,
        paths,
    };
    let make = |bytes: &[u8]| crate::voice_storage::compat_path(string(bytes));
    let (Ok(text), Ok(bare), Ok(output)) = (
        make(&effects.paths[0]),
        make(&effects.paths[1]),
        make(&effects.paths[2]),
    ) else {
        return FAILED;
    };
    let Ok(mut workspace) = pool().try_lock() else {
        return FAILED;
    };
    workspace.sources.classes = classes;
    let mode = if effects.descriptor.windows == 0 {
        TextMode::Binary
    } else {
        TextMode::Windows
    };
    workspace.compile(
        &mut effects,
        &Paths {
            text: &text,
            bare: &bare,
            output: &output,
        },
        width,
        mode,
    )
}
