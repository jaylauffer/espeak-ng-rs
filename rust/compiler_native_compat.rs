//! Native compiler host: Rust owns every compiler lease and its publication.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    compiler_context_compat::{self, Context, SourceOpen},
    compiler_driver_compat, compiler_output_compat, compiler_program_compat,
    compiler_tables_compat,
};
use crate::compiler_driver::{Counts, Op};
use std::ffi::{c_char, c_int, c_void};
type Engine = unsafe extern "C" fn(*mut c_void, *mut Context, c_int, c_int) -> c_int;
type Log = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Error = unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int;
type Fallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int;
type Classes = unsafe extern "C" fn(*mut u8, *mut u8);
#[repr(C)]
pub struct Descriptor {
    pub(super) context: *mut c_void,
    pub(super) home: *const c_char,
    pub(super) source: *const c_char,
    pub(super) destination: *const c_char,
    pub(super) alpha: *const u8,
    pub(super) spaces: *const u8,
    pub(super) width: usize,
    pub(super) windows: c_int,
    pub(super) keep_prefix_x: c_int,
    pub(super) version: u32,
    pub(super) log_file: *mut c_void,
    pub(super) engine: Option<Engine>,
    pub(super) log: Option<Log>,
    pub(super) error: Option<Error>,
    pub(super) fallback: Option<Fallback>,
    pub(super) classes: Option<Classes>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 120);
struct Runtime {
    d: Descriptor,
    context: *mut Context,
}
impl Runtime {
    fn engine(&mut self, op: c_int, value: c_int) -> c_int {
        // SAFETY: synchronous serialized primitive, quiescent initialized C view;
        // no Rust context reference survives its effect and no lease is consumed.
        unsafe { self.d.engine.unwrap()(self.d.context, self.context, op, value) }
    }
    unsafe fn effect(
        &mut self,
        op: c_int,
        value: c_int,
        source: *const c_char,
        destination: *const c_char,
        error_path: *mut u8,
    ) -> c_int {
        if op == Op::Allocate as i32 {
            let mut status = 0;
            // SAFETY: generic controller supplied an initialized owned path;
            // admitted width and disjoint initialized errno scratch.
            self.context = unsafe {
                compiler_context_compat::espeak_rs_compiler_context_begin(
                    source,
                    self.d.width,
                    &mut status,
                )
            };
            return status;
        }
        if self.context.is_null() {
            return 22;
        }
        match op {
            n if n == Op::Tables as i32 => {
                let owner = compiler_tables_compat::espeak_rs_compiler_tables_begin();
                // SAFETY: unique live context, only a scalar publication.
                unsafe { (*self.context).table_owner = owner };
                if owner == 0 {
                    12
                } else {
                    0
                }
            }
            n if n == Op::Program as i32 => {
                let owner = compiler_program_compat::espeak_rs_compiler_program_begin();
                // SAFETY: unique live context, only a scalar publication.
                unsafe { (*self.context).program_owner = owner };
                if owner == 0 {
                    12
                } else {
                    0
                }
            }
            n if n == Op::Initialize as i32 => {
                // SAFETY: exclusive context between primitive effects. Owned
                // filename initialized exactly as prior strncpy0 projection.
                unsafe {
                    (*self.context).error_count = 0;
                    (*self.context).f_errors = self.d.log_file;
                    (*self.context).current_fname = [0; 80];
                    std::ptr::copy_nonoverlapping(
                        c"phonemes".as_ptr().cast::<u8>(),
                        std::ptr::addr_of_mut!((*self.context).current_fname).cast(),
                        9,
                    );
                }
                0
            }
            n if n == Op::SourceOpen as i32 => {
                let d = SourceOpen {
                    context: self.context,
                    path: source,
                    width: self.d.width,
                    depth: 0,
                    text: 0,
                    classes: self.d.classes,
                };
                let mut status = 0;
                // SAFETY: copied descriptor and bounded owned path; no foreign
                // view/loan spans classification or proactor completion.
                let input = unsafe {
                    compiler_context_compat::espeak_rs_compiler_context_source(&d, &mut status)
                };
                // SAFETY: completed source admission, publish numeric file ID.
                unsafe { (*self.context).f_in = input };
                status
            }
            n if n == Op::OutputOpen as i32 => {
                // Read the current engine sample rate after voice/wave setup,
                // rather than assuming the public rate is still authoritative.
                let rate = self.engine(op, value);
                // SAFETY: live context field address, no reference retained.
                let source = unsafe { std::ptr::addr_of!((*self.context).phsrc) };
                let d = compiler_output_compat::Descriptor {
                    source: source.cast(),
                    destination,
                    width: self.d.width,
                    version: self.d.version,
                    rate,
                    context: self.context.cast(),
                    fallback: self.d.fallback,
                };
                let mut opened = compiler_output_compat::Outcome::default();
                // SAFETY: owned terminated paths and disjoint initialized output;
                // context retained until output end, no context loan spans I/O.
                let status = unsafe {
                    compiler_output_compat::espeak_rs_compiler_output_begin(&d, &mut opened)
                };
                if status != 0 {
                    // SAFETY: controller supplied disjoint 4096-byte scratch.
                    unsafe {
                        std::ptr::copy_nonoverlapping(opened.filename.as_ptr(), error_path, 4096);
                    }
                    return status;
                }
                // SAFETY: completed effects before publishing the output lease.
                unsafe {
                    (*self.context).output_owner = opened.owner;
                    (*self.context).output_mask = opened.mask;
                }
                0
            }
            n if n == Op::BaseTable as i32 => {
                // SAFETY: scalar reset precedes the table primitive's live view.
                unsafe { (*self.context).n_phoneme_tabs = 0 };
                self.engine(op, value)
            }
            n if n == Op::SourceClose as i32 => 0, // Bytes retained through cleanup.
            n if n == Op::OutputClose as i32 => {
                // SAFETY: snapshot only a scalar; no context borrow spans close.
                let owner = unsafe { (*self.context).output_owner };
                let status = compiler_output_compat::espeak_rs_compiler_output_end(owner);
                // SAFETY: consume the completed lease once and read fresh statuses.
                unsafe {
                    (*self.context).output_owner = 0;
                    if status != 0 {
                        status
                    } else if (*self.context).table_status != 0 {
                        (*self.context).table_status
                    } else {
                        (*self.context).program_status
                    }
                }
            }
            n if n == Op::Cleanup as i32 => {
                let context = std::mem::replace(&mut self.context, std::ptr::null_mut());
                // SAFETY: unique creating-thread context after all primitive views
                // drain; revoke the runtime pointer before ordered lease teardown.
                unsafe { compiler_context_compat::espeak_rs_compiler_context_end(context) };
                0
            }
            _ => self.engine(op, value),
        }
    }
}
unsafe extern "C" fn effect(
    opaque: *mut c_void,
    op: c_int,
    value: c_int,
    source: *const c_char,
    destination: *const c_char,
    error_path: *mut u8,
) -> c_int {
    // SAFETY: private stack runtime retained by the serialized generic controller;
    // owned path/scratch loans are synchronous and bounded by that controller.
    unsafe { (&mut *opaque.cast::<Runtime>()).effect(op, value, source, destination, error_path) }
}
unsafe extern "C" fn counts(opaque: *mut c_void, out: *mut Counts) {
    // SAFETY: private runtime, live quiescent context and initialized scalar output.
    unsafe {
        let c = (*opaque.cast::<Runtime>()).context;
        out.write(Counts {
            references: (*c).count_references,
            reused: (*c).duplicate_references,
            errors: (*c).error_count,
        });
    }
}
unsafe extern "C" fn log(opaque: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: private copied descriptor and owned initialized synchronous byte loan.
    unsafe {
        let d = &(*opaque.cast::<Runtime>()).d;
        d.log.unwrap()(d.context, bytes, length);
    }
}
unsafe extern "C" fn error(opaque: *mut c_void, status: c_int, path: *const c_char) -> c_int {
    // SAFETY: private copied descriptor and owned terminated synchronous path loan.
    unsafe {
        let d = &(*opaque.cast::<Runtime>()).d;
        d.error.unwrap()(d.context, status, path)
    }
}
/// Serialized engine, initialized descriptor/maps/bounded paths; callbacks are
/// synchronous primitives, cannot retain loans, unwind or consume compiler leases.
/// Generic admission snapshots inputs before effects and refuses nested compilation.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_phonemes_native(d: *const Descriptor) -> c_int {
    if d.is_null() {
        return 22;
    }
    // SAFETY: caller supplied initialized aligned descriptor; copy before effects.
    let d = unsafe { d.read() };
    if d.engine.is_none()
        || d.log.is_none()
        || d.error.is_none()
        || d.fallback.is_none()
        || d.classes.is_none()
        || d.log_file.is_null()
    {
        return 22;
    }
    let generic = compiler_driver_compat::Descriptor {
        context: std::ptr::null_mut(),
        home: d.home,
        source: d.source,
        destination: d.destination,
        alpha: d.alpha,
        spaces: d.spaces,
        width: d.width,
        windows: d.windows,
        keep_prefix_x: d.keep_prefix_x,
        effect: Some(effect),
        counts: Some(counts),
        log: Some(log),
        error: Some(error),
    };
    let mut runtime = Runtime {
        d,
        context: std::ptr::null_mut(),
    };
    let generic = compiler_driver_compat::Descriptor {
        context: (&mut runtime as *mut Runtime).cast(),
        ..generic
    };
    // SAFETY: initialized private descriptor and stack runtime survive the complete
    // synchronous run; original paths/maps are copied before primitive effects.
    unsafe { compiler_driver_compat::espeak_rs_compile_phonemes(&generic) }
}
