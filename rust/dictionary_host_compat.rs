//! Owned native dictionary context and complete controller callback wiring.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    dict_list_compat as lists, dictionary_compile_compat as driver,
    dictionary_diagnostic_compat as diagnostic, rule_file_compat as rules,
};
use std::{
    alloc::{alloc, dealloc, Layout},
    ffi::{c_char, c_int, c_void},
    sync::atomic::{AtomicBool, Ordering},
    thread::ThreadId,
};
#[repr(C)]
pub struct Context {
    log: *mut c_void,
    error: *mut c_void,
    word_phonemes: [u8; 200],
    line: c_int,
    count: c_int,
    text_mode: c_int,
    debug: c_int,
    need_dictionary: c_int,
    letters: [u8; 95],
    group: [u8; 13],
    group3: c_int,
}
#[repr(C)]
struct Owner {
    context: Context,
    hosting: bool,
    thread: ThreadId,
}
#[repr(C)]
pub struct Environment {
    alnums: [u8; 256],
    spaces: [u8; 256],
    language_text: c_int,
    listx: c_int,
    letter_offset: c_int,
}
type Encode =
    unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, usize, *mut c_int) -> c_int;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    context: *mut Context,
    source: *const c_char,
    name: *const c_char,
    home: *const c_char,
    table: *const c_char,
    width: usize,
    separator: c_int,
    windows: c_int,
    signed_char: c_int,
    error_context: *mut c_void,
    environment: Option<unsafe extern "C" fn(c_int, *mut Environment)>,
    encode: Option<Encode>,
    translate: Option<unsafe extern "C" fn(*mut c_char)>,
    case: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> c_int>,
    transpose: Option<unsafe extern "C" fn(*mut c_void, *mut c_char) -> c_int>,
    digit: Option<unsafe extern "C" fn(u32) -> c_int>,
    reload: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
    file_error: Option<unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> c_int>,
}
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<Context>() == 352);
    assert!(std::mem::offset_of!(Context, letters) == 236);
    assert!(std::mem::size_of::<Descriptor>() == 136);
};
#[cfg(target_pointer_width = "32")]
const _: () = {
    assert!(std::mem::size_of::<Context>() == 340);
    assert!(std::mem::offset_of!(Context, letters) == 228);
    assert!(std::mem::size_of::<Descriptor>() == 72);
};
const _: () = assert!(std::mem::size_of::<Environment>() == 524);
static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Guard(*mut Owner);
impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: host/end refuse throughout effects; creating-thread owner lives.
        unsafe { (*self.0).hosting = false };
        ACTIVE.store(false, Ordering::Release);
    }
}
/// Allocate initialized quiescent storage; streams remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_context_begin(
    log: *mut c_void,
    error: *mut c_void,
    flags: c_int,
    result: *mut c_int,
) -> *mut Context {
    if result.is_null() {
        return std::ptr::null_mut();
    }
    if log.is_null() || error.is_null() {
        // SAFETY: admitted initialized disjoint scalar, no owner created.
        unsafe { result.write(libc::EINVAL) };
        return std::ptr::null_mut();
    }
    // SAFETY: nonzero checked layout; allocation may return null.
    let owner = unsafe { alloc(Layout::new::<Owner>()) }.cast::<Owner>();
    if owner.is_null() {
        // SAFETY: admitted writable disjoint result.
        unsafe { result.write(libc::ENOMEM) };
        return std::ptr::null_mut();
    }
    // SAFETY: integers, bytes and nullable raw pointers admit zero initialization.
    let mut context: Context = unsafe { std::mem::zeroed() };
    context.log = log;
    context.error = error;
    context.debug = flags & 1;
    // SAFETY: unique aligned allocation, initialized before handing out a view.
    unsafe {
        owner.write(Owner {
            context,
            hosting: false,
            thread: std::thread::current().id(),
        });
        result.write(0);
    }
    owner.cast()
}
/// Consume a unique live owner, once on its creating thread after views drain.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_context_end(context: *mut Context) -> c_int {
    if context.is_null() {
        return 0;
    }
    let owner = context.cast::<Owner>();
    // SAFETY: only live native owner metadata, no Rust view spans an effect.
    if unsafe { (*owner).hosting || (*owner).thread != std::thread::current().id() } {
        return libc::EAGAIN;
    }
    // SAFETY: unique quiescent allocation, all callbacks drained; FILEs not owned.
    unsafe {
        std::ptr::drop_in_place(owner);
        dealloc(owner.cast(), Layout::new::<Owner>());
    }
    0
}
#[derive(Clone, Copy)]
struct Runtime {
    d: Descriptor,
    output: *mut c_void,
}
unsafe fn config(r: *mut c_void) -> Runtime {
    // SAFETY: local initialized runtime retained through synchronous callbacks.
    unsafe { r.cast::<Runtime>().read() }
}
#[allow(clippy::too_many_arguments)] // Mirrors the existing diagnostic ABI.
unsafe fn report(
    r: *mut c_void,
    family: i32,
    line: i32,
    kind: i32,
    value: i32,
    first: *const c_char,
    second: *const c_char,
    group: *const u8,
) {
    // SAFETY: live native owner; copied runtime before raw field projection.
    let c = unsafe { config(r).d.context };
    // SAFETY: raw field addresses, no native context reference spans FILE callbacks.
    let d = unsafe {
        diagnostic::Descriptor {
            log: (*c).log,
            error: (*c).error,
            line: std::ptr::addr_of_mut!((*c).line),
            count: std::ptr::addr_of_mut!((*c).count),
            group: std::ptr::addr_of_mut!((*c).group).cast(),
            width: 13,
        }
    };
    // SAFETY: guarded owner, initialized selected loans copied before publication.
    unsafe {
        diagnostic::espeak_rs_dictionary_diagnostic(
            &d, family, line, kind, value, first, second, group,
        )
    };
}
unsafe extern "C" fn list_report(
    r: *mut c_void,
    line: i32,
    kind: i32,
    value: i32,
    first: *const c_char,
    second: *const c_char,
) {
    // SAFETY: initialized generic list diagnostic scratch, synchronous dispatch.
    unsafe { report(r, 0, line, kind, value, first, second, std::ptr::null()) };
}
unsafe extern "C" fn file_report(
    r: *mut c_void,
    line: i32,
    group: *const u8,
    kind: i32,
    value: i32,
    text: *const c_char,
) {
    // SAFETY: generic file supplies initialized group13 and selected NUL text.
    unsafe { report(r, 2, line, kind, value, text, std::ptr::null(), group) };
}
unsafe extern "C" fn log(r: *mut c_void, kind: i32, text: *const c_char) {
    // SAFETY: generic driver supplies owned NUL text, copied by diagnostic bridge.
    unsafe { report(r, 3, 0, kind, 0, text, std::ptr::null(), std::ptr::null()) };
}
unsafe extern "C" fn translate(r: *mut c_void, input: *mut c_char, output: *mut c_char) {
    // SAFETY: copied runtime, primitive receives only the initialized owned input.
    let d = unsafe { config(r).d };
    // SAFETY: serialized synchronous primitive; cannot retain input or unwind.
    unsafe { d.translate.unwrap()(input) };
    // Preserve historical strncpy0(output, ctx->word_phonemes, N_WORD_BYTES-4),
    // including the private buffer's initially empty value. Do not replace it
    // with the engine global pronunciation buffer (a separate legacy behavior).
    for i in 0..155 {
        // SAFETY: live owner's initialized word200, exclusive generic output200.
        let byte = unsafe {
            std::ptr::addr_of!((*d.context).word_phonemes)
                .cast::<u8>()
                .add(i)
                .read()
        };
        // SAFETY: one bounded output byte, no loan survives the native primitive.
        unsafe { output.add(i).write(byte as c_char) };
        if byte == 0 {
            return;
        }
    }
    // SAFETY: generic output admits200 bytes; strncpy0 terminates at index155.
    unsafe { output.add(155).write(0) };
}
unsafe extern "C" fn write(r: *mut c_void, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: exclusive generic driver writer token, live only in this nested call.
    unsafe { driver::espeak_rs_dictionary_write(config(r).output.cast(), bytes, length) }
}
unsafe extern "C" fn tell(r: *mut c_void, position: *mut u64) -> i32 {
    // SAFETY: live exclusive driver writer token and disjoint writable scalar.
    unsafe { driver::espeak_rs_dictionary_tell(config(r).output.cast(), position) }
}
unsafe fn environment(d: Descriptor, rules: i32) -> Environment {
    // SAFETY: bytes and integers only; primitives receive initialized scratch.
    let mut e: Environment = unsafe { std::mem::zeroed() };
    // SAFETY: serialized native projection fills scratch, cannot retain its loan.
    unsafe { d.environment.unwrap()(rules, &mut e) };
    e
}
unsafe extern "C" fn compile_lists(
    r: *mut c_void,
    path: *const c_char,
    writer: *mut c_void,
) -> i32 {
    // SAFETY: copied configuration; environment refreshed after preceding log.
    let d = unsafe { config(r).d };
    // SAFETY: synchronous environment primitive, owned initialized storage.
    let e = unsafe { environment(d, 0) };
    let mut runtime = Runtime { d, output: writer };
    let host = lists::Descriptor {
        context: std::ptr::addr_of_mut!(runtime).cast(),
        alnums: e.alnums.as_ptr(),
        language_text: e.language_text,
        listx: e.listx,
        windows: d.windows,
        encode: d.encode,
        translate: Some(translate),
        case: d.case,
        transpose: d.transpose,
        report: Some(list_report),
        write: Some(write),
    };
    // SAFETY: fresh raw context scalars, no reference retained by the controller.
    let mut state = unsafe {
        lists::Published {
            line: (*d.context).line,
            text_mode: (*d.context).text_mode,
            need_dictionary: (*d.context).need_dictionary,
        }
    };
    // SAFETY: copied native config, generic driver owned path, exclusive writer.
    let status = unsafe { lists::espeak_rs_compile_dict_lists(&host, path, &mut state) };
    // SAFETY: all nested effects drained; preserve unconditional list publication.
    unsafe {
        (*d.context).line = state.line;
        (*d.context).text_mode = state.text_mode;
        (*d.context).need_dictionary = state.need_dictionary;
    }
    status
}
unsafe extern "C" fn compile_rules(
    r: *mut c_void,
    path: *const c_char,
    writer: *mut c_void,
) -> i32 {
    // SAFETY: copied runtime; environment refreshed after the rule-path log.
    let d = unsafe { config(r).d };
    // SAFETY: initialized owned projection scratch retained until copied.
    let e = unsafe { environment(d, 1) };
    let mut runtime = Runtime { d, output: writer };
    let host = rules::Descriptor {
        context: std::ptr::addr_of_mut!(runtime).cast(),
        // SAFETY: initialized letter95, copied by generic admission before effects.
        letters: unsafe { std::ptr::addr_of!((*d.context).letters).cast() },
        spaces: e.spaces.as_ptr(),
        // SAFETY: fresh initialized scalar, no view spans a callback.
        debug: unsafe { (*d.context).debug },
        signed_char: d.signed_char,
        letter_offset: e.letter_offset,
        text_windows: d.windows,
        encode: d.encode,
        digit: d.digit,
        report: Some(file_report),
        write: Some(write),
        tell: Some(tell),
    };
    // SAFETY: integer/byte fields only, initialized disjoint result.
    let mut state: rules::State = unsafe { std::mem::zeroed() };
    // SAFETY: live guarded context, owned path, bounded synchronous writer loan.
    let status = unsafe { rules::espeak_rs_compile_rule_file(&host, path, &mut state) };
    if status == 0 {
        // SAFETY: all nested effects drained; preserve success-only publication.
        unsafe {
            (*d.context).line = state.line;
            (*d.context).group3 = state.group3;
            (*d.context).group = state.group;
            (*d.context).letters = state.letters;
        }
    }
    status
}
unsafe extern "C" fn reload(r: *mut c_void, name: *const c_char) {
    // SAFETY: copied primitive config and generic driver's owned terminated name.
    let d = unsafe { config(r).d };
    // SAFETY: serialized engine primitive cannot retain loan, access views or unwind.
    unsafe { d.reload.unwrap()(d.error_context, name) };
}
unsafe extern "C" fn errors(r: *mut c_void) -> i32 {
    // SAFETY: fresh initialized native count after all nested diagnostics drained.
    unsafe { (*config(r).d.context).count }
}
unsafe extern "C" fn error(r: *mut c_void, status: i32, path: *const c_char) -> i32 {
    // SAFETY: copied primitive config, owned path passed only for the synchronous call.
    let d = unsafe { config(r).d };
    // SAFETY: live caller error-context, serialized allocator primitive.
    unsafe { d.file_error.unwrap()(d.error_context, status, path) }
}
/// Native owner and serialized engine primitives; input strings copy before effects.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_dictionary_host_compile(pointer: *const Descriptor) -> c_int {
    if pointer.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: initialized descriptor copied before any owner/publication effects.
    let d = unsafe { pointer.read() };
    if d.context.is_null()
        || !(2..=4096).contains(&d.width)
        || d.environment.is_none()
        || d.encode.is_none()
        || d.translate.is_none()
        || d.case.is_none()
        || d.transpose.is_none()
        || d.digit.is_none()
        || d.reload.is_none()
        || d.file_error.is_none()
    {
        return libc::EINVAL;
    }
    let owner = d.context.cast::<Owner>();
    // SAFETY: only unique live owner's metadata, no borrowed context spans effects.
    if unsafe { (*owner).hosting || (*owner).thread != std::thread::current().id() }
        || ACTIVE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        return libc::EAGAIN;
    }
    // SAFETY: creating-thread live owner admitted before all nested callbacks.
    unsafe { (*owner).hosting = true };
    let _guard = Guard(owner);
    let mut runtime = Runtime {
        d,
        output: std::ptr::null_mut(),
    };
    let driver = driver::Descriptor {
        context: std::ptr::addr_of_mut!(runtime).cast(),
        source: d.source,
        name: d.name,
        home: d.home,
        table: d.table,
        path_width: d.width,
        separator: d.separator,
        lists: Some(compile_lists),
        rules: Some(compile_rules),
        log: Some(log),
        reload: Some(reload),
        errors: Some(errors),
        error: Some(error),
    };
    // SAFETY: generic controller snapshots all four input strings before effects;
    // runtime is accessed only by raw copies, not an &mut spanning callbacks.
    unsafe { driver::espeak_rs_compile_dictionary(&driver) }
}
