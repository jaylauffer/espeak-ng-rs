//! C dictionary driver bridge. Rule sources use pooled proactor reads.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    rule_compile::{Diagnostic, Host as RuleHost},
    rule_file::{self, Host},
};
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    sync::{Mutex, OnceLock},
};
type Encode =
    unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, usize, *mut c_int) -> c_int;
type Digit = unsafe extern "C" fn(u32) -> c_int;
type Report = unsafe extern "C" fn(*mut c_void, c_int, *const u8, c_int, c_int, *const c_char);
type Write = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int;
type Tell = unsafe extern "C" fn(*mut c_void, *mut u64) -> c_int;
#[repr(C)]
pub struct Descriptor {
    pub(super) context: *mut c_void,
    pub(super) letters: *const u8,
    pub(super) spaces: *const u8,
    pub(super) debug: c_int,
    pub(super) signed_char: c_int,
    pub(super) letter_offset: c_int,
    pub(super) text_windows: c_int,
    pub(super) encode: Option<Encode>,
    pub(super) digit: Option<Digit>,
    pub(super) report: Option<Report>,
    pub(super) write: Option<Write>,
    pub(super) tell: Option<Tell>,
}
#[repr(C)]
pub struct State {
    pub(super) line: c_int,
    pub(super) group3: c_int,
    pub(super) rules: c_int,
    pub(super) groups: c_int,
    pub(super) groups3: c_int,
    pub(super) letters: [u8; 95],
    pub(super) group: [u8; 13],
}
struct Effects {
    context: *mut c_void,
    encode: Encode,
    digit: Digit,
    report: Report,
    write: Write,
    tell: Tell,
    line: i32,
    group: [u8; 13],
}
impl RuleHost for Effects {
    fn encode(&mut self, input: &[u8], output: &mut [u8; 80]) -> Option<i32> {
        let mut bad = 0;
        // SAFETY: synchronous callback, initialized NUL input and owned 80-byte
        // scratch; pointers may not be retained beyond this call.
        let ok = unsafe {
            (self.encode)(
                self.context,
                input.as_ptr().cast(),
                output.as_mut_ptr().cast(),
                output.len(),
                &mut bad,
            )
        };
        (ok != 0).then_some(bad)
    }
    fn digit(&mut self, code: u32) -> bool {
        // SAFETY: synchronous primitive classifier from the descriptor.
        unsafe { (self.digit)(code) != 0 }
    }
    fn diagnostic(&mut self, kind: Diagnostic, value: i32, text: &[u8]) {
        self.report(kind as i32, value, text);
    }
}
impl Host for Effects {
    fn context(&mut self, line: i32, group: &[u8; 13]) {
        self.line = line;
        self.group = *group;
    }
    fn report(&mut self, kind: i32, value: i32, text: &[u8]) {
        // SAFETY: fully initialized 13-byte group, terminated text and
        // serialized context. Callback cannot retain these owned pointers.
        unsafe {
            (self.report)(
                self.context,
                self.line,
                self.group.as_ptr(),
                kind,
                value,
                text.as_ptr().cast(),
            )
        };
    }
    fn write(&mut self, bytes: &[u8]) -> bool {
        // SAFETY: initialized bytes retained for this synchronous writer call.
        unsafe { (self.write)(self.context, bytes.as_ptr(), bytes.len()) != 0 }
    }
    fn position(&mut self) -> Option<u64> {
        let mut position = 0;
        // SAFETY: caller-provided primitive writes the owned position slot.
        let ok = unsafe { (self.tell)(self.context, &mut position) };
        (ok != 0).then_some(position)
    }
}
#[derive(Default)]
struct Pool {
    source: Vec<u8>,
    compiler: rule_file::Workspace,
}
fn pool() -> &'static Mutex<Pool> {
    static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Pool::default()))
}

/// Serialized descriptor inputs are initialized and live through copying;
/// callbacks cannot retain scratch, unwind or mutate the source/workspace.
/// State is a disjoint writable record, published only on successful control
/// completion. Primitive file writes/errors may remain visible on rejection.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_compile_rule_file(
    descriptor: *const Descriptor,
    path: *const c_char,
    state: *mut State,
) -> c_int {
    const FAILED: i32 = 0x100001ff;
    if descriptor.is_null() || path.is_null() || state.is_null() {
        return FAILED;
    }
    // SAFETY: caller supplies the fully initialized descriptor.
    let d = unsafe { descriptor.read() };
    let (Some(encode), Some(digit), Some(report), Some(write), Some(tell)) =
        (d.encode, d.digit, d.report, d.write, d.tell)
    else {
        return FAILED;
    };
    if d.letters.is_null() || d.spaces.is_null() {
        return FAILED;
    }
    let mut effects = Effects {
        context: d.context,
        encode,
        digit,
        report,
        write,
        tell,
        line: 0,
        group: [0; 13],
    };
    let mut letters = [0; 95];
    let mut spaces = [0; 256];
    // SAFETY: initialized 95/256-byte arrays; copied before any effects.
    unsafe {
        std::ptr::copy_nonoverlapping(d.letters, letters.as_mut_ptr(), 95);
        std::ptr::copy_nonoverlapping(d.spaces, spaces.as_mut_ptr(), 256)
    };
    // SAFETY: live terminated compatibility path; create an owned path before
    // proactor completions or native callbacks, without a foreign string loan.
    let path = match crate::voice_storage::compat_path(unsafe { CStr::from_ptr(path) }.to_bytes()) {
        Ok(path) => path,
        Err(_) => {
            effects.report(19, 0, b"\0");
            return FAILED;
        }
    };
    let Ok(mut pool) = pool().try_lock() else {
        effects.report(17, 0, b"\0");
        return FAILED;
    };
    if crate::engine_io::read_file_into(&path, rule_file::MAX_BYTES, &mut pool.source).is_err() {
        effects.report(19, 0, b"\0");
        return FAILED;
    }
    let env = rule_file::Environment {
        letters: &letters,
        spaces: &spaces,
        debug: d.debug != 0,
        signed_char: d.signed_char != 0,
        letter_offset: d.letter_offset,
        text_mode: if d.text_windows != 0 {
            crate::voice_reader::TextMode::Windows
        } else {
            crate::voice_reader::TextMode::Binary
        },
    };
    let Pool { source, compiler } = &mut *pool;
    match compiler.compile(source, &env, &mut effects) {
        Ok(value) => {
            let value = State {
                line: value.line,
                group3: value.group3,
                rules: value.rules,
                groups: value.groups,
                groups3: value.groups3,
                letters: value.letters,
                group: value.group,
            };
            // SAFETY: caller's disjoint writable result, with no callback loan.
            unsafe { state.write(value) };
            0
        }
        Err(_) => FAILED,
    }
}
