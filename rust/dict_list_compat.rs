//! Serialized dictionary compiler effects and pooled proactor list sources.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::dict_list::{self, Environment, Host, State};
use std::{
    ffi::{c_char, c_int, c_void},
    sync::{Mutex, OnceLock},
};
type Encode =
    unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, usize, *mut c_int) -> c_int;
type Translate = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut c_char);
type Case = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> c_int;
type Transpose = unsafe extern "C" fn(*mut c_void, *mut c_char) -> c_int;
type Report = unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, *const c_char, *const c_char);
type Write = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int;
#[repr(C)]
pub struct Descriptor {
    context: *mut c_void,
    alnums: *const u8,
    language_text: c_int,
    listx: c_int,
    windows: c_int,
    encode: Option<Encode>,
    translate: Option<Translate>,
    case: Option<Case>,
    transpose: Option<Transpose>,
    report: Option<Report>,
    write: Option<Write>,
}
#[repr(C)]
pub struct Published {
    line: c_int,
    text_mode: c_int,
    need_dictionary: c_int,
}
struct Effects {
    d: Descriptor,
}
impl Host for Effects {
    fn encode(&mut self, input: &[u8], output: &mut [u8; 200]) -> Option<i32> {
        let mut bad = 0;
        // SAFETY: synchronous primitive receives terminated owned input and
        // initialized writable scratch; neither pointer may be retained.
        let ok = unsafe {
            (self.d.encode?)(
                self.d.context,
                input.as_ptr().cast(),
                output.as_mut_ptr().cast(),
                200,
                &mut bad,
            )
        };
        (ok != 0).then_some(bad)
    }
    fn translate(&mut self, input: &mut [u8], output: &mut [u8; 200]) {
        // SAFETY: owned terminated translation scratch (204 bytes), output
        // has 200 writable bytes; serialized effect cannot retain these loans.
        unsafe {
            (self.d.translate.unwrap())(
                self.d.context,
                input.as_mut_ptr().cast(),
                output.as_mut_ptr().cast(),
            )
        };
    }
    fn case(&mut self, code: u32) -> Option<u32> {
        let mut lower = 0;
        // SAFETY: primitive classifier writes only the owned scalar result.
        let ok = unsafe { (self.d.case.unwrap())(self.d.context, code, &mut lower) };
        (ok != 0).then_some(lower)
    }
    fn transpose(&mut self, word: &mut [u8]) -> Option<usize> {
        // SAFETY: initialized owned 204-byte NUL key; existing engine alphabet
        // projection is serialized and may overwrite only its key prefix.
        let n = unsafe { (self.d.transpose.unwrap())(self.d.context, word.as_mut_ptr().cast()) };
        usize::try_from(n).ok()
    }
    fn report(&mut self, state: &State, kind: i32, value: i32, first: &[u8], second: &[u8]) {
        // SAFETY: initialized terminated owned text and scalar state; no loan
        // survives the synchronous native log writer.
        unsafe {
            (self.d.report.unwrap())(
                self.d.context,
                state.line,
                kind,
                value,
                first.as_ptr().cast(),
                second.as_ptr().cast(),
            )
        };
    }
    fn write(&mut self, bytes: &[u8]) -> bool {
        // SAFETY: initialized record bytes for the synchronous native writer.
        unsafe { (self.d.write.unwrap())(self.d.context, bytes.as_ptr(), bytes.len()) != 0 }
    }
}
unsafe fn admit(descriptor: *const Descriptor) -> Option<(Effects, [u8; 256])> {
    if descriptor.is_null() {
        return None;
    }
    // SAFETY: caller supplies a fully initialized live descriptor.
    let d = unsafe { descriptor.read() };
    if d.alnums.is_null()
        || d.encode.is_none()
        || d.translate.is_none()
        || d.case.is_none()
        || d.transpose.is_none()
        || d.report.is_none()
        || d.write.is_none()
    {
        return None;
    }
    let mut alnums = [0; 256];
    // SAFETY: exactly 256 initialized bytes, copied before all effects.
    unsafe { std::ptr::copy_nonoverlapping(d.alnums, alnums.as_mut_ptr(), 256) };
    Some((Effects { d }, alnums))
}
fn published(state: State) -> Published {
    Published {
        line: state.line,
        text_mode: i32::from(state.text_mode),
        need_dictionary: state.need_dictionary,
    }
}
/// Caller serializes engine effects and supplies initialized input through NUL
/// within 200 bytes, disjoint output/state/hash. Rejection leaves output/hash
/// untouched; already performed effects and parser state remain visible.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_dict_line(
    descriptor: *const Descriptor,
    input: *const c_char,
    output: *mut u8,
    capacity: usize,
    state: *mut Published,
    hash: *mut c_int,
) -> usize {
    if input.is_null() || output.is_null() || state.is_null() || hash.is_null() {
        return 0;
    }
    // SAFETY: descriptor and disjoint initialized state retained by caller.
    let Some((mut host, alnums)) = (unsafe { admit(descriptor) }) else {
        return 0;
    };
    // SAFETY: caller retains a fully initialized disjoint parser state.
    let old = unsafe { state.read() };
    let mut current = State {
        line: old.line,
        text_mode: old.text_mode != 0,
        need_dictionary: old.need_dictionary,
    };
    let mut line = [0; 200];
    let mut n = 0;
    for byte in &mut line {
        // SAFETY: caller admits initialized input through NUL, or 200 bytes
        // for an unterminated rejected input; no unused foreign tail is read.
        *byte = unsafe { *input.add(n) as u8 };
        n += 1;
        if *byte == 0 {
            break;
        }
    }
    let env = Environment {
        alnums: &alnums,
        language_text: host.d.language_text != 0,
    };
    let record = dict_list::compile_line(&mut host, &env, &mut current, &mut line[..n]);
    // SAFETY: caller's disjoint writable state; no callback remains active.
    unsafe { state.write(published(current)) };
    let Ok(Some(record)) = record else {
        return 0;
    };
    if record.length > capacity {
        return 0;
    }
    // SAFETY: output admits exactly the completed record, hash is disjoint.
    unsafe {
        std::ptr::copy_nonoverlapping(record.bytes.as_ptr(), output, record.length);
        *hash = record.hash as c_int;
    }
    record.length
}
#[derive(Default)]
struct Pool {
    source: Vec<u8>,
    compiler: dict_list::Workspace,
}
fn pool() -> &'static Mutex<Pool> {
    static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Pool::default()))
}
/// Compiles roots, list/listx, emoji and extra in legacy precedence order. Path
/// and byte classifiers are copied before effects. Missing optional files are
/// skipped; resource/read/write failures report and abort without C replay.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_dict_lists(
    descriptor: *const Descriptor,
    prefix: *const c_char,
    state: *mut Published,
) -> c_int {
    const FAILED: i32 = 0x100001ff;
    if prefix.is_null() || state.is_null() {
        return FAILED;
    }
    // SAFETY: serialized initialized descriptor and state from the C driver.
    let Some((mut host, alnums)) = (unsafe { admit(descriptor) }) else {
        return FAILED;
    };
    // SAFETY: caller retains a fully initialized disjoint parser state.
    let old = unsafe { state.read() };
    let mut current = State {
        line: old.line,
        text_mode: old.text_mode != 0,
        need_dictionary: old.need_dictionary,
    };
    let mut prefix_copy = [0; 4096];
    let mut prefix_length = None;
    for (index, byte) in prefix_copy.iter_mut().enumerate() {
        // SAFETY: caller supplies initialized path bytes through its NUL within
        // 4096 bytes, or all 4096 readable bytes for a rejected path.
        *byte = unsafe { *prefix.add(index) as u8 };
        if *byte == 0 {
            prefix_length = Some(index);
            break;
        }
    }
    let Some(prefix_length) = prefix_length.filter(|n| *n <= 4080) else {
        host.report(&current, 8, 0, b"\0", b"\0");
        return FAILED;
    };
    let prefix = &prefix_copy[..prefix_length];
    let env = Environment {
        alnums: &alnums,
        language_text: host.d.language_text != 0,
    };
    let mode = if host.d.windows != 0 {
        crate::voice_reader::TextMode::Windows
    } else {
        crate::voice_reader::TextMode::Binary
    };
    let Ok(mut pool) = pool().try_lock() else {
        host.report(&current, 8, 0, b"\0", b"\0");
        return FAILED;
    };
    pool.compiler.reset();
    let order = if host.d.listx != 0 {
        ["roots", "list", "listx", "emoji", "extra"]
    } else {
        ["roots", "listx", "list", "emoji", "extra"]
    };
    let mut filename = [0; 4096];
    let result = (|| {
        for name in order {
            current.text_mode = false;
            let mut found = false;
            let mut filename_length = 0;
            for extension in [".txt", ""] {
                filename_length = prefix.len() + name.len() + extension.len();
                filename[..prefix.len()].copy_from_slice(prefix);
                filename[prefix.len()..prefix.len() + name.len()].copy_from_slice(name.as_bytes());
                filename[prefix.len() + name.len()..filename_length]
                    .copy_from_slice(extension.as_bytes());
                let path = crate::voice_storage::compat_path(&filename[..filename_length])
                    .map_err(|_| dict_list::Error::Io)?;
                match crate::engine_io::read_file_into(
                    &path,
                    dict_list::MAX_BYTES,
                    &mut pool.source,
                ) {
                    Ok(()) => {
                        found = true;
                        break;
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                        ) => {}
                    Err(_) => return Err(dict_list::Error::Io),
                }
            }
            if !found {
                continue;
            }
            filename[filename_length] = 0;
            host.report(&current, 9, 0, &filename[..=filename_length], b"\0");
            let Pool { source, compiler } = &mut *pool;
            let count = compiler.source(source, mode, &env, &mut current, &mut host)?;
            host.report(&current, 10, count as i32, b"\0", b"\0");
        }
        pool.compiler.write(&mut host)
    })();
    // SAFETY: caller's writable state, publishing even partial parser state.
    unsafe { state.write(published(current)) };
    if let Err(error) = result {
        if error != dict_list::Error::Bounds {
            host.report(
                &current,
                if error == dict_list::Error::Io { 11 } else { 8 },
                0,
                b"\0",
                b"\0",
            );
        }
        FAILED
    } else {
        0
    }
}
