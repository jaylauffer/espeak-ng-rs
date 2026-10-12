//! Native diagnostic loans, scope and borrowed C-runtime FILE operations.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::compiler_context_compat::{self, Context};
use crate::compiler_diagnostic::{self, Kind};
use std::ffi::{c_char, c_int, c_void};
type Message = unsafe extern "C" fn(c_int, *mut c_char, usize);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Status {
    context: *mut Context,
    status: c_int,
    name: *const c_char,
    width: usize,
    message: Option<Message>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Status>() == 40);
unsafe fn copy(bytes: *const u8, length: usize, capacity: usize) -> Result<[u8; 4096], c_int> {
    if length > capacity || (length != 0 && bytes.is_null()) {
        return Err(22);
    }
    let mut copied = [0; 4096];
    if length != 0 {
        // SAFETY: admitted initialized byte span; copy before native publication.
        unsafe { std::ptr::copy_nonoverlapping(bytes, copied.as_mut_ptr(), length) };
    }
    Ok(copied)
}
unsafe fn emit(context: *mut Context, kind: Kind, bytes: &[u8], message: &[u8]) -> c_int {
    // SAFETY: admitted guarded native owner; scalar/owned filename snapshots,
    // no context reference survives formatting or C-runtime writes.
    let (source, line, count) = unsafe {
        (
            std::ptr::addr_of!((*context).current_fname).read(),
            (*context).linenum,
            (*context).error_count,
        )
    };
    let Some(end) = source.iter().position(|b| *b == 0) else {
        return 22;
    };
    let Some(diagnostic) = compiler_diagnostic::render(&source[..end], line, kind, bytes, message)
    else {
        return 22;
    };
    if count.checked_add(diagnostic.increment).is_none() {
        return 22;
    }
    for part in diagnostic.parts() {
        // SAFETY: live exclusively borrowed FILE, fresh scalar before each
        // primitive, owned initialized bytes; never close or retain the loan.
        let file = unsafe { (*context).f_errors.cast::<libc::FILE>() };
        if file.is_null() {
            return 22;
        }
        // SAFETY: admitted borrowed FILE and owned byte loan; retain original
        // prefix/body/newline writes and ignored short writes.
        unsafe { libc::fwrite(part.as_ptr().cast(), 1, part.len(), file) };
    }
    // SAFETY: read fresh progress after effects as original error_count++ did.
    let Some(count) = (unsafe { (*context).error_count }).checked_add(diagnostic.increment) else {
        return 22;
    };
    // SAFETY: guarded initialized context; publish fresh count after effects.
    unsafe { (*context).error_count = count };
    0
}
unsafe fn bytes(
    context: *mut c_void,
    input: *const u8,
    length: usize,
    capacity: usize,
    kind: Kind,
    line: Option<c_int>,
) -> c_int {
    if line == Some(c_int::MIN) {
        return 22;
    }
    // SAFETY: initialized byte loan copied before context/line publication.
    let input = match unsafe { copy(input, length, capacity) } {
        Ok(b) => b,
        Err(e) => return e,
    };
    let context = context.cast::<Context>();
    // SAFETY: live serialized creating-thread native context owner.
    let _guard = match unsafe { compiler_context_compat::diagnostic_guard(context) } {
        Ok(g) => g,
        Err(e) => return e,
    };
    // SAFETY: guarded live native scalar; refuse before any line publication.
    if unsafe { (*context).f_errors.is_null() } {
        return 22;
    }
    if let Some(line) = line {
        // SAFETY: scoped scalar line publication matches the item diagnostic.
        unsafe { (*context).linenum = line };
    }
    // SAFETY: native context guarded through writes; only owned byte spans.
    unsafe { emit(context, kind, &input[..length], b"") }
}
/// Live creating-thread Rust context and initialized byte span, at most 4096.
/// Input may overlap context fields; it copies before line/count publication.
/// Borrowed FILE and native owner stay live/exclusive through all primitives.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_diagnostic_text(
    context: *mut c_void,
    input: *const u8,
    length: usize,
) -> c_int {
    // SAFETY: caller supplies documented initialized native view and byte loan.
    unsafe { bytes(context, input, length, 4096, Kind::Text, None) }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_diagnostic_item(
    context: *mut c_void,
    line: c_int,
    input: *const u8,
    length: usize,
) -> c_int {
    // SAFETY: identical native diagnostic loan; line publishes after its copy.
    unsafe { bytes(context, input, length, 4096, Kind::Text, Some(line)) }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_diagnostic_table(
    context: *mut c_void,
    code: c_int,
    value: c_int,
    input: *const u8,
    length: usize,
) {
    // SAFETY: table diagnostics retain the original <256-byte name admission.
    unsafe { bytes(context, input, length, 255, Kind::Table(code, value), None) };
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_diagnostic_resource(
    context: *mut c_void,
    code: c_int,
    a: c_int,
    b: c_int,
    input: *const u8,
    length: usize,
) {
    // SAFETY: resource diagnostics retain the original <4096-byte name bound.
    unsafe {
        bytes(
            context,
            input,
            length,
            4095,
            Kind::Resource(code, a, b),
            None,
        )
    };
}
/// Descriptor copied before effects; optional context name initialized through
/// NUL within width (1..4096), all width bytes on refusal. Message primitive fills
/// a 512-byte terminated result synchronously, cannot retain loans, invalidate
/// context/FILE, or unwind. Source/resource/end/diagnostic reentry refuses during
/// it. Selected path copies before callback; prefix/count read fresh afterwards.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_diagnostic_status(descriptor: *const Status) -> c_int {
    if descriptor.is_null() {
        return 22;
    }
    // SAFETY: complete initialized descriptor copied before callbacks/writes.
    let d = unsafe { descriptor.read() };
    let Some(message) = d.message else {
        return 22;
    };
    let mut name = [0; 4096];
    let mut length = 0;
    if !d.name.is_null() {
        if !(1..=4096).contains(&d.width) {
            return 22;
        }
        for (i, byte) in name[..d.width].iter_mut().enumerate() {
            // SAFETY: initialized through NUL or admitted full width on refusal.
            *byte = unsafe { *d.name.add(i) as u8 };
            if *byte == 0 {
                length = i;
                break;
            }
            if i + 1 == d.width {
                return 22;
            }
        }
    }
    // SAFETY: live serialized native owner; guard all message/FILE effects.
    let _guard = match unsafe { compiler_context_compat::diagnostic_guard(d.context) } {
        Ok(g) => g,
        Err(e) => return e,
    };
    // SAFETY: live guarded scalar FILE view, refused before message effects.
    if unsafe { (*d.context).f_errors.is_null() } {
        return 22;
    }
    let mut output = [0; 512];
    // SAFETY: initialized owned message result, scope retained through callback;
    // no native context reference spans this foreign primitive.
    unsafe { message(d.status, output.as_mut_ptr().cast(), output.len()) };
    let Some(end) = output.iter().position(|b| *b == 0) else {
        return 22;
    };
    // SAFETY: fresh native metadata after message effects, owned string loans.
    unsafe {
        emit(
            d.context,
            Kind::Status(!d.name.is_null()),
            &name[..length],
            &output[..end],
        )
    }
}
