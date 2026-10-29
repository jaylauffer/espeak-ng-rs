//! The voice catalog, selection and voice-file primitives against the engine
//! (from voices.c): listing, selection by properties, the current-voice
//! record, ordinal options and phoneme replacement. voices.c keeps the voice
//! list and current-voice record as data the remaining bridges also read.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]
use super::{
    clause_postprocess_compat as post, engine_c as c, synthdata_compat as synth, ForeignSelection,
    ForeignVoice,
};
use crate::crt::{PATH_BUFFER, PATH_SEPARATOR};
use std::{
    ffi::{c_char, c_int, c_void},
    ptr::{self, addr_of, addr_of_mut},
};

/// `N_VOICES_LIST` (voices.c).
const VOICES: usize = 500;

unsafe extern "C" {
    pub(super) static mut n_voices_list: c_int;
    pub(super) static mut voices_list: [*mut ForeignVoice; VOICES];
    static mut rust_voice_workspace: *mut c_void;
    static mut rust_voice_catalog: *mut c_void;
    pub(super) static mut rust_current_voice: *mut c_void;
    pub(super) static mut current_voice_selected: ForeignVoice;
    pub(super) fn espeak_ListVoices(spec: *mut ForeignVoice) -> *const *const ForeignVoice;
}

/// The bytes of `text`, at most `length`, up to a NUL (`%.*s`).
///
/// # Safety
/// `text` has `length` readable bytes or a NUL before them.
unsafe fn prefix<'a>(text: *const u8, length: usize) -> &'a [u8] {
    let mut end = 0;
    // SAFETY: caller contract.
    while end < length && unsafe { *text.add(end) } != 0 {
        end += 1;
    }
    // SAFETY: the bytes just read.
    unsafe { std::slice::from_raw_parts(text, end) }
}

/// `GetFileLength` of a voice path (the request and native hosts' `length`).
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_voice_length(
    _: *mut c_void,
    path: *const u8,
    _: usize,
) -> i64 {
    // SAFETY: the host's terminated path.
    i64::from(unsafe { super::espeak_rs_file_length(path.cast()) })
}
unsafe extern "C" fn catalog_diagnostic(
    _: *mut c_void,
    kind: u32,
    identifier: *const u8,
    length: usize,
) {
    // SAFETY: the catalog's identifier bytes.
    let identifier = unsafe { prefix(identifier, length) };
    match kind {
        1 => c::error(&[
            b"Error (",
            identifier,
            b"): gender attribute specified on a language file\n",
        ]),
        2 => c::error(&[b"Invalid voice metadata: ", identifier, b"\n"]),
        3 => c::error(&[format!(
            "Warning: maximum number {} of (N_VOICES_LIST = {VOICES} - 1) reached\n",
            VOICES - 1
        )
        .as_bytes()]),
        _ => {}
    }
}
/// Whether `path_home/voices/<language>` is a directory.
unsafe extern "C" fn voice_directory(_: *mut c_void, language: *const u8, length: usize) -> u32 {
    // SAFETY: the engine's data path and the catalog's language bytes.
    let mut path = unsafe {
        [
            c::text(addr_of!(c::path_home).cast()),
            b"/voices/",
            prefix(language, length),
        ]
        .concat()
    };
    path.truncate(PATH_BUFFER - 1);
    path.push(0);
    // SAFETY: the terminated path.
    u32::from(unsafe { super::espeak_rs_file_length(path.as_ptr().cast()) } == -libc::EISDIR)
}

/// `FreeVoiceList`.
///
/// # Safety
/// No catalog view is in use; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_free_voice_list() {
    // SAFETY: caller contract.
    unsafe {
        rust_voice_workspace = ptr::null_mut();
        super::espeak_rs_voice_catalog_destroy(rust_voice_catalog);
        rust_voice_catalog = ptr::null_mut();
        *addr_of_mut!(voices_list) = [ptr::null_mut(); VOICES];
        n_voices_list = 0;
    }
}
/// `espeak_ListVoices`: the voices matching `spec` (all when it is null).
///
/// # Safety
/// `spec` is null or a voice description; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_list_voices_engine(
    spec: *const ForeignVoice,
) -> *const *const ForeignVoice {
    // SAFETY: serialized engine state and the engine's data path.
    unsafe {
        espeak_rs_free_voice_list();
        rust_voice_catalog = super::espeak_rs_voice_catalog_create(
            addr_of!(c::path_home),
            addr_of_mut!(voices_list).cast(),
            VOICES,
            addr_of_mut!(n_voices_list),
            ptr::null_mut(),
            Some(catalog_diagnostic),
        );
        if rust_voice_catalog.is_null() {
            return ptr::null();
        }
        rust_voice_workspace = super::espeak_rs_voice_catalog_workspace(rust_voice_catalog).cast();
        super::espeak_rs_voice_catalog_list(
            rust_voice_catalog,
            spec,
            PATH_SEPARATOR,
            ptr::null_mut(),
            Some(voice_directory),
        )
        .cast_const()
        .cast()
    }
}
/// The identifier `SelectVoice` returns when it adds a variant.
static mut SELECTED: [u8; 50] = [0; 50];
/// `SelectVoice`: the identifier of the best voice for `spec`.
///
/// # Safety
/// `spec` is a voice description and `found` writable; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_select_voice_engine(
    spec: *const ForeignVoice,
    found: *mut c_int,
) -> *const c_char {
    // SAFETY: serialized engine state and the caller's description.
    unsafe {
        if n_voices_list == 0 {
            espeak_ListVoices(ptr::null_mut());
        }
        if rust_voice_workspace.is_null() {
            espeak_ListVoices(ptr::null_mut());
        }
        if rust_voice_workspace.is_null() {
            *found = 0;
            return ptr::null();
        }
        let selector = *spec;
        let mut selection: ForeignSelection = std::mem::zeroed();
        let status = super::espeak_rs_voice_select(
            rust_voice_workspace.cast(),
            &selector,
            addr_of!(voices_list).cast(),
            PATH_SEPARATOR,
            ptr::null_mut(),
            Some(voice_directory),
            &mut selection,
        );
        if status == 2 {
            *found = 0;
            return ptr::null();
        }
        *found = selection.found as c_int;
        if status == 1 || selection.index >= n_voices_list as usize {
            return ptr::null();
        }
        let base = (*(*addr_of!(voices_list))[selection.index]).identifier;
        if selection.suffix[0] == 0 {
            return base;
        }
        let text = [
            c::text(base.cast()),
            b"+",
            prefix(selection.suffix.as_ptr(), selection.suffix.len()),
        ]
        .concat();
        let selected = &mut *addr_of_mut!(SELECTED);
        if text.len() >= selected.len() {
            *found = 0;
            return ptr::null();
        }
        selected[..text.len()].copy_from_slice(&text);
        selected[text.len()] = 0;
        selected.as_ptr().cast()
    }
}
/// `FreeCurrentVoice`.
///
/// # Safety
/// No view of the current voice is in use; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_free_current_voice() {
    // SAFETY: caller contract.
    unsafe {
        if super::voice_native_compat::espeak_rs_voice_native_clear(
            addr_of_mut!(rust_current_voice).cast(),
        ) != 0
        {
            return;
        }
        let selected = &mut *addr_of_mut!(current_voice_selected);
        selected.identifier = ptr::null();
        selected.name = ptr::null();
        selected.languages = ptr::null();
    }
}
/// `espeak_GetCurrentVoice`.
#[no_mangle]
extern "C" fn espeak_rs_current_voice() -> *mut ForeignVoice {
    addr_of_mut!(current_voice_selected)
}

unsafe extern "C" fn ordinal_environment(
    opaque: *mut c_void,
    kind: u32,
    key: u32,
    _: *const u8,
    _: usize,
    number: i32,
) -> i32 {
    if kind == 1 {
        // SAFETY: the caller's mnemonic table.
        let name = unsafe { mnemonic(opaque.cast_const(), key as c_int) };
        c::error(&[
            &name,
            b": Bad option number ",
            number.to_string().as_bytes(),
            b"\n",
        ]);
    }
    -1
}
/// `LookupMnemName`'s text, as `%s` printed it.
unsafe fn mnemonic(table: *const c_void, key: c_int) -> Vec<u8> {
    // SAFETY: the caller's terminated mnemonic table.
    let name = unsafe { super::LookupMnemName(table.cast(), key) };
    if name.is_null() {
        b"(null)".to_vec()
    } else {
        // SAFETY: the table's terminated name.
        unsafe { c::text(name.cast()) }.to_vec()
    }
}
/// `ReadNumbers`: the ordinal options in `text` into `flags`.
///
/// # Safety
/// As the C function.
#[no_mangle]
unsafe extern "C" fn espeak_rs_read_numbers_engine(
    text: *const c_char,
    flags: *mut c_int,
    maximum: c_int,
    table: *const c_void,
    key: c_int,
) {
    // SAFETY: caller contract.
    unsafe {
        if super::espeak_rs_language_ordinals(
            text,
            flags.cast(),
            maximum,
            key as u32,
            table.cast_mut(),
            Some(ordinal_environment),
        ) != 0
        {
            let name = mnemonic(table, key);
            c::error(&[b"Invalid option ordinals: ", &name, b"\n"]);
        }
    }
}
/// `PhonemeReplacement`: a voice file's `replace` directive.
///
/// # Safety
/// `text` is terminated; serialized engine.
pub(super) unsafe fn replacement(text: *const c_char) {
    // SAFETY: the engine's phoneme table and replacement storage.
    unsafe {
        if super::espeak_rs_voice_replacement(
            text,
            addr_of!(synth::phoneme_tab).cast(),
            synth::n_phoneme_tab as usize,
            addr_of_mut!(post::replace_phonemes).cast(),
            addr_of_mut!(post::n_replace_phonemes),
        ) != 0
        {
            c::error(&[b"Invalid phoneme replacement directive\n"]);
        }
    }
}
