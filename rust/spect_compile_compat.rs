//! Copied spectrum compiler admission and primitive C stream projections.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    spect_compile::{Host, Request, ResultState, Workspace},
    spect_file::UNSUPPORTED,
    wavegen::Wavegen,
};
use std::{
    ffi::{c_char, c_void},
    sync::{Mutex, OnceLock},
};
type Log = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Bad = unsafe extern "C" fn(*mut c_void, *const c_char);
type Position = unsafe extern "C" fn(*mut c_void) -> i64;
type Write = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
#[repr(C)]
pub struct Descriptor {
    context: *mut c_void,
    source: *const c_char,
    path: *const c_char,
    generator: *mut Wavegen,
    width: usize,
    rate: i32,
    control: i32,
    initial_count: i32,
    log: Option<Log>,
    bad: Option<Bad>,
    position: Option<Position>,
    write: Option<Write>,
}
struct Effects {
    descriptor: Descriptor,
}
impl Host for Effects {
    fn log(&mut self, bytes: &[u8]) {
        // SAFETY: owned initialized bytes, synchronous primitive, no retained loan.
        unsafe {
            self.descriptor.log.unwrap()(self.descriptor.context, bytes.as_ptr(), bytes.len());
        }
    }
    fn bad_source(&mut self, path: &[u8]) {
        let mut text = [0; 4096];
        let n = path.len().min(4095);
        text[..n].copy_from_slice(&path[..n]);
        // SAFETY: owned terminated string, synchronous error projection.
        unsafe {
            self.descriptor.bad.unwrap()(self.descriptor.context, text.as_ptr().cast());
        }
    }
    fn position(&mut self) -> Result<i64, i32> {
        // SAFETY: live serialized caller-owned stream, synchronous primitive.
        let n = unsafe { self.descriptor.position.unwrap()(self.descriptor.context) };
        if n < 0 {
            Err(5)
        } else {
            Ok(n)
        }
    }
    fn generator(&mut self) -> &mut Wavegen {
        // SAFETY: initialized live serialized engine Rust owner, admitted nonnull;
        // borrowed only for synchronous packing after I/O and before callbacks.
        unsafe { &mut *self.descriptor.generator }
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), i32> {
        // SAFETY: complete initialized bounded payload, synchronous stream effect.
        let status = unsafe {
            self.descriptor.write.unwrap()(self.descriptor.context, bytes.as_ptr(), bytes.len())
        };
        if status == 0 {
            Ok(())
        } else {
            Err(status)
        }
    }
}
unsafe fn snapshot(pointer: *const c_char) -> Option<[u8; 4096]> {
    if pointer.is_null() {
        return None;
    }
    let mut out = [0; 4096];
    for (i, b) in out.iter_mut().enumerate() {
        // SAFETY: initialized through NUL within 4096, or all 4096 on rejection.
        *b = unsafe { *pointer.add(i) as u8 };
        if *b == 0 {
            return Some(out);
        }
    }
    None
}
fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn pool() -> &'static Mutex<Workspace> {
    static POOL: OnceLock<Mutex<Workspace>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Workspace::default()))
}
#[cfg(target_pointer_width = "64")]
const _: () =
    assert!(std::mem::size_of::<Descriptor>() == 88 && std::mem::size_of::<ResultState>() == 12);
/// Descriptor/paths initialized and terminated, output live/disjoint, generator
/// initialized and exclusive. Effects cannot retain loans or unwind. No caller
/// path/descriptor reference or generator borrow crosses a proactor completion.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_spect(
    descriptor: *const Descriptor,
    out: *mut ResultState,
) -> i32 {
    if descriptor.is_null() || out.is_null() {
        return UNSUPPORTED;
    }
    // SAFETY: initialized descriptor copied before any I/O or effect.
    let d = unsafe { descriptor.read() };
    if !(2..=4096).contains(&d.width)
        || d.generator.is_null()
        || d.log.is_none()
        || d.bad.is_none()
        || d.position.is_none()
        || d.write.is_none()
    {
        return UNSUPPORTED;
    }
    // SAFETY: initialized source/reference strings copied before effects.
    let (Some(source), Some(reference)) =
        (unsafe { snapshot(d.source) }, unsafe { snapshot(d.path) })
    else {
        return UNSUPPORTED;
    };
    let mut filename = [0; 4096];
    let a = text(&source).len().min(d.width - 1);
    filename[..a].copy_from_slice(&source[..a]);
    let mut n = a;
    if n < d.width - 1 {
        filename[n] = b'/';
        n += 1;
    }
    let b = text(&reference).len().min(d.width - 1 - n);
    filename[n..n + b].copy_from_slice(&reference[..b]);
    let Ok(native) = crate::voice_storage::compat_path(text(&filename)) else {
        return UNSUPPORTED;
    };
    let Ok(mut workspace) = pool().try_lock() else {
        return UNSUPPORTED;
    };
    let request = Request {
        path: &native,
        filename: text(&filename),
        reference: text(&reference),
        rate: d.rate,
        control: d.control,
        initial_count: d.initial_count,
    };
    let (status, result) = workspace.compile(&mut Effects { descriptor: d }, &request);
    // SAFETY: caller-owned disjoint result; publish only after all effects finish.
    unsafe {
        out.write(result);
    }
    status
}
