//! Native resource lease admission, progress publication and output primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    compiler_context_compat::{self, Context},
    compiler_output_compat, compiler_resource_compat,
};
use crate::{
    compiler_output::{DATA, MANIFEST},
    compiler_resource::Progress,
    spect_file::UNSUPPORTED,
    wavegen::Wavegen,
};
use std::ffi::{c_char, c_int, c_void};
// libc does not expose localeconv on Windows. Only read the initialized first
// member of the CRT's lconv; never assume its remaining platform-specific layout.
// Microsoft locale.h declares decimal_point first and localeconv with C linkage:
// https://github.com/microsoft/win32metadata/blob/main/generation/WinSDK/RecompiledIdlHeaders/ucrt/locale.h
#[cfg(windows)]
#[repr(C)]
struct LocalePrefix {
    decimal_point: *const c_char,
}
#[cfg(windows)]
unsafe extern "C" {
    // The target's standard CRT linkage also supplies tolower/isspace/fwrite.
    fn localeconv() -> *const LocalePrefix;
}
#[cfg(not(windows))]
use libc::localeconv;
type Error = unsafe extern "C" fn(*mut c_void, i32, i32, i32, *const u8, usize);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    context: *mut Context,
    name: *const c_char,
    generator: *mut Wavegen,
    log: *mut c_void,
    width: usize,
    rate: i32,
    control: i32,
    error: Option<Error>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 56);
unsafe extern "C" fn log(context: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: copied local descriptor remains initialized through synchronous use.
    let file = unsafe { (*context.cast::<Descriptor>()).log.cast::<libc::FILE>() };
    // SAFETY: live exclusive borrowed diagnostic FILE and initialized byte loan;
    // retain C's ignored short-write diagnostic status policy, never close it.
    unsafe { libc::fwrite(bytes.cast(), 1, length, file) };
}
unsafe extern "C" fn error(
    context: *mut c_void,
    code: i32,
    a: i32,
    b: i32,
    name: *const u8,
    length: usize,
) {
    // SAFETY: local initialized descriptor copied before primitive dispatch.
    let d = unsafe { context.cast::<Descriptor>().read() };
    // SAFETY: live guarded native context; synchronous C diagnostic projection
    // cannot retain the owned byte loan, invalidate the context or unwind.
    unsafe { d.error.unwrap()(d.context.cast(), code, a, b, name, length) };
}
unsafe extern "C" fn position(context: *mut c_void) -> i64 {
    // SAFETY: initialized local descriptor and guarded live context. Read the
    // current output token between effects rather than retaining a stale token.
    let owner = unsafe { (*(*context.cast::<Descriptor>()).context).output_owner };
    compiler_output_compat::espeak_rs_compiler_output_position(owner, DATA as u32)
}
unsafe extern "C" fn write(
    context: *mut c_void,
    destination: i32,
    bytes: *const u8,
    length: usize,
) -> i32 {
    // SAFETY: guarded initialized context; fresh scalar token before each write.
    let owner = unsafe { (*(*context.cast::<Descriptor>()).context).output_owner };
    // SAFETY: owned initialized byte loan copied by the output owner before
    // proactor I/O, creating thread retained; no native context reference spans it.
    unsafe {
        compiler_output_compat::espeak_rs_compiler_output_write(
            owner,
            if destination == 0 { DATA } else { MANIFEST } as u32,
            bytes,
            length,
        )
    }
}
/// Descriptor/address initialized and aligned, address disjoint from native
/// context/generator. Inputs may overlap address; snapshots precede publication.
/// Context must be the live creating-thread Rust native owner, generator/row
/// views initialized/exclusive, log a borrowed live FILE. Locale/engine access
/// serialized. Diagnostic primitive may use the context between Rust scalar
/// accesses but cannot retain loans, invalidate views/leases or unwind. Native
/// source/resource/end reentry refuses while effects run. Contents use existing
/// pooled proactor I/O; context end consumes the retained resource lease once.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_resource_native(
    descriptor: *const Descriptor,
    address: *mut c_int,
) -> c_int {
    if descriptor.is_null() || address.is_null() {
        return UNSUPPORTED;
    }
    // SAFETY: complete initialized descriptor snapshot before effects or writes.
    let mut d = unsafe { descriptor.read() };
    if d.context.is_null()
        || d.generator.is_null()
        || d.log.is_null()
        || d.error.is_none()
        || !(2..=4096).contains(&d.width)
    {
        return UNSUPPORTED;
    }
    // SAFETY: initialized bounded name snapshot before metadata/publication.
    let Some(name) = (unsafe { compiler_resource_compat::snapshot(d.name) }) else {
        return UNSUPPORTED;
    };
    // SAFETY: live native owner; guard source/resource/teardown during effects.
    let _guard = match unsafe { compiler_context_compat::resource_guard(d.context) } {
        Ok(guard) => guard,
        Err(status) => return status,
    };
    // SAFETY: guarded initialized native context and disjoint address; snapshot
    // fields and row mnemonic before effects without retaining foreign references.
    let (source, mut owner, table, mnemonic, initial) = unsafe {
        let c = d.context;
        (
            std::ptr::addr_of!((*c).phsrc).read(),
            (*c).resource_owner,
            (*c).n_phoneme_tabs.checked_sub(1),
            if (*c).phoneme_out.is_null() {
                0
            } else {
                (*(*c).phoneme_out).mnemonic
            },
            Progress {
                address: address.read(),
                references: (*c).count_references,
                duplicates: (*c).duplicate_references,
                frames: (*c).count_frames,
            },
        )
    };
    let Some(table) = table else {
        return UNSUPPORTED;
    };
    if !source.contains(&0) {
        return UNSUPPORTED;
    }
    let mut lower = [0; 256];
    let mut space = [0; 256];
    for i in 0..256 {
        // SAFETY: unsigned-byte arguments satisfy C locale classifier contracts;
        // locale remains stable for this serialized compiler call.
        unsafe {
            lower[i] = libc::tolower(i as c_int);
            space[i] = u8::from(libc::isspace(i as c_int) != 0);
        }
    }
    // SAFETY: C runtime supplies a live initialized locale record and nonempty
    // terminated decimal string; read the unsigned first byte as native C did.
    let decimal = unsafe { *(*localeconv()).decimal_point as u8 };
    if decimal == 0 {
        return UNSUPPORTED;
    }
    if owner == 0 {
        owner = compiler_resource_compat::espeak_rs_compiler_resources_begin();
        if owner == 0 {
            return 12;
        }
        // SAFETY: guarded owner metadata, complete admission before publication.
        unsafe { (*d.context).resource_owner = owner };
    }
    let descriptor = compiler_resource_compat::Descriptor {
        owner,
        context: std::ptr::addr_of_mut!(d).cast(),
        source: source.as_ptr().cast(),
        name: name.as_ptr().cast(),
        generator: d.generator,
        lower: lower.as_ptr(),
        space: space.as_ptr(),
        width: d.width,
        rate: d.rate,
        control: d.control,
        mnemonic: mnemonic as i32,
        table,
        decimal: decimal.into(),
        initial,
        log: Some(log),
        error: Some(error),
        position: Some(position),
        write: Some(write),
    };
    let mut result = initial;
    // SAFETY: complete local descriptor/paths/maps and disjoint initialized output;
    // guarded context/generator live through callbacks and proactor completion.
    let status = unsafe {
        compiler_resource_compat::espeak_rs_compiler_resource_load(&descriptor, &mut result)
    };
    // SAFETY: effects completed, scalar publication matches the prior native C
    // host even on load failure; no foreign view/reference survives the guard.
    unsafe {
        address.write(result.address);
        (*d.context).count_references = result.references;
        (*d.context).duplicate_references = result.duplicates;
        (*d.context).count_frames = result.frames;
    }
    status
}
