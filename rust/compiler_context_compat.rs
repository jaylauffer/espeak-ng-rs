//! Typed native compiler owner; C receives only quiescent writable projections.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    compiler_output_compat, compiler_program_compat, compiler_resource_compat,
    compiler_source_compat::{self, Session},
    compiler_tables_compat,
};
use crate::phoneme::Phoneme;
use std::{
    alloc::{alloc, dealloc, Layout},
    ffi::{c_char, c_int, c_void},
    thread::ThreadId,
};
#[repr(C)]
pub struct Context {
    phoneme_out: *mut Phoneme,
    table_owner: u64,
    table_status: c_int,
    phoneme_tab2: *mut Phoneme,
    definition_row: Phoneme,
    definition_phoneme: c_int,
    program_owner: u64,
    program_status: c_int,
    n_phoneme_tabs: c_int,
    n_phcodes: c_int,
    output_owner: u64,
    output_mask: u32,
    f_errors: *mut c_void,
    f_in: c_int,
    source_pool: *mut Session,
    linenum: c_int,
    count_references: c_int,
    duplicate_references: c_int,
    count_frames: c_int,
    error_count: c_int,
    current_fname: [u8; 80],
    resource_owner: u64,
    item_type: c_int,
    item_terminator: c_int,
    item_string: [u8; 256],
    phsrc: [u8; 4096],
}
impl Default for Context {
    fn default() -> Self {
        // SAFETY: only integer/byte fields, Phoneme integers and nullable raw pointers.
        unsafe { std::mem::zeroed() }
    }
}
#[repr(C)]
struct Owner {
    context: Context,
    opening: bool,
    thread: ThreadId,
}
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<Context>() == 4592);
    assert!(std::mem::offset_of!(Context, phsrc) == 496);
    assert!(std::mem::offset_of!(Context, source_pool) == 112);
    assert!(std::mem::offset_of!(Context, resource_owner) == 224);
    assert!(std::mem::size_of::<SourceOpen>() == 48);
};
#[repr(C)]
pub struct SourceOpen {
    context: *mut Context,
    path: *const c_char,
    width: usize,
    depth: usize,
    text: c_int,
    classes: Option<unsafe extern "C" fn(*mut u8, *mut u8)>,
}
unsafe fn snapshot(path: *const c_char, width: usize) -> Result<[u8; 4096], i32> {
    if path.is_null() || !(2..=4096).contains(&width) {
        return Err(22);
    }
    let mut bytes = [0; 4096];
    for (i, byte) in bytes[..width].iter_mut().enumerate() {
        // SAFETY: initialized through NUL, or full admitted width on rejection.
        *byte = unsafe { *path.add(i) as u8 };
        if *byte == 0 {
            return Ok(bytes);
        }
    }
    Err(crate::compiler_driver::NAME_TOO_LONG)
}
/// Bounded initialized path, disjoint writable errno. Returned initialized view
/// has a unique Rust owner and is consumed once on the creating thread.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_context_begin(
    path: *const c_char,
    width: usize,
    error: *mut c_int,
) -> *mut Context {
    if error.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: bounded initialized source is copied before result publication.
    let phsrc = match unsafe { snapshot(path, width) } {
        Ok(p) => p,
        Err(e) => {
            // SAFETY: writable scalar after the source snapshot is complete.
            unsafe { error.write(e) };
            return std::ptr::null_mut();
        }
    };
    // SAFETY: checked nonzero layout, allocator returns null on refusal.
    let owner = unsafe { alloc(Layout::new::<Owner>()) }.cast::<Owner>();
    if owner.is_null() {
        // SAFETY: admitted writable scalar, no owner was created.
        unsafe { error.write(12) };
        return std::ptr::null_mut();
    }
    let context = Context {
        phsrc,
        ..Context::default()
    };
    // SAFETY: unique aligned writable allocation, fully initialized before C view.
    unsafe {
        owner.write(Owner {
            context,
            opening: false,
            thread: std::thread::current().id(),
        });
        error.write(0);
    }
    owner.cast()
}
/// Consume the live unique owner after all C views drain, on its creating thread.
/// Refused reentry/thread checks retain it unchanged. NULL is an empty teardown.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_context_end(context: *mut Context) -> c_int {
    if context.is_null() {
        return 0;
    }
    let owner = context.cast::<Owner>();
    // SAFETY: live owner metadata, no foreign access during this control call.
    if unsafe { (*owner).opening || (*owner).thread != std::thread::current().id() } {
        return libc::EAGAIN;
    }
    // SAFETY: live exclusive metadata, guard reentry before cleanup effects.
    unsafe {
        (*owner).opening = true;
    }
    // SAFETY: unique quiescent context; copy all leases and revoke C views first.
    let (program, tables, output, sources, resources) = unsafe {
        let c = &mut *context;
        let leases = (
            c.program_owner,
            c.table_owner,
            c.output_owner,
            c.source_pool,
            c.resource_owner,
        );
        *c = Context::default();
        leases
    };
    let mut status = 0;
    for result in [
        compiler_program_compat::espeak_rs_compiler_program_end(program),
        compiler_tables_compat::espeak_rs_compiler_tables_end(tables),
        compiler_output_compat::espeak_rs_compiler_output_end(output),
    ] {
        if status == 0 {
            status = result;
        }
    }
    // SAFETY: copied exclusive source session ends once on its creating thread.
    unsafe {
        compiler_source_compat::espeak_rs_compiler_sources_end(sources);
    }
    let result = compiler_resource_compat::espeak_rs_compiler_resources_end(resources);
    if status == 0 {
        status = result;
    }
    // SAFETY: no live consumers remain, allocation is consumed exactly once.
    unsafe {
        std::ptr::drop_in_place(owner);
        dealloc(owner.cast(), Layout::new::<Owner>());
    }
    status
}
/// Descriptor/path initialized and snapshotted before callbacks; context is live
/// and serialized. No foreign reference or caller path loan spans proactor I/O.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_context_source(
    d: *const SourceOpen,
    error: *mut c_int,
) -> c_int {
    if error.is_null() {
        return 0;
    }
    if d.is_null() {
        // SAFETY: admitted writable scalar; no descriptor/owner access occurs.
        unsafe { error.write(22) };
        return 0;
    }
    // SAFETY: copied initialized descriptor; outputs may overlap inputs.
    let d = unsafe { d.read() };
    // SAFETY: initialized bounded path copied before callbacks/publication.
    let path = match unsafe { snapshot(d.path, d.width) } {
        Ok(p) => p,
        Err(e) => {
            // SAFETY: writable scalar after descriptor/path snapshot.
            unsafe { error.write(e) };
            return 0;
        }
    };
    if d.context.is_null()
        || d.classes.is_none()
        || d.depth >= crate::compiler_source::DEPTH
        || !(0..=1).contains(&d.text)
    {
        // SAFETY: admitted writable scalar, no owner mutation occurred.
        unsafe { error.write(22) };
        return 0;
    }
    let owner = d.context.cast::<Owner>();
    // SAFETY: raw metadata checks avoid creating aliased references on reentry.
    if unsafe { (*owner).opening || (*owner).thread != std::thread::current().id() } {
        // SAFETY: admitted writable scalar, refused owner remains unchanged.
        unsafe { error.write(libc::EAGAIN) };
        return 0;
    }
    // SAFETY: exclusive metadata; no reference spans callbacks/completions.
    unsafe {
        (*owner).opening = true;
    }
    let mut file_error = 0;
    // SAFETY: read/copy scalar token only, no reference into context crosses effects.
    let mut session = unsafe { (*d.context).source_pool };
    if session.is_null() {
        let mut spaces = [0; 256];
        let mut digits = [0; 256];
        // SAFETY: initialized owned scratch, primitive fills it synchronously.
        unsafe {
            d.classes.unwrap()(spaces.as_mut_ptr(), digits.as_mut_ptr());
        }
        // SAFETY: copied initialized maps and disjoint local status.
        session = unsafe {
            compiler_source_compat::espeak_rs_compiler_sources_begin(
                spaces.as_ptr(),
                digits.as_ptr(),
                &mut file_error,
            )
        };
        if session.is_null() && file_error == 11 {
            // The generic source session ABI retains its historical status;
            // this native errno surface uses the actual platform constant.
            file_error = libc::EAGAIN;
        }
        // SAFETY: guarded exclusive scalar update, callback has returned.
        unsafe {
            (*d.context).source_pool = session;
        }
    }
    let input = if session.is_null() {
        0
    } else {
        // SAFETY: exclusive owned session and copied terminated path, local output.
        unsafe {
            compiler_source_compat::espeak_rs_compiler_source_open(
                session,
                path.as_ptr().cast(),
                d.depth,
                if cfg!(windows) { d.text } else { 0 },
                &mut file_error,
            )
        }
    };
    // SAFETY: owner still live; reentry/end refused while opening, publish last.
    unsafe {
        (*owner).opening = false;
        error.write(file_error);
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static CLASSES: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "C" fn classes(spaces: *mut u8, digits: *mut u8) {
        CLASSES.fetch_add(1, Ordering::Relaxed);
        for i in 0..256 {
            // SAFETY: ABI provides two initialized writable 256-byte maps.
            unsafe {
                spaces
                    .add(i)
                    .write(u8::from((i as u8).is_ascii_whitespace()));
                digits.add(i).write(u8::from((i as u8).is_ascii_digit()));
            }
        }
    }
    #[test]
    fn native_context_snapshots_paths_reuses_source_session_and_ends_leases() {
        let root = std::env::temp_dir().join(format!("espeak-context-rust-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("source");
        std::fs::write(&path, b"alpha 12\n").unwrap();
        let name = CString::new(path.to_str().unwrap()).unwrap();
        let mut error = 42;
        // SAFETY: terminated input and disjoint output; unique native owner.
        let context =
            unsafe { espeak_rs_compiler_context_begin(c"original".as_ptr(), 260, &mut error) };
        assert!(!context.is_null() && error == 0);
        let mut d = SourceOpen {
            context,
            path: name.as_ptr(),
            width: 4096,
            depth: 0,
            text: 0,
            classes: Some(classes),
        };
        let before = CLASSES.load(Ordering::Relaxed);
        // SAFETY: live exclusive owner, initialized descriptor and disjoint status.
        assert_eq!(
            // SAFETY: live exclusive owner and initialized disjoint inputs.
            unsafe { espeak_rs_compiler_context_source(&d, &mut error) },
            1
        );
        // SAFETY: quiescent live C view, no control/byte loans overlap this read.
        let session = unsafe { (*context).source_pool };
        for _ in 0..16 {
            // SAFETY: same serialized live owner and initialized inputs.
            assert_eq!(
                // SAFETY: same serialized live owner, no outstanding byte loan.
                unsafe { espeak_rs_compiler_context_source(&d, &mut error) },
                1
            );
            // SAFETY: quiescent view after source control returns.
            assert_eq!(unsafe { (*context).source_pool }, session);
        }
        d.depth = crate::compiler_source::DEPTH - 1;
        // SAFETY: initialized deepest valid descriptor, exclusive live context.
        let deepest = unsafe { espeak_rs_compiler_context_source(&d, &mut error) };
        assert_eq!(deepest, 13);
        d.depth += 1;
        // SAFETY: complete initialized descriptor; depth refuses before effects.
        let refused = unsafe { espeak_rs_compiler_context_source(&d, &mut error) };
        assert_eq!(refused, 0);
        assert_eq!(error, libc::EINVAL);
        assert_eq!(CLASSES.load(Ordering::Relaxed), before + 1);
        // SAFETY: all views drained; consume once on creating thread.
        assert_eq!(unsafe { espeak_rs_compiler_context_end(context) }, 0);
        let spaces = [0; 256];
        let digits = [0; 256];
        // SAFETY: copied initialized maps and disjoint error output.
        let reopened = unsafe {
            compiler_source_compat::espeak_rs_compiler_sources_begin(
                spaces.as_ptr(),
                digits.as_ptr(),
                &mut error,
            )
        };
        assert!(!reopened.is_null() && error == 0);
        // SAFETY: consume the fresh exclusive session exactly once.
        unsafe {
            compiler_source_compat::espeak_rs_compiler_sources_end(reopened);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_context_rejects_bounds_and_foreign_thread_without_consuming_owner() {
        let mut error = 42;
        let bad = [b'x'; 260];
        // SAFETY: fully initialized rejection extent and disjoint output.
        assert!(
            // SAFETY: fully initialized rejection extent and disjoint status.
            unsafe { espeak_rs_compiler_context_begin(bad.as_ptr().cast(), 260, &mut error) }
                .is_null()
        );
        assert_eq!(error, crate::compiler_driver::NAME_TOO_LONG);
        // SAFETY: terminated source and disjoint output.
        let context =
            unsafe { espeak_rs_compiler_context_begin(c"owned".as_ptr(), 260, &mut error) };
        assert!(!context.is_null());
        let address = context as usize;
        // One bounded test thread checks refusal; production creates no worker.
        std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let context = address as *mut Context;
                    let mut error = 42;
                    let d = SourceOpen {
                        context,
                        path: c"missing".as_ptr(),
                        width: 260,
                        depth: 0,
                        text: 0,
                        classes: Some(classes),
                    };
                    // SAFETY: live owner remains quiescent on parent; rejected metadata
                    // checks create no owner/session references or effects.
                    unsafe {
                        assert_eq!(espeak_rs_compiler_context_source(&d, &mut error), 0);
                        assert_eq!(error, libc::EAGAIN);
                        assert_eq!(espeak_rs_compiler_context_end(context), libc::EAGAIN);
                    }
                })
                .join()
                .unwrap();
        });
        // SAFETY: creating thread retains the unique owner after refusal.
        unsafe {
            assert!((*context).source_pool.is_null());
            assert_eq!(crate::compiler_driver::string(&(*context).phsrc), b"owned");
            assert_eq!(espeak_rs_compiler_context_end(context), 0);
            assert_eq!(espeak_rs_compiler_context_end(std::ptr::null_mut()), 0);
        }
    }
}
