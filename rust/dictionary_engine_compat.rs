//! The dictionary loader, suffix removal and letter test against the engine
//! (from dictionary.c): `LoadDictionary`, `FreeDictionaryCache`,
//! `RemoveEnding` and `IsVowel`.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::engine_c as c;
use crate::{
    crt::{PATH_BUFFER, PATH_SEPARATOR},
    dictionary_storage::Cache,
    translator::Translator,
};
use std::{
    ffi::{c_char, c_int},
    ptr,
};

/// The dictionaries loaded so far, shared by every translator.
static mut CACHE: *mut Cache = ptr::null_mut();

/// `LoadDictionary`: 0 when loaded, 1 when the file cannot be read, 2 for bad
/// data, 3 when out of memory.
///
/// # Safety
/// `tr` is a live translator; `name` is terminated; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_load_dictionary(
    tr: *mut Translator,
    name: *const c_char,
    no_error: c_int,
) -> c_int {
    if tr.is_null() || name.is_null() {
        return 2;
    }
    // SAFETY: caller's terminated name, copied before anything is written.
    let requested = unsafe { c::text(name.cast()) }.to_vec();
    // The global names the requested dictionary even when loading fails, as
    // in the C path: the compiler compiles it, and voice loading treats an
    // empty name as "no dictionary" and deletes the translator. The
    // translator's own name stays with its last valid data.
    let global = ptr::addr_of_mut!(c::dictionary_name).cast::<u8>();
    if global.cast_const() != name.cast() {
        let length = requested.len().min(39);
        // SAFETY: the 40-byte global; the copy came from the caller's name.
        unsafe {
            ptr::copy_nonoverlapping(requested.as_ptr(), global, length);
            global.add(length).write(0);
        }
    }
    // SAFETY: the engine's terminated data directory.
    let home = unsafe { c::text(ptr::addr_of!(c::path_home).cast()) };
    let mut path = [home, &[PATH_SEPARATOR], &requested, b"_dict"].concat();
    path.truncate(PATH_BUFFER - 1);
    let shown = path.clone();
    path.push(0);
    // SAFETY: serialized engine; the cache is made once and kept.
    let cache = unsafe {
        if CACHE.is_null() {
            CACHE = super::espeak_rs_dictionary_cache_create();
        }
        CACHE
    };
    if cache.is_null() {
        return 3;
    }
    // SAFETY: live translator and cache, terminated path and name.
    let status = unsafe {
        super::translator_compat::espeak_rs_translator_load(tr, cache, path.as_ptr().cast(), name)
    };
    if status != 0 {
        if status == 1 && no_error == 0 {
            c::error(&[b"Can't read dictionary file: '", &shown, b"'\n"]);
        }
        if status == 2 {
            c::error(&[b"Bad dictionary data: '", &shown, b"'\n"]);
        }
        return if status == 4 { 2 } else { status };
    }
    // SAFETY: the live translator, now bound to its dictionary.
    let (minimum, size) = unsafe { ((*tr).dict_min_size, (*tr).data_dict_size) };
    if minimum > 0 && size < minimum as u32 as usize {
        c::error(&[
            b"Full dictionary is not installed for '",
            &requested,
            b"'\n",
        ]);
    }
    0
}

/// `FreeDictionaryCache`.
///
/// # Safety
/// No translator uses a cached dictionary; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_free_dictionary_cache() {
    // SAFETY: caller contract; the cache was made by the loader.
    unsafe {
        super::espeak_rs_dictionary_cache_destroy(CACHE);
        CACHE = ptr::null_mut();
    }
}

/// `IsLetter`: whether `letter` is in the translator's letter `group`.
///
/// # Safety
/// `tr` is null or a live translator.
#[no_mangle]
unsafe extern "C" fn espeak_rs_translator_is_letter(
    tr: *const Translator,
    letter: c_int,
    group: c_int,
) -> c_int {
    if tr.is_null() {
        return 0;
    }
    // SAFETY: the live translator's letter tables, borrowed for the call.
    unsafe {
        let letters = (*tr).match_letters();
        super::espeak_rs_is_letter(&letters, letter, group as u32)
    }
}

/// `RemoveEnding`: removes the suffix `end_type` names from `word`, keeping
/// the original in `word_copy`; returns the suffix flags.
///
/// # Safety
/// As the C function; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_remove_ending_engine(
    tr: *mut Translator,
    word: *mut u8,
    end_type: c_int,
    word_copy: *mut u8,
) -> c_int {
    if tr.is_null() || word.is_null() {
        return 0;
    }
    // N_WORD_BYTES limits the original copy, not the word being shortened.
    // SAFETY: caller's terminated word.
    let length = unsafe { c::text(word) }.len() + 1;
    // SAFETY: the live translator; scalars copied out.
    let (language, added, expect_verb, base, extent) = unsafe {
        let tr = &*tr;
        (
            tr.translator_name as u32,
            tr.langopts.suffix_add_e,
            tr.expect_verb,
            tr.rule_text_base as usize,
            tr.rule_text_length,
        )
    };
    let mut context = crate::suffix::Context {
        language,
        added_character: added,
        expect_verb,
        signed_bytes: u32::from(c_char::MIN != 0),
        preceding: [b' '; 4],
    };
    let address = word as usize;
    let history = base != 0 && address >= base && address - base >= 3 && address - base < extent;
    if history {
        // SAFETY: the three bytes before the word lie in the rule text.
        unsafe { ptr::copy_nonoverlapping(word.sub(3), context.preceding.as_mut_ptr(), 3) };
    }
    // SAFETY: the live translator's letter tables, borrowed for the call.
    let letters = unsafe { (*tr).match_letters() };
    let mut effects = crate::suffix::Effects {
        flags: 0,
        expect_verb: 0,
        added: 0,
        preceding: 0,
    };
    // SAFETY: caller's word and copy; the descriptors outlive the call.
    if unsafe {
        super::espeak_rs_remove_ending(
            word,
            length,
            end_type as u32,
            &context,
            &letters,
            word_copy,
            &mut effects,
        )
    } != 0
    {
        return 0;
    }
    // SAFETY: the live translator, and the rule-text byte before the word.
    unsafe {
        (*tr).expect_verb = effects.expect_verb;
        if effects.preceding != 0 && history {
            word.sub(1).write((effects.preceding - 1) as u8);
        }
    }
    if effects.added != 0 && c::tracing() {
        c::trace(&[b"add e\n"]);
    }
    effects.flags as c_int
}
