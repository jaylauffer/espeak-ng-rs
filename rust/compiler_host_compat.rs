//! Complete native callback host over the existing owned compiler controllers.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    compiler_context_compat::{self, Context, SourceOpen},
    compiler_diagnostic_compat as diagnostic, compiler_files_compat as files,
    compiler_items_compat as items, compiler_output_compat as output,
    compiler_program_compat as program, compiler_resource_native_compat as resource,
    compiler_source_compat as source, compiler_tables_compat as tables,
};
use crate::{
    compiler_files::Frame,
    compiler_output::{INDEX, LOG, TABLE},
    compiler_program::Token,
    phoneme::Phoneme,
    wavegen::Wavegen,
};
use std::ffi::{c_char, c_int, c_void};
type Rate = unsafe extern "C" fn() -> c_int;
type Generator = unsafe extern "C" fn() -> *mut Wavegen;
type Message = unsafe extern "C" fn(c_int, *mut c_char, usize);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub(super) context: *mut Context,
    pub(super) width: usize,
    pub(super) resource_log: *mut c_void,
    pub(super) rate: Option<Rate>,
    pub(super) generator: Option<Generator>,
    pub(super) message: Option<Message>,
    pub(super) file_error: Option<unsafe extern "C" fn(c_int)>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 56);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<Descriptor>() == 28);
// Only raw runtime/context scalar accesses cross callbacks. In particular no
// &mut Runtime survives a generic controller invoking another host callback.
unsafe fn config(r: *mut c_void) -> Descriptor {
    // SAFETY: initialized local descriptor retained through synchronous effects.
    unsafe { r.cast::<Descriptor>().read() }
}
unsafe fn name(bytes: *const u8, length: usize, limit: usize) -> Option<[u8; 4096]> {
    if length >= limit || (length != 0 && bytes.is_null()) {
        return None;
    }
    let mut out = [0; 4096];
    if length != 0 {
        // SAFETY: initialized bounded byte loan copied before native publication.
        unsafe { std::ptr::copy_nonoverlapping(bytes, out.as_mut_ptr(), length) };
    }
    Some(out)
}
unsafe fn status(r: *mut c_void, code: i32, name: *const c_char, width: usize) {
    // SAFETY: initialized copied local host descriptor.
    let d = unsafe { config(r) };
    let s = diagnostic::Status {
        context: d.context,
        status: code,
        name,
        width,
        message: d.message,
    };
    // SAFETY: native context retained, selected initialized path, primitive loan.
    unsafe { diagnostic::espeak_rs_compiler_diagnostic_status(&s) };
}
unsafe extern "C" fn error(r: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: owned bounded message and live native context; diagnostics scope it.
    let d = unsafe { config(r) };
    // SAFETY: native diagnostic retains the live owner and snapshots its byte loan.
    unsafe { diagnostic::espeak_rs_compiler_diagnostic_text(d.context.cast(), bytes, length) };
}
unsafe extern "C" fn item_error(r: *mut c_void, line: i32, bytes: *const u8, length: usize) {
    // SAFETY: copied host config, owned initialized message; scoped publication.
    let d = unsafe { config(r) };
    // SAFETY: owned message loan, native diagnostic scope retains the selected context.
    unsafe {
        diagnostic::espeak_rs_compiler_diagnostic_item(d.context.cast(), line, bytes, length)
    };
}
unsafe extern "C" fn table_error(
    r: *mut c_void,
    code: i32,
    value: i32,
    bytes: *const u8,
    length: usize,
) {
    // SAFETY: copied host config and initialized owned table message loan.
    let d = unsafe { config(r) };
    // SAFETY: owned table message loan, scoped native diagnostic dispatch.
    unsafe {
        diagnostic::espeak_rs_compiler_diagnostic_table(
            d.context.cast(),
            code,
            value,
            bytes,
            length,
        )
    };
}
unsafe extern "C" fn table_write(r: *mut c_void, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: fresh native output token, loan copied by existing writer before I/O.
    let c = unsafe { config(r).context };
    // SAFETY: live output token and owned bytes copied by the bounded writer.
    unsafe {
        output::espeak_rs_compiler_output_write((*c).output_owner, TABLE as u32, bytes, length)
    }
}
unsafe fn table(
    r: *mut c_void,
    operation: u32,
    name: *const c_char,
    control: i32,
) -> tables::Outcome {
    // SAFETY: initialized live scalar metadata; no context reference spans effects.
    let c = unsafe { config(r).context };
    // SAFETY: quiescent native scalars; no reference spans generic table callbacks.
    let d = unsafe {
        tables::Descriptor {
            owner: (*c).table_owner,
            context: r,
            name,
            target: if operation == 3 {
                (*c).phoneme_out
            } else {
                std::ptr::null_mut()
            },
            line: (*c).linenum,
            control,
            base: Some(base),
            error: Some(table_error),
            write: Some(table_write),
        }
    };
    // SAFETY: only nullable pointers and integer fields, initialized before call.
    let mut result: tables::Outcome = unsafe { std::mem::zeroed() };
    // SAFETY: copied bounded inputs and disjoint initialized result; live owner.
    let code = unsafe { tables::espeak_rs_compiler_tables_call(&d, operation, &mut result) };
    if code != 0 {
        result.status = code;
        // SAFETY: scalar status publication before diagnostic effects.
        unsafe {
            if (*c).table_status == 0 {
                (*c).table_status = code;
            }
        }
        // SAFETY: owned static terminated path and guarded status diagnostic.
        unsafe { status(r, code, c"phoneme tables".as_ptr(), 16) };
    } else {
        // SAFETY: completed effects before publishing the current quiescent view.
        unsafe {
            (*c).phoneme_tab2 = result.current;
            (*c).n_phoneme_tabs = result.tables;
            (*c).n_phcodes = result.count;
        }
    }
    result
}
unsafe fn lookup(r: *mut c_void, name: *const c_char, control: i32) -> i32 {
    // SAFETY: admitted local runtime and copied name.
    let t = unsafe { table(r, 1, name, control) };
    if t.status != 0 {
        -1
    } else {
        t.value
    }
}
unsafe extern "C" fn item_lookup(
    r: *mut c_void,
    line: i32,
    bytes: *const u8,
    length: usize,
) -> i32 {
    // SAFETY: owned initialized name, copied before publishing line.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return -1;
    };
    // SAFETY: initialized copied runtime retained throughout synchronous lookup.
    let c = unsafe { config(r).context };
    // SAFETY: live native context scalar, then synchronous table operation.
    unsafe {
        (*c).linenum = line;
        lookup(r, name.as_ptr().cast(), 2)
    }
}
unsafe fn item(r: *mut c_void, kind: i32) -> i32 {
    // SAFETY: initialized native owner; only raw scalar/array pointers copied.
    let c = unsafe { config(r).context };
    // SAFETY: quiescent source/item fields, generic scanner copies bytes before effects.
    let d = unsafe {
        items::Descriptor {
            source: (*c).source_pool,
            slot: (*c).f_in,
            kind,
            line: (*c).linenum,
            terminator: (*c).item_terminator,
            previous: std::ptr::addr_of!((*c).item_string).cast(),
            context: r,
            lookup: Some(item_lookup),
            error: Some(item_error),
        }
    };
    let mut result = std::mem::MaybeUninit::uninit();
    // SAFETY: generic item controller copies previous bytes before callbacks;
    // no context reference survives scanning, lookup or diagnostics.
    let code = unsafe { items::espeak_rs_compiler_item(&d, result.as_mut_ptr()) };
    if code != 0 {
        // SAFETY: failure status/kind scalar publication matches prior C host.
        unsafe {
            if (*c).program_status == 0 {
                (*c).program_status = code;
            }
            (*c).item_type = -1;
        }
        return -2;
    }
    // SAFETY: success initializes the complete item result.
    let result = unsafe { result.assume_init() };
    // SAFETY: source/lookup/diagnostic effects complete before scalar/byte update.
    unsafe {
        (*c).linenum = result.line;
        (*c).item_type = result.token.kind;
        (*c).item_terminator = result.token.terminator;
        (*c).item_string = result.token.text;
    }
    result.token.value
}
unsafe extern "C" fn base(r: *mut c_void, bytes: *mut u8, width: usize) -> i32 {
    // SAFETY: owned item controller, then snapshot its fully initialized text.
    unsafe { item(r, 2) };
    // SAFETY: live copied runtime after item callbacks have completed.
    let c = unsafe { config(r).context };
    // SAFETY: scalar-owned array snapshot after item effects.
    let name = unsafe { std::ptr::addr_of!((*c).item_string).read() };
    let Some(n) = name.iter().position(|b| *b == 0) else {
        return 22;
    };
    if n >= width || bytes.is_null() {
        return 22;
    }
    // SAFETY: disjoint owned base scratch from generic table controller.
    unsafe { std::ptr::copy_nonoverlapping(name.as_ptr(), bytes, n + 1) };
    0
}
unsafe extern "C" fn next(r: *mut c_void, kind: i32, out: *mut Token) -> i32 {
    // SAFETY: scoped copied configuration and live native scalar view.
    let c = unsafe { config(r).context };
    // SAFETY: keep existing program failure precedence before consuming source.
    if unsafe { (*c).program_status != 0 } {
        // SAFETY: fresh sticky status scalar on the live guarded context.
        return unsafe { (*c).program_status };
    }
    // SAFETY: item callbacks use the same raw runtime, no &mut borrow spans them.
    let value = unsafe { item(r, kind) };
    // SAFETY: fresh native fields after item effects; initialized output scratch
    // belongs to the generic controller and is disjoint from context/runtime.
    unsafe {
        out.write(Token {
            value,
            kind: (*c).item_type,
            terminator: (*c).item_terminator,
            text: (*c).item_string,
        });
        (*c).program_status
    }
}
unsafe extern "C" fn unget(r: *mut c_void) {
    // SAFETY: live scoped host; source scanner releases its borrow before return.
    let c = unsafe { config(r).context };
    // SAFETY: live source session and slot copied; source effect retains no loans.
    let line = unsafe { source::espeak_rs_compiler_source_unget((*c).source_pool, (*c).f_in) };
    // SAFETY: completed source effect before line publication.
    unsafe { (*c).linenum = line };
}
unsafe extern "C" fn named_lookup(r: *mut c_void, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: initialized bounded owned name loan.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return -1;
    };
    // SAFETY: copied name lives through synchronous table effects.
    unsafe { lookup(r, name.as_ptr().cast(), 2) }
}
unsafe extern "C" fn mode_lookup(
    r: *mut c_void,
    bytes: *const u8,
    length: usize,
    control: i32,
) -> i32 {
    // SAFETY: initialized bounded owned name loan.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return -1;
    };
    // SAFETY: copied name and scoped context retained through table effects.
    unsafe { lookup(r, name.as_ptr().cast(), control) }
}
unsafe extern "C" fn resource_error(
    c: *mut c_void,
    code: i32,
    a: i32,
    b: i32,
    bytes: *const u8,
    length: usize,
) {
    // SAFETY: actual context passed by native resource host, owned message loan;
    // diagnostic guard nests under its resource scope and retains the owner.
    unsafe { diagnostic::espeak_rs_compiler_diagnostic_resource(c, code, a, b, bytes, length) };
}
unsafe extern "C" fn load(r: *mut c_void, bytes: *const u8, length: usize, control: i32) -> i32 {
    // SAFETY: initialized bounded owned name copied before engine queries.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return 0;
    };
    // SAFETY: initialized local configuration; queries cannot access native views,
    // retain host state or unwind. Read the rate after querying the generator,
    // matching the previous descriptor's selected engine state.
    let d = unsafe { config(r) };
    // SAFETY: admitted synchronous generator query; its returned engine state stays live.
    let generator = unsafe { d.generator.unwrap()() };
    // SAFETY: admitted synchronous rate query, serialized with the engine state.
    let rate = unsafe { d.rate.unwrap()() };
    let request = resource::Descriptor {
        context: d.context,
        name: name.as_ptr().cast(),
        generator,
        log: d.resource_log,
        width: d.width,
        rate,
        control,
        error: Some(resource_error),
    };
    let mut address = 0;
    // SAFETY: live context/generator and copied name; address disjoint, resource
    // scope handles source/diagnostic effects and final progress publication.
    unsafe { resource::espeak_rs_compiler_resource_native(&request, &mut address) };
    address
}
unsafe extern "C" fn position(r: *mut c_void) -> i64 {
    // SAFETY: live native scalar output token copied before flushing.
    let c = unsafe { config(r).context };
    // SAFETY: live native output token, no reference spans the flush effect.
    let owner = unsafe { (*c).output_owner };
    output::espeak_rs_compiler_output_flush(owner, INDEX as u32);
    // SAFETY: read fresh token after flush effects, as the prior host did.
    output::espeak_rs_compiler_output_position(unsafe { (*c).output_owner }, INDEX as u32)
}
unsafe extern "C" fn write(r: *mut c_void, log: i32, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: scoped native token and owned initialized loan, copied by writer.
    let c = unsafe { config(r).context };
    // SAFETY: bounded writer snapshots its byte loan before I/O.
    unsafe {
        output::espeak_rs_compiler_output_write(
            (*c).output_owner,
            if log != 0 { LOG } else { INDEX } as u32,
            bytes,
            length,
        )
    }
}
unsafe extern "C" fn eof(r: *mut c_void) -> i32 {
    // SAFETY: live scoped source session and numeric input, no retained loans.
    let c = unsafe { config(r).context };
    // SAFETY: live source session and numeric slot, synchronous EOF query.
    unsafe { source::espeak_rs_compiler_source_eof((*c).source_pool, (*c).f_in) }
}
unsafe extern "C" fn row(r: *mut c_void, code: i32, out: *mut Phoneme) -> i32 {
    // SAFETY: live scalar metadata; selected row view remains quiescent here.
    let c = unsafe { config(r).context };
    // SAFETY: quiescent current-table pointer metadata, retained by the native table lease.
    if !(0..=255).contains(&code) || unsafe { (*c).phoneme_tab2.is_null() } {
        return 22;
    }
    // SAFETY: selected initialized 256-row current view and disjoint owned output.
    unsafe { out.write((*c).phoneme_tab2.add(code as usize).read()) };
    0
}
unsafe extern "C" fn project(r: *mut c_void, input: *const Phoneme) -> i32 {
    // SAFETY: initialized owned row copied before native view publication.
    let value = unsafe { input.read() };
    // SAFETY: live copied runtime; source row was already snapshotted.
    let c = unsafe { config(r).context };
    // SAFETY: scalar metadata admission before a selected current-table write.
    if unsafe { (*c).definition_phoneme != 0 && (*c).phoneme_tab2.is_null() } {
        return 22;
    }
    // SAFETY: admitted live context/current row; no loan spans another effect.
    unsafe {
        (*c).definition_row = value;
        (*c).phoneme_out = std::ptr::addr_of_mut!((*c).definition_row);
        if (*c).definition_phoneme != 0 {
            (*c).phoneme_tab2.add(value.code as usize).write(value);
        }
    }
    0
}
unsafe extern "C" fn find(
    r: *mut c_void,
    bytes: *const u8,
    length: usize,
    out: *mut Phoneme,
) -> i32 {
    // SAFETY: initialized owned name, copied before table effects.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return -22;
    };
    // SAFETY: live table owner; current/found pointers valid after return.
    let result = unsafe { table(r, 2, name.as_ptr().cast(), 0) };
    if result.found.is_null() {
        return 0;
    }
    // SAFETY: initialized quiescent found row; caller supplied owned row output.
    unsafe { out.write(result.found.read()) };
    1
}
unsafe extern "C" fn import(
    r: *mut c_void,
    bytes: *const u8,
    length: usize,
    out: *mut Phoneme,
) -> i32 {
    // SAFETY: owned initialized name copied before table effects.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return 22;
    };
    // SAFETY: live copied runtime before direct import effects.
    let c = unsafe { config(r).context };
    // Import retains its original direct status path, without TableCall's
    // current-view publication or extra status diagnostic/sticky-table update.
    // SAFETY: quiescent native table metadata; owned output row is disjoint.
    let d = unsafe {
        tables::Descriptor {
            owner: (*c).table_owner,
            context: r,
            name: name.as_ptr().cast(),
            target: out,
            line: (*c).linenum,
            control: 0,
            base: Some(base),
            error: Some(table_error),
            write: Some(table_write),
        }
    };
    // SAFETY: nullable pointers and integers only, fully initialized result.
    let mut result: tables::Outcome = unsafe { std::mem::zeroed() };
    // SAFETY: initialized copied name/row and disjoint result; all loans synchronous.
    unsafe { tables::espeak_rs_compiler_tables_call(&d, 3, &mut result) }
}
unsafe extern "C" fn unknown(r: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: owned initialized bounded feature name.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return;
    };
    // SAFETY: owned terminated name retained through status-message effects.
    unsafe { status(r, 0x10000fff, name.as_ptr().cast(), 256) };
}
unsafe fn definition(r: *mut c_void, phoneme: i32) -> i32 {
    // SAFETY: live copied runtime before definition/controller effects.
    let c = unsafe { config(r).context };
    let mut alnum = [0; 256];
    for (i, v) in alnum.iter_mut().enumerate() {
        // SAFETY: unsigned-byte locale input; whole engine/locale is serialized.
        *v = u8::from(unsafe { libc::isalnum(i as c_int) } != 0);
    }
    // SAFETY: scalar publication before the definition controller's callbacks.
    unsafe { (*c).definition_phoneme = phoneme };
    // SAFETY: native program token and log mask copied, no view borrow spans callbacks.
    let descriptor = unsafe {
        program::Descriptor {
            owner: (*c).program_owner,
            context: r,
            words: std::ptr::null(),
            length: 0,
            control: i32::from((*c).output_mask & (1 << LOG) != 0),
            kind: 0,
            next: Some(next),
            unget: Some(unget),
            error: Some(error),
            lookup: Some(named_lookup),
            load: Some(load),
            position: Some(position),
            write: Some(write),
        }
    };
    let descriptor = program::Definition {
        program: descriptor,
        alnum: alnum.as_ptr(),
        eof: Some(eof),
        lookup: Some(mode_lookup),
        row: Some(row),
        project: Some(project),
        find: Some(find),
        import: Some(import),
        unknown: Some(unknown),
    };
    let mut result = std::mem::MaybeUninit::uninit();
    // SAFETY: copied descriptor/map, owned result; callbacks use raw runtime only.
    let code = unsafe {
        program::espeak_rs_compiler_definition(&descriptor, phoneme, result.as_mut_ptr())
    };
    // SAFETY: sticky first-error publication after all definition effects.
    unsafe {
        if code != 0 && (*c).program_status == 0 {
            (*c).program_status = code;
        }
    }
    code
}

/// Live disjoint 256-byte classifier outputs. The serialized engine owns locale.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_host_classes(spaces: *mut u8, digits: *mut u8) {
    for i in 0..256 {
        // SAFETY: unsigned-byte locale input and admitted initialized map scratch.
        unsafe {
            spaces
                .add(i)
                .write(u8::from(libc::isspace(i as c_int) != 0));
            digits
                .add(i)
                .write(u8::from(libc::isdigit(i as c_int) != 0));
        }
    }
}
unsafe extern "C" fn reset(r: *mut c_void) -> i32 {
    // SAFETY: live admitted context; scalar publication before source effects.
    unsafe {
        let c = config(r).context;
        (*c).linenum = 1;
        (*c).count_references = 0;
        (*c).duplicate_references = 0;
        (*c).count_frames = 0;
    }
    0
}
unsafe extern "C" fn frame(r: *mut c_void, out: *mut Frame) -> i32 {
    // SAFETY: owned disjoint frame scratch and live quiescent native fields.
    unsafe {
        let c = config(r).context;
        out.write(Frame {
            file: (*c).f_in,
            line: (*c).linenum,
            name: (*c).current_fname,
        });
    }
    0
}
unsafe extern "C" fn project_frame(r: *mut c_void, input: *const Frame) -> i32 {
    // SAFETY: initialized owned frame copied before publishing native fields.
    unsafe {
        let value = input.read();
        let c = config(r).context;
        (*c).f_in = value.file;
        (*c).linenum = value.line;
        (*c).current_fname = value.name;
    }
    0
}
unsafe extern "C" fn open(r: *mut c_void, bytes: *const u8, length: usize, depth: usize) -> i32 {
    // SAFETY: scoped copied host descriptor and owned bounded path loan.
    let d = unsafe { config(r) };
    // SAFETY: admitted width; copied path before source or errno effects.
    let Some(path) = (unsafe { name(bytes, length, d.width) }) else {
        return -22;
    };
    let request = SourceOpen {
        context: d.context,
        path: path.as_ptr().cast(),
        width: d.width,
        depth,
        text: 0,
        classes: Some(espeak_rs_compiler_host_classes),
    };
    let mut code = 0;
    // SAFETY: host guard retains context; internal source admission nests here.
    let file =
        unsafe { compiler_context_compat::espeak_rs_compiler_context_source(&request, &mut code) };
    if file == 0 {
        // SAFETY: admitted synchronous errno primitive, no loans or owner access.
        unsafe { d.file_error.unwrap()(code) };
    }
    file
}
unsafe extern "C" fn close(_: *mut c_void) -> i32 {
    0
} // Session retains completed source bytes.
unsafe extern "C" fn count(r: *mut c_void) -> i32 {
    // SAFETY: live admitted context, copied native table count only.
    unsafe { (*config(r).context).n_phoneme_tabs }
}
unsafe extern "C" fn end(r: *mut c_void) -> i32 {
    // SAFETY: complete table effects precede a fresh sticky status read.
    unsafe {
        table(r, 4, std::ptr::null(), 0);
        (*config(r).context).table_status
    }
}
unsafe extern "C" fn start(r: *mut c_void, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: initialized bounded name copied before table/base-item effects.
    let Some(name) = (unsafe { name(bytes, length, 256) }) else {
        return 22;
    };
    // SAFETY: copied name retained; complete effects precede sticky status read.
    unsafe {
        table(r, 0, name.as_ptr().cast(), 0);
        (*config(r).context).table_status
    }
}
unsafe extern "C" fn compile(r: *mut c_void, phoneme: i32) -> i32 {
    // SAFETY: guarded host; complete definition effects precede sticky status read.
    unsafe {
        definition(r, phoneme);
        (*config(r).context).program_status
    }
}
unsafe extern "C" fn terminate(r: *mut c_void) -> i32 {
    // SAFETY: live scoped table owner; no name used by terminate operation.
    unsafe { table(r, 6, std::ptr::null(), 0).status }
}
unsafe fn admit(pointer: *const Descriptor) -> Result<Descriptor, i32> {
    if pointer.is_null() {
        return Err(22);
    }
    // SAFETY: initialized descriptor copied before any callback or publication.
    let d = unsafe { pointer.read() };
    if d.context.is_null()
        || !(2..=4096).contains(&d.width)
        || d.rate.is_none()
        || d.generator.is_none()
        || d.message.is_none()
        || d.file_error.is_none()
    {
        return Err(22);
    }
    Ok(d)
}
/// Live native context on its creating thread, serialized engine and callbacks.
/// Descriptor copied before effects; name terminates within 256 when used.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_host_tables(
    pointer: *const Descriptor,
    operation: u32,
    name: *const c_char,
    control: i32,
) -> i32 {
    if operation > 6 {
        return 22;
    }
    // SAFETY: initialized descriptor; all structural admission precedes effects.
    let mut d = match unsafe { admit(pointer) } {
        Ok(d) => d,
        Err(e) => return e,
    };
    // SAFETY: live actual native owner, retained until all callbacks return.
    let _guard = match unsafe { compiler_context_compat::host_guard(d.context) } {
        Ok(g) => g,
        Err(e) => return e,
    };
    // SAFETY: local copied configuration, no Rust reference spans callbacks.
    unsafe { table(std::ptr::addr_of_mut!(d).cast(), operation, name, control).status }
}
/// Same admission as tables; copied root and owned include frames/controllers.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compiler_host_files(pointer: *const Descriptor) -> i32 {
    // SAFETY: initialized descriptor; all structural admission precedes effects.
    let mut d = match unsafe { admit(pointer) } {
        Ok(d) => d,
        Err(e) => return e,
    };
    // SAFETY: live actual native owner, retained until all callbacks return.
    let _guard = match unsafe { compiler_context_compat::host_guard(d.context) } {
        Ok(g) => g,
        Err(e) => return e,
    };
    let c = d.context;
    // SAFETY: raw context root loan copied by file controller before callbacks.
    let request = files::Descriptor {
        context: std::ptr::addr_of_mut!(d).cast(),
        // SAFETY: live raw native root loan, copied by file controller before effects.
        root: unsafe { std::ptr::addr_of!((*c).phsrc).cast() },
        width: d.width,
        reset: Some(reset),
        eof: Some(eof),
        next: Some(next),
        frame: Some(frame),
        project: Some(project_frame),
        open: Some(open),
        close: Some(close),
        tables: Some(count),
        end: Some(end),
        start: Some(start),
        compile: Some(compile),
        terminate: Some(terminate),
        error: Some(error),
    };
    let mut completed = 0;
    // SAFETY: owned descriptor/result; only raw runtime access across callbacks.
    let code = unsafe { files::espeak_rs_compiler_files(&request, &mut completed) };
    // SAFETY: complete effects before first-error scalar publication.
    unsafe {
        if code != 0 && (*c).program_status == 0 {
            (*c).program_status = code;
        }
    }
    code
}
