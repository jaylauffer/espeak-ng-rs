//! The C engine state and entry points the native hosts still reach: the
//! translators and options C defines, and the C functions (bridges to Rust,
//! or C that later work moves) they call. Translators are opaque here; the
//! hosts cast them to `crate::translator::Translator`, whose layout is
//! checked against C.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]
use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    pub(super) static mut translator: *mut c_void;
    pub(super) static mut translator2: *mut c_void;
    pub(super) static mut translator3: *mut c_void;
    pub(super) static mut voice: *mut c_void;
    pub(super) static mut option_sayas: c_int;
    pub(super) static mut option_phonemes: c_int;
    pub(super) static mut dictionary_skipwords: c_int;
    pub(super) static mut f_trans: *mut libc::FILE;
    pub(super) static mut pre_pause: c_int;
    /// `N_WORD_PHONEMES` bytes (translate.h).
    pub(super) static mut word_phonemes: [c_char; 200];
    pub(super) static mut dictionary_name: [c_char; 40];
    pub(super) static mut path_home: c_char;

    pub(super) fn LookupBounded(
        tr: *mut c_void,
        word: *const c_char,
        output: *mut c_char,
        capacity: usize,
    ) -> c_int;
    pub(super) fn LookupDictListBounded(
        tr: *mut c_void,
        word: *mut *mut c_char,
        output: *mut c_char,
        flags: *mut u32,
        end_flags: c_int,
        words: *mut c_void,
        remaining: c_int,
        capacity: usize,
    ) -> c_int;
    pub(super) fn TranslateWord(
        tr: *mut c_void,
        word: *mut c_char,
        words: *mut c_void,
        output: *mut c_char,
    ) -> c_int;
    pub(super) fn TranslateRules(
        tr: *mut c_void,
        start: *mut c_char,
        phonemes: *mut c_char,
        size: c_int,
        end_phonemes: *mut c_char,
        word_flags: c_int,
        dictionary_flags: *mut u32,
    ) -> c_int;
    pub(super) fn SetTranslator3(name: *const c_char) -> c_int;
    #[allow(clippy::too_many_arguments)]
    pub(super) fn espeak_rs_lookup_dict(
        tr: *mut c_void,
        word: *const c_char,
        next: *const c_char,
        phonemes: *mut c_char,
        flags: *mut u32,
        end_flags: c_int,
        words: *mut c_void,
        remaining: c_int,
    ) -> *const c_char;
    pub(super) fn AppendPhonemes(
        tr: *mut c_void,
        string: *mut c_char,
        size: c_int,
        phonemes: *const c_char,
    );
    pub(super) fn SetWordStress(
        tr: *mut c_void,
        output: *mut c_char,
        dictionary_flags: *mut u32,
        tonic: c_int,
        control: c_int,
    );
}

/// `option_phonemes & espeakPHONEMES_TRACE`.
pub(super) fn tracing() -> bool {
    // SAFETY: serialized engine scalar.
    unsafe { option_phonemes & 0x08 != 0 }
}
/// The bytes of a terminated string, without its NUL.
///
/// # Safety
/// `text` is a terminated string that outlives the borrow.
pub(super) unsafe fn text<'a>(text: *const u8) -> &'a [u8] {
    // SAFETY: caller contract.
    unsafe { std::ffi::CStr::from_ptr(text.cast()) }.to_bytes()
}
/// Writes `parts` to the trace stream `f_trans`, as one `fprintf` did.
pub(super) fn trace(parts: &[&[u8]]) {
    // SAFETY: the engine's trace stream, live while set; serialized engine.
    let file = unsafe { f_trans };
    if !file.is_null() {
        // SAFETY: a live C stream.
        unsafe { crate::crt::write(file, &parts.concat()) };
    }
}
/// Writes `parts` to standard error, as one `fprintf(stderr, ...)` did.
pub(super) fn error(parts: &[&[u8]]) {
    // SAFETY: the C runtime's standard error stream.
    unsafe { crate::crt::write(crate::crt::standard_error(), &parts.concat()) };
}
