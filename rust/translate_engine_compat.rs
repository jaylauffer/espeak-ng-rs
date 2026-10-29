//! The clause and word translators' hosts against the engine (from
//! translate.c): the alternate translators, word replacement, text control,
//! word-to-list, character conversion and clause hosts the Rust translators
//! call, and the public entry points that wrap them. translate.c keeps the
//! clause state as data its few remaining bridges also read.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]
use super::{
    alternate_translator_compat as alternate, character_conversion_compat as character,
    clause_postprocess_compat as post, engine_c as c, replacement_compat as replacement,
    synthdata_compat as synth, text_control_compat as control, translate_clause_compat as clause,
    translate_word2_compat as word2, WChar,
};
use crate::{
    character_conversion::{Call, ResultCode},
    phoneme::Phoneme,
    phoneme_list::Source as Entry,
    text_control::ResultValue,
    translate_clause::{Read, Row},
    translate_word2::Phoneme as Word2Phoneme,
    translator::Translator,
};
use std::{
    ffi::{c_char, c_int, c_uint, c_void},
    ptr::{self, addr_of, addr_of_mut},
};

// translate.h and synthesize.h.
const WORD_BYTES: usize = 160;
const WORD_PHONEMES: usize = 200;
const SOURCE_BYTES: usize = 800; // N_TR_SOURCE
const SOURCE: usize = SOURCE_BYTES + 40; // sizeof(source)
const CLAUSE_WORDS: usize = 300;
const EMBEDDED_LIST: usize = 250;
const EMBEDDED_VALUES: usize = 15;
const PHONEME_LIST: usize = 1000;
const MARKER: usize = 50; // N_MARKER_LENGTH
const COMBINE_WORDS: usize = 9;
const CAPS_IN_WORD: usize = 10;
const APOSTROPHE: usize = 16;
const TEXT_MODE: u32 = 0x2000_0000;
const VOWEL: u8 = 2; // phVOWEL

unsafe extern "C" {
    static mut translator2_language: [u8; 20];
    static mut translator3_language: [u8; 20];
    static mut option_sayas2: c_int;
    static mut option_emphasis: c_int;
    static mut option_capitals: c_int;
    static mut option_endpause: c_int;
    static mut option_phoneme_input: c_int;
    static mut count_sayas_digits: c_int;
    static mut skip_sentences: c_int;
    static mut skip_words: c_int;
    static mut skip_characters: c_int;
    static mut skip_marker: [u8; MARKER];
    static mut skipping_text: bool;
    static mut end_character_position: c_int;
    static mut count_sentences: c_int;
    static mut count_words: c_int;
    static mut clause_start_char: c_int;
    static mut clause_start_word: c_int;
    static mut new_sentence: bool;
    static mut word_emphasis: c_int;
    static mut embedded_flag: c_int;
    static mut max_clause_pause: c_int;
    static mut any_stressed_words: bool;
    static mut current_alphabet: *mut c_void;
    static mut embedded_ix: c_int;
    static mut embedded_read: c_int;
    static mut embedded_list: [c_uint; EMBEDDED_LIST];
    static mut count_characters: c_int;
    /// The clause's source text (`source` in translate.c).
    static mut espeak_clause_source: [u8; SOURCE];
    fn SelectTranslator(name: *const c_char) -> *mut c_void;
    fn DeleteTranslator(tr: *mut c_void);
    fn LoadDictionary(tr: *mut c_void, name: *const c_char, no_error: c_int) -> c_int;
    fn SelectPhonemeTableName(name: *const c_char) -> c_int;
    fn LookupPhonemeTable(name: *const c_char) -> c_int;
    fn TranslateWord3(
        tr: *mut c_void,
        word: *mut c_char,
        rows: *mut c_void,
        remaining: c_int,
        word_out: *mut c_char,
        any_stressed: *mut bool,
        alphabet: *mut c_void,
        phonemes: *mut c_char,
        size: usize,
    ) -> c_int;
    fn towlower2(code: c_uint, tr: *mut c_void) -> c_int;
    fn IsAlpha(code: c_uint) -> c_int;
    fn IsSpace(code: c_uint) -> c_int;
    fn IsBracket(code: c_int) -> c_int;
    fn IsEmoji(code: c_uint) -> c_int;
    fn IsEmojiModifier(code: c_uint) -> c_int;
    fn IsRegionalIndicator(code: c_uint) -> c_int;
    fn IsEmojiTag(code: c_uint) -> c_int;
    fn isspace2(code: c_uint) -> c_int;
    fn IsDigit(code: c_uint) -> c_int;
    fn IsVowel(tr: *mut c_void, letter: c_int) -> c_int;
    fn EncodePhonemesBounded(
        text: *const c_char,
        output: *mut c_char,
        capacity: usize,
        bad: *mut c_int,
    ) -> *const c_char;
    fn InitText2();
    fn InitNamedata();
    fn ReadClause(
        tr: *mut c_void,
        buf: *mut c_char,
        charix: *mut i16,
        charix_top: *mut c_int,
        n_buf: c_int,
        tone_type: *mut c_int,
        voice_change: *mut c_char,
    ) -> c_int;
    fn Eof() -> c_int;
    fn MakePhonemeList(tr: *mut c_void, pause: c_int, start: bool);
}

fn source() -> *mut u8 {
    addr_of_mut!(espeak_clause_source).cast()
}
/// The text of a terminated buffer of `capacity` bytes.
///
/// # Safety
/// `text` has `capacity` readable bytes or a NUL before them.
unsafe fn terminated(text: *const u8, capacity: usize) -> Option<usize> {
    // SAFETY: caller contract.
    let end = unsafe { libc::memchr(text.cast(), 0, capacity) };
    (!end.is_null()).then(|| end as usize - text as usize)
}
/// `wcschr(list, code) != NULL` for a translator's wide list.
///
/// # Safety
/// `list` is a terminated wide string.
unsafe fn contains(list: *const WChar, code: u32) -> bool {
    if list.is_null() {
        return false;
    }
    // wcschr finds the terminator itself for code 0.
    let code = code as WChar;
    let mut index = 0;
    loop {
        // SAFETY: caller contract.
        let value = unsafe { *list.add(index) };
        if value == code {
            return true;
        }
        if value == 0 {
            return false;
        }
        index += 1;
    }
}
/// `TranslateWordWithBounds`'s rule-source swap: `tr` reads `base` for
/// the call, as the C hosts did, then its own text again.
///
/// # Safety
/// `tr` is live; `base` holds `length` bytes.
unsafe fn with_rule_source<T>(
    tr: *mut Translator,
    base: *const u8,
    length: usize,
    call: impl FnOnce() -> T,
) -> T {
    // SAFETY: caller contract.
    unsafe {
        let saved = ((*tr).rule_text_base, (*tr).rule_text_length);
        (*tr).rule_text_base = base;
        (*tr).rule_text_length = length;
        let result = call();
        ((*tr).rule_text_base, (*tr).rule_text_length) = saved;
        result
    }
}

// The alternate translators (RustAlternateTranslator).
fn alternate_slot(slot: u32) -> *mut *mut c_void {
    match slot {
        0 => addr_of_mut!(c::translator2),
        1 => addr_of_mut!(c::translator3),
        _ => ptr::null_mut(),
    }
}
fn alternate_cache(slot: u32) -> *mut u8 {
    match slot {
        0 => addr_of_mut!(translator2_language).cast(),
        1 => addr_of_mut!(translator3_language).cast(),
        _ => ptr::null_mut(),
    }
}
unsafe extern "C" fn alternate_present(_: *mut c_void, slot: u32) -> i32 {
    let p = alternate_slot(slot);
    // SAFETY: a translator slot.
    i32::from(!p.is_null() && !unsafe { *p }.is_null())
}
unsafe extern "C" fn alternate_cached(_: *mut c_void, slot: u32, index: usize) -> i32 {
    let p = alternate_cache(slot);
    if p.is_null() || index >= 20 {
        return -1;
    }
    // SAFETY: in bounds of the 20-byte name.
    i32::from(unsafe { *p.add(index) })
}
unsafe extern "C" fn alternate_write(_: *mut c_void, slot: u32, index: usize, byte: u8) -> i32 {
    let p = alternate_cache(slot);
    if p.is_null() || index >= 20 {
        return -1;
    }
    // SAFETY: in bounds of the 20-byte name.
    unsafe { *p.add(index) = byte };
    0
}
unsafe extern "C" fn alternate_lookup(_: *mut c_void, name: *const u8) -> i32 {
    // SAFETY: serialized engine; the host's terminated name.
    unsafe { SelectPhonemeTableName(name.cast()) }
}
unsafe extern "C" fn alternate_create(_: *mut c_void, slot: u32, name: *const u8) -> i32 {
    let p = alternate_slot(slot);
    // SAFETY: a translator slot; the host's terminated name.
    unsafe {
        if p.is_null() || !(*p).is_null() {
            return -1;
        }
        *p = SelectTranslator(name.cast());
        if (*p).is_null() {
            -1
        } else {
            0
        }
    }
}
unsafe extern "C" fn alternate_load(_: *mut c_void, slot: u32) -> i32 {
    let p = alternate_slot(slot);
    // SAFETY: a translator slot and its live translator.
    unsafe {
        if p.is_null() || (*p).is_null() {
            return -1;
        }
        let tr = *p;
        LoadDictionary(
            tr,
            addr_of!((*tr.cast::<Translator>()).dictionary_name).cast(),
            0,
        )
    }
}
unsafe extern "C" fn alternate_main_table(_: *mut c_void) -> i32 {
    // SAFETY: serialized engine voice.
    unsafe {
        let voice = c::voice.cast::<crate::voice::Voice>();
        if voice.is_null() {
            -1
        } else {
            (*voice).phoneme_table
        }
    }
}
unsafe extern "C" fn alternate_effect(_: *mut c_void, slot: u32, effect: u32, value: i32) -> i32 {
    let p = alternate_slot(slot);
    if p.is_null() {
        return -1;
    }
    // SAFETY: a translator slot and its live translator.
    unsafe {
        match effect {
            0 => DeleteTranslator(*p),
            1 => *p = ptr::null_mut(),
            2 | 3 => {
                let tr = (*p).cast::<Translator>();
                if tr.is_null() {
                    return -1;
                }
                if effect == 2 {
                    (*tr).phoneme_tab_ix = value;
                } else {
                    (*tr).phonemes_repeat[0] = 0;
                }
            }
            4 => synth::SelectPhonemeTable(value),
            _ => return -1,
        }
    }
    0
}
fn alternate_table(context: *mut c_void) -> alternate::Callbacks {
    alternate::Callbacks {
        context,
        present: Some(alternate_present),
        cached: Some(alternate_cached),
        cache: Some(alternate_write),
        lookup: Some(alternate_lookup),
        create: Some(alternate_create),
        load: Some(alternate_load),
        main_table: Some(alternate_main_table),
        effect: Some(alternate_effect),
    }
}
/// `SetTranslator2` (`slot` 0) and `SetTranslator3` (`slot` 1).
///
/// # Safety
/// `name` is null or terminated; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_select_alternate(slot: u32, name: *const c_char) -> c_int {
    if name.is_null() {
        return -1;
    }
    let mut length = 0;
    // SAFETY: the caller's name, read to its NUL or 20 bytes.
    while length < 20 && unsafe { *name.add(length) } != 0 {
        length += 1;
    }
    if length >= 20 {
        return -1;
    }
    let mut context = slot;
    let table = alternate_table(addr_of_mut!(context).cast());
    let mut out = -1;
    // SAFETY: the table outlives the call.
    unsafe {
        alternate::espeak_rs_alternate_translator(&table, slot, name.cast(), length, &mut out)
    };
    out
}
/// `FreeAlternateTranslators`.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_free_alternates() {
    let mut context = 0u32;
    let table = alternate_table(addr_of_mut!(context).cast());
    let mut out = 0;
    // SAFETY: the table outlives the call.
    unsafe { alternate::espeak_rs_alternate_translator(&table, 2, ptr::null(), 0, &mut out) };
}

// Word replacement (RustReplacement).
struct ReplacementHost {
    tr: *mut Translator,
    word: *mut u8,
    text: *mut u8,
    rows: *mut Row,
    remaining: c_int,
}
fn replacement_host<'a>(opaque: *mut c_void) -> &'a mut ReplacementHost {
    // SAFETY: every callback receives the live host its entry made.
    unsafe { &mut *opaque.cast::<ReplacementHost>() }
}
unsafe extern "C" fn replacement_value(opaque: *mut c_void, field: u32) -> i32 {
    let h = replacement_host(opaque);
    // SAFETY: the live translator, its row and the engine's options.
    unsafe {
        match field {
            0 => i32::from((*h.tr).langopts.ideographs as c_char),
            1 => (*h.tr).langopts.alt_alphabet,
            2 => {
                if h.rows.is_null() {
                    0
                } else {
                    (*h.rows).flags as i32
                }
            }
            3 => c::dictionary_skipwords,
            _ => 0,
        }
    }
}
unsafe extern "C" fn replacement_store(opaque: *mut c_void, field: u32, value: i32) -> i32 {
    let h = replacement_host(opaque);
    // SAFETY: the live row and the engine's options.
    unsafe {
        if field == 2 && !h.rows.is_null() {
            (*h.rows).flags = value as u32;
        } else if field == 3 {
            c::dictionary_skipwords = value;
        } else {
            return -1;
        }
    }
    0
}
unsafe extern "C" fn replacement_class(opaque: *mut c_void, code: u32, kind: u32) -> i32 {
    let h = replacement_host(opaque);
    // SAFETY: pure classifiers; the live translator's terminated list.
    unsafe {
        match kind {
            0 => IsAlpha(code),
            1 => IsSpace(code),
            2 => i32::from(super::ucd_isupper(code) != 0),
            3 => i32::from(code < 128 && libc::isspace(code as c_int) != 0),
            4 => i32::from(contains((*h.tr).punct_within_word, code)),
            _ => 0,
        }
    }
}
unsafe extern "C" fn replacement_lower(opaque: *mut c_void, code: u32) -> u32 {
    // SAFETY: the live translator.
    unsafe { towlower2(code, replacement_host(opaque).tr.cast()) as u32 }
}
unsafe extern "C" fn replacement_main(
    opaque: *mut c_void,
    source: *mut u8,
    initialized: usize,
    offset: usize,
    text: *mut u8,
    flags: *mut u32,
) -> i32 {
    let h = replacement_host(opaque);
    let mut word = h.word;
    let mut destination = text;
    let swapped = !source.is_null();
    if swapped {
        if initialized > 3 + WORD_BYTES * 2 || offset >= initialized {
            return -1;
        }
        word = source.wrapping_add(offset);
    } else if !text.is_null() {
        destination = h.text;
    }
    let translate = || {
        // SAFETY: the live translator, its word and the engine's outputs.
        unsafe {
            TranslateWord3(
                h.tr.cast(),
                word.cast(),
                h.rows.cast(),
                h.remaining,
                destination.cast(),
                addr_of_mut!(any_stressed_words),
                current_alphabet,
                addr_of_mut!(c::word_phonemes).cast(),
                WORD_PHONEMES,
            )
        }
    };
    // SAFETY: the live translator; `source` holds `initialized` bytes.
    let result = unsafe {
        if swapped {
            with_rule_source(h.tr, source, initialized, translate)
        } else {
            translate()
        }
    };
    // SAFETY: the caller's flags and text buffers.
    unsafe {
        *flags = result as u32;
        if !swapped && !text.is_null() && result as u32 & TEXT_MODE != 0 {
            if destination.is_null() {
                return -1;
            }
            let Some(length) = terminated(destination, WORD_BYTES + 1) else {
                return -1;
            };
            ptr::copy(destination, text, length + 1);
        }
    }
    0
}
unsafe extern "C" fn replacement_snapshot(_: *mut c_void, out: *mut u8) -> i32 {
    // SAFETY: the engine's 200-byte word phonemes and the caller's buffer.
    unsafe {
        ptr::copy_nonoverlapping(addr_of!(c::word_phonemes).cast::<u8>(), out, WORD_PHONEMES)
    };
    0
}
unsafe extern "C" fn replacement_publish(_: *mut c_void, output: *const u8) -> i32 {
    // SAFETY: the caller's phonemes and the engine's 200-byte buffer.
    unsafe {
        let Some(length) = terminated(output, WORD_PHONEMES) else {
            return -1;
        };
        ptr::copy(
            output,
            addr_of_mut!(c::word_phonemes).cast::<u8>(),
            length + 1,
        );
    }
    0
}
/// `TranslateWordWithBounds`.
unsafe fn translate_word_with_bounds(
    tr: *mut Translator,
    word: *mut u8,
    rows: *mut Row,
    remaining: c_int,
    text: *mut u8,
    depth: c_int,
) -> c_int {
    if tr.is_null() || word.is_null() {
        return 0;
    }
    let mut host = ReplacementHost {
        tr,
        word,
        text,
        rows,
        remaining,
    };
    let table = replacement::Callbacks {
        context: addr_of_mut!(host).cast(),
        value: Some(replacement_value),
        store: Some(replacement_store),
        classify: Some(replacement_class),
        lower: Some(replacement_lower),
        main: Some(replacement_main),
        snapshot: Some(replacement_snapshot),
        publish: Some(replacement_publish),
    };
    let mut flags = 0;
    // SAFETY: the table and host outlive the call.
    if unsafe {
        replacement::espeak_rs_replacement(&table, u32::from(!text.is_null()), depth, &mut flags)
    } == 0
    {
        flags as c_int
    } else {
        0
    }
}
/// `TranslateWord`.
///
/// # Safety
/// As the C function; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_word_engine(
    tr: *mut Translator,
    word: *mut u8,
    rows: *mut Row,
    text: *mut u8,
) -> c_int {
    // SAFETY: forwarded caller contract.
    unsafe { translate_word_with_bounds(tr, word, rows, 0, text, 0) }
}

// Text control (RustTextControl).
struct TextHost {
    tr: *mut Translator,
    word: *mut u8,
    length: usize,
    base: *const u8,
    base_length: usize,
    rows: *mut Row,
    remaining: c_int,
    count: *mut u8,
    pronunciation: *mut u8,
    flags: *mut c_int,
}
impl TextHost {
    fn new() -> Self {
        Self {
            tr: ptr::null_mut(),
            word: ptr::null_mut(),
            length: 0,
            base: ptr::null(),
            base_length: 0,
            rows: ptr::null_mut(),
            remaining: 0,
            count: ptr::null_mut(),
            pronunciation: ptr::null_mut(),
            flags: ptr::null_mut(),
        }
    }
    /// `ControlWordBounds`: the rule text the word lies in, if any.
    fn bounds(&mut self) {
        // SAFETY: serialized engine translators.
        let owners = unsafe {
            [
                self.tr,
                c::translator.cast(),
                c::translator2.cast(),
                c::translator3.cast(),
            ]
        };
        for owner in owners {
            if owner.is_null() {
                continue;
            }
            // SAFETY: a live translator's rule-text extent.
            let (base, length) = unsafe { ((*owner).rule_text_base, (*owner).rule_text_length) };
            if base.is_null() {
                continue;
            }
            let at = self.word as usize;
            if at >= base as usize && at - (base as usize) < length {
                self.base = base;
                self.base_length = length;
                self.length = length - (at - base as usize);
                return;
            }
        }
    }
}
fn text_host<'a>(opaque: *mut c_void) -> &'a mut TextHost {
    // SAFETY: every callback receives the live host its entry made.
    unsafe { &mut *opaque.cast::<TextHost>() }
}
unsafe extern "C" fn control_value(opaque: *mut c_void, field: u32) -> i32 {
    let h = text_host(opaque);
    // SAFETY: serialized engine state and the host's live owners.
    unsafe {
        match field {
            0 => embedded_ix,
            1 => embedded_read,
            2 => c::option_sayas,
            3 => option_sayas2,
            4 => count_sayas_digits,
            5 => word_emphasis,
            6 => option_emphasis,
            7 => c::pre_pause,
            8 => {
                if h.tr.is_null() {
                    0
                } else {
                    (*h.tr).langopts.param[COMBINE_WORDS]
                }
            }
            9 => h.remaining,
            10 => c::dictionary_skipwords,
            11 => {
                if h.flags.is_null() {
                    0
                } else {
                    *h.flags
                }
            }
            12 => count_sentences,
            13 => count_words,
            14 => end_character_position,
            15 => skip_sentences,
            16 => skip_words,
            17 => skip_characters,
            18 => i32::from(skipping_text),
            19 => i32::from(new_sentence),
            20 => embedded_flag,
            21 if h.length <= 840 => h.length as i32,
            _ => -1,
        }
    }
}
unsafe extern "C" fn control_store(opaque: *mut c_void, field: u32, value: i32) -> i32 {
    let h = text_host(opaque);
    if (field == 0 || field == 1) && !(0..=EMBEDDED_LIST as i32).contains(&value) {
        return -1;
    }
    // SAFETY: serialized engine state and the host's flags.
    unsafe {
        match field {
            0 => embedded_ix = value,
            1 => embedded_read = value,
            2 => c::option_sayas = value,
            3 => option_sayas2 = value,
            4 => count_sayas_digits = value,
            5 => word_emphasis = value,
            6 => option_emphasis = value,
            7 => c::pre_pause = value,
            10 => c::dictionary_skipwords = value,
            11 => {
                if h.flags.is_null() {
                    return -1;
                }
                *h.flags = value;
            }
            12 => count_sentences = value,
            13 => count_words = value,
            14 => end_character_position = value,
            15 => skip_sentences = value,
            16 => skip_words = value,
            17 => skip_characters = value,
            18 => skipping_text = value != 0,
            19 => new_sentence = value != 0,
            20 => embedded_flag = value,
            _ => return -1,
        }
    }
    0
}
unsafe extern "C" fn control_byte(opaque: *mut c_void, kind: u32, index: usize) -> i32 {
    let h = text_host(opaque);
    // SAFETY: each buffer read in bounds.
    unsafe {
        match kind {
            0 if index < SOURCE => i32::from(*source().add(index)),
            1 if !h.word.is_null() && index < h.length => i32::from(*h.word.add(index)),
            2 if !h.pronunciation.is_null() && index < WORD_PHONEMES => {
                i32::from(*h.pronunciation.add(index))
            }
            3 if !h.count.is_null() && index < WORD_PHONEMES => i32::from(*h.count.add(index)),
            4 if index < MARKER => i32::from((*addr_of!(skip_marker))[index]),
            _ => -1,
        }
    }
}
unsafe extern "C" fn control_write(opaque: *mut c_void, kind: u32, index: usize, value: u8) -> i32 {
    let h = text_host(opaque);
    // SAFETY: each buffer written in bounds.
    unsafe {
        match kind {
            0 if index < SOURCE => *source().add(index) = value,
            1 if !h.word.is_null() && index < h.length => *h.word.add(index) = value,
            2 if !h.pronunciation.is_null() && index < WORD_PHONEMES => {
                *h.pronunciation.add(index) = value
            }
            4 if index < MARKER => (*addr_of_mut!(skip_marker))[index] = value,
            _ => return -1,
        }
    }
    0
}
unsafe extern "C" fn embedded_command(_: *mut c_void, index: i32, out: *mut u32) -> i32 {
    let Ok(index) = usize::try_from(index) else {
        return -1;
    };
    if index >= EMBEDDED_LIST {
        return -1;
    }
    // SAFETY: the engine's command list, in bounds; the caller's output.
    unsafe { *out = (*addr_of!(embedded_list))[index] };
    0
}
unsafe extern "C" fn embedded_put_command(_: *mut c_void, index: i32, value: u32) -> i32 {
    let Ok(index) = usize::try_from(index) else {
        return -1;
    };
    if index >= EMBEDDED_LIST {
        return -1;
    }
    // SAFETY: the engine's command list, in bounds.
    unsafe { (*addr_of_mut!(embedded_list))[index] = value };
    0
}
unsafe extern "C" fn control_default(_: *mut c_void, index: usize, out: *mut i32) -> i32 {
    if index >= EMBEDDED_VALUES {
        return -1;
    }
    // SAFETY: the engine's defaults, in bounds; the caller's output.
    unsafe { *out = (*addr_of!(super::wave_memory_compat::embedded_default)).0[index] };
    0
}
unsafe extern "C" fn control_alpha(_: *mut c_void, code: u32) -> i32 {
    i32::from(super::ucd_isalpha(code) != 0)
}
unsafe extern "C" fn control_lower(_: *mut c_void, code: u8) -> u8 {
    // SAFETY: the C converter of a byte.
    unsafe { libc::tolower(c_int::from(code)) as u8 }
}
unsafe extern "C" fn control_row(opaque: *mut c_void, index: usize, out: *mut u32) -> i32 {
    let h = text_host(opaque);
    if h.rows.is_null() || h.remaining < 0 || index >= h.remaining as usize {
        return -1;
    }
    // SAFETY: an admitted row; the caller's output.
    unsafe { *out = (*h.rows.add(index)).flags };
    0
}
unsafe extern "C" fn control_put_row(opaque: *mut c_void, index: usize, flags: u32) -> i32 {
    let h = text_host(opaque);
    if h.rows.is_null() || h.remaining < 0 || index >= h.remaining as usize {
        return -1;
    }
    // SAFETY: an admitted row.
    unsafe { (*h.rows.add(index)).flags = flags };
    0
}
unsafe extern "C" fn control_translate(
    opaque: *mut c_void,
    main: u32,
    offset: usize,
    row: usize,
    remaining: usize,
    out: *mut u32,
) -> i32 {
    let h = text_host(opaque);
    let tr = if main != 0 {
        // SAFETY: serialized engine translator.
        unsafe { c::translator.cast::<Translator>() }
    } else {
        h.tr
    };
    if tr.is_null()
        || h.word.is_null()
        || h.base.is_null()
        || h.base_length == 0
        || h.length > 840
        || h.rows.is_null()
        || h.remaining < 0
        || offset >= h.length
        || row >= h.remaining as usize
        || remaining > h.remaining as usize - row
    {
        return -1;
    }
    // SAFETY: the live translator, whose rule source is the host's for the call.
    unsafe {
        *out = with_rule_source(tr, h.base, h.base_length, || {
            translate_word_with_bounds(
                tr,
                h.word.add(offset),
                h.rows.add(row),
                remaining as c_int,
                ptr::null_mut(),
                0,
            )
        }) as u32;
    }
    0
}
unsafe extern "C" fn control_vowel(_: *mut c_void, code: u8, out: *mut i32) -> i32 {
    // SAFETY: the engine's phoneme table; the caller's output.
    unsafe {
        let ph = (*addr_of!(synth::phoneme_tab)).0[code as usize];
        if ph.is_null() {
            return -1;
        }
        *out = i32::from((*ph).kind == VOWEL);
    }
    0
}
unsafe extern "C" fn control_lookup(_: *mut c_void, name: *const u8, out: *mut i32) -> i32 {
    // SAFETY: the host's name, terminated within 12 bytes; the caller's output.
    unsafe {
        if terminated(name, 12).is_none() {
            return -1;
        }
        *out = LookupPhonemeTable(name.cast());
    }
    0
}
unsafe extern "C" fn control_select(_: *mut c_void, table: i32) -> i32 {
    // SAFETY: serialized engine.
    unsafe { synth::SelectPhonemeTable(table) };
    0
}
unsafe extern "C" fn control_init(_: *mut c_void, which: u32) -> i32 {
    // SAFETY: serialized engine.
    unsafe {
        match which {
            0 => InitText2(),
            1 => InitNamedata(),
            _ => return -1,
        }
    }
    0
}
/// One text-control operation (`espeak_rs_text_control`'s operations).
unsafe fn text_control(host: &mut TextHost, operation: u32, argument: usize) -> ResultValue {
    let table = control::Callbacks {
        context: (host as *mut TextHost).cast(),
        value: Some(control_value),
        store: Some(control_store),
        byte: Some(control_byte),
        write: Some(control_write),
        command: Some(embedded_command),
        put_command: Some(embedded_put_command),
        default: Some(control_default),
        alpha: Some(control_alpha),
        lower: Some(control_lower),
        row: Some(control_row),
        put_row: Some(control_put_row),
        translate: Some(control_translate),
        vowel: Some(control_vowel),
        lookup: Some(control_lookup),
        select: Some(control_select),
        init: Some(control_init),
    };
    let mut out = ResultValue { value: 0, index: 0 };
    // SAFETY: the table and host outlive the call.
    let status = unsafe { control::espeak_rs_text_control(&table, operation, argument, &mut out) };
    if status != 0 {
        out = ResultValue {
            value: 0,
            index: usize::MAX,
        };
    }
    out
}
/// `Word_EmbeddedCmd`.
unsafe fn word_embedded_command() {
    // SAFETY: serialized engine.
    unsafe { text_control(&mut TextHost::new(), 1, 0) };
}
/// `EmbeddedCommand`: the command at `*index`, advancing it.
unsafe fn embedded_command_at(index: &mut u32) -> c_int {
    // SAFETY: serialized engine.
    let out = unsafe { text_control(&mut TextHost::new(), 0, *index as usize) };
    if out.index == usize::MAX {
        return 0;
    }
    *index = out.index as u32;
    out.value as c_int
}
/// `CombineFlag`.
unsafe fn combine_flag(
    tr: *mut Translator,
    rows: *mut Row,
    remaining: c_int,
    word: *mut u8,
    flags: *mut c_int,
    count: *mut u8,
    pronunciation: *mut u8,
) {
    let mut host = TextHost {
        tr,
        word,
        rows,
        remaining,
        count,
        pronunciation,
        flags,
        ..TextHost::new()
    };
    host.bounds();
    // SAFETY: serialized engine; the host's live buffers.
    unsafe { text_control(&mut host, 2, 0) };
}
/// `SwitchLanguage`.
unsafe fn switch_language(word: *mut u8, pronunciation: *mut u8) {
    let mut host = TextHost {
        word,
        pronunciation,
        ..TextHost::new()
    };
    host.bounds();
    // SAFETY: serialized engine; the host's live buffers.
    unsafe { text_control(&mut host, 3, 0) };
}
/// `InitText`.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_init_text_engine(control: c_int) {
    // SAFETY: serialized engine.
    unsafe { text_control(&mut TextHost::new(), 4, control as c_uint as usize) };
}

// Word-to-list (RustTranslateWord2).
struct Word2Host {
    tr: *mut Translator,
    word: *mut u8,
    rows: *mut Row,
    remaining: c_int,
    source_length: usize,
}
fn word2_host<'a>(opaque: *mut c_void) -> &'a mut Word2Host {
    // SAFETY: every callback receives the live host its entry made.
    unsafe { &mut *opaque.cast::<Word2Host>() }
}
fn word_phonemes() -> *mut u8 {
    addr_of_mut!(c::word_phonemes).cast()
}
unsafe extern "C" fn word2_value(opaque: *mut c_void, field: u32) -> i32 {
    let h = word2_host(opaque);
    // SAFETY: the host's rows (one before, and after when remaining), the
    // live translator and the engine's state.
    unsafe {
        let tr = &*h.tr;
        match field {
            0 => i32::from((*h.rows).length),
            1 => i32::from((*h.rows).source),
            2 => (*h.rows).flags as i32,
            3 => (*h.rows.wrapping_sub(1)).flags as i32,
            4 => {
                if h.remaining > 1 {
                    (*h.rows.add(1)).flags as i32
                } else {
                    0
                }
            }
            5 => h.remaining,
            6 => post::n_ph_list2,
            7 => embedded_flag,
            8 => embedded_ix,
            9 => c::option_sayas,
            10 => option_capitals,
            11 => option_emphasis,
            12 => tr.prepause_timeout,
            13 => tr.end_stressed_vowel,
            14 => tr.prev_dict_flags[0],
            15 => tr.prev_dict_flags[1],
            16 => i32::from(any_stressed_words),
            17 => (*c::voice.cast::<crate::voice::Voice>()).phoneme_table,
            _ => 0,
        }
    }
}
unsafe extern "C" fn word2_store(opaque: *mut c_void, field: u32, value: i32) -> i32 {
    let h = word2_host(opaque);
    // SAFETY: the host's row, the live translator and the engine's state.
    unsafe {
        let tr = &mut *h.tr;
        match field {
            2 => (*h.rows).flags = value as u32,
            6 => {
                if !(0..PHONEME_LIST as i32).contains(&value) {
                    return -1;
                }
                post::n_ph_list2 = value;
            }
            7 => embedded_flag = value,
            8 => {
                if !(0..=EMBEDDED_LIST as i32).contains(&value) {
                    return -1;
                }
                embedded_ix = value;
            }
            12 => tr.prepause_timeout = value,
            13 => tr.end_stressed_vowel = value,
            14 => tr.prev_dict_flags[0] = value,
            15 => tr.prev_dict_flags[1] = value,
            16 => any_stressed_words = value != 0,
            _ => return -1,
        }
    }
    0
}
unsafe extern "C" fn word2_byte(opaque: *mut c_void, offset: usize, pronunciation: u32) -> i32 {
    let h = word2_host(opaque);
    let (base, limit) = if pronunciation != 0 {
        (word_phonemes(), WORD_PHONEMES)
    } else {
        (h.word, h.source_length)
    };
    if offset >= limit {
        return -1;
    }
    // SAFETY: in bounds.
    i32::from(unsafe { *base.add(offset) })
}
unsafe extern "C" fn word2_write(
    opaque: *mut c_void,
    offset: usize,
    byte: u8,
    pronunciation: u32,
) -> i32 {
    let h = word2_host(opaque);
    let (base, limit) = if pronunciation != 0 {
        (word_phonemes(), WORD_PHONEMES)
    } else {
        (h.word, h.source_length)
    };
    if offset >= limit {
        return -1;
    }
    // SAFETY: in bounds.
    unsafe { *base.add(offset) = byte };
    0
}
fn list_index(index: i32) -> Option<usize> {
    usize::try_from(index).ok().filter(|&i| i < PHONEME_LIST)
}
unsafe extern "C" fn list_entry(_: *mut c_void, index: i32, out: *mut Entry) -> i32 {
    let Some(index) = list_index(index) else {
        return -1;
    };
    // SAFETY: the engine's list, in bounds; the caller's output.
    unsafe { *out = (*addr_of!(post::ph_list2)).0[index] };
    0
}
unsafe extern "C" fn list_put(_: *mut c_void, index: i32, entry: *const Entry) -> i32 {
    let Some(index) = list_index(index) else {
        return -1;
    };
    // SAFETY: the engine's list, in bounds; the caller's entry.
    unsafe { (*addr_of_mut!(post::ph_list2)).0[index] = *entry };
    0
}
unsafe extern "C" fn word2_embedded(_: *mut c_void) -> i32 {
    // SAFETY: serialized engine.
    unsafe { word_embedded_command() };
    0
}
unsafe extern "C" fn word2_encode(opaque: *mut c_void, language: u32) -> i32 {
    let h = word2_host(opaque);
    // SAFETY: the host's word and the engine's word phonemes.
    unsafe {
        if language != 0 {
            switch_language(h.word, word_phonemes());
        } else {
            let mut bad = 0;
            if EncodePhonemesBounded(
                h.word.cast(),
                word_phonemes().cast(),
                WORD_PHONEMES,
                &mut bad,
            )
            .is_null()
            {
                return -1;
            }
        }
    }
    0
}
unsafe extern "C" fn word2_translate(
    opaque: *mut c_void,
    alternate: u32,
    mode: u32,
    replacement: *mut u8,
    flags: *mut u32,
) -> i32 {
    let h = word2_host(opaque);
    // SAFETY: serialized engine translators.
    let tr = unsafe {
        if alternate != 0 {
            c::translator2
        } else {
            c::translator
        }
    }
    .cast::<Translator>();
    if tr.is_null() {
        return -1;
    }
    // SAFETY: the live translator, the host's word and rows, the caller's
    // replacement text (WORD_BYTES+3 bytes) and flags.
    unsafe {
        *flags = if mode != 0 {
            with_rule_source(tr, replacement, WORD_BYTES + 3, || {
                translate_word_with_bounds(
                    tr,
                    replacement.add(1),
                    h.rows,
                    h.remaining,
                    ptr::null_mut(),
                    0,
                )
            })
        } else {
            translate_word_with_bounds(tr, h.word, h.rows, h.remaining, replacement.add(2), 0)
        } as u32;
    }
    0
}
unsafe extern "C" fn word2_combine(opaque: *mut c_void, flags: *mut u32) -> i32 {
    let h = word2_host(opaque);
    // SAFETY: the host's owners and the engine's word phonemes.
    unsafe {
        let mut result = *flags as c_int;
        combine_flag(
            h.tr,
            h.rows,
            h.remaining,
            h.word,
            &mut result,
            word_phonemes(),
            word_phonemes(),
        );
        *flags = result as u32;
    }
    0
}
unsafe extern "C" fn word2_dictionary(_: *mut c_void, out: *mut u8) -> i32 {
    // SAFETY: the engine's 40-byte dictionary name and the caller's buffer.
    unsafe {
        let name = addr_of!(c::dictionary_name).cast::<u8>();
        let Some(length) = terminated(name, 40) else {
            return -1;
        };
        ptr::copy_nonoverlapping(name, out, length + 1);
    }
    0
}
unsafe extern "C" fn word2_restore_dictionary(_: *mut c_void, name: *const u8) -> i32 {
    // SAFETY: the caller's name and the engine's 40-byte dictionary name.
    unsafe {
        let Some(length) = terminated(name, 40) else {
            return -1;
        };
        ptr::copy(
            name,
            addr_of_mut!(c::dictionary_name).cast::<u8>(),
            length + 1,
        );
    }
    0
}
unsafe extern "C" fn word2_alternate(_: *mut c_void, name: *const u8, table: *mut i32) -> i32 {
    // SAFETY: the host's terminated name and output.
    unsafe {
        let name = if *name != 0 {
            name.cast()
        } else {
            c"en".as_ptr()
        };
        *table = espeak_rs_select_alternate(0, name);
    }
    0
}
unsafe extern "C" fn word2_select(_: *mut c_void, table: i32) -> i32 {
    // SAFETY: serialized engine.
    unsafe { synth::SelectPhonemeTable(table) };
    0
}
unsafe extern "C" fn word2_phoneme(_: *mut c_void, code: u8, out: *mut Word2Phoneme) -> i32 {
    // SAFETY: the engine's phoneme table; the caller's output.
    unsafe {
        let ph: *const Phoneme = (*addr_of!(synth::phoneme_tab)).0[code as usize];
        if ph.is_null() {
            return -1;
        }
        *out = Word2Phoneme {
            kind: i32::from((*ph).kind),
            length: i32::from((*ph).standard_length),
            program: u32::from((*ph).program),
        };
    }
    0
}
unsafe extern "C" fn word2_alpha(_: *mut c_void, code: u8) -> i32 {
    // SAFETY: pure classifier.
    unsafe { IsAlpha(u32::from(code)) }
}
unsafe extern "C" fn word2_invalid(_: *mut c_void, code: u8) {
    // SAFETY: the C runtime's standard output.
    unsafe {
        crate::crt::write(
            crate::crt::standard_output(),
            format!("Invalid phoneme code {code}\n").as_bytes(),
        );
    }
}
/// `TranslateWord2`.
unsafe fn translate_word2(
    tr: *mut Translator,
    word: *mut u8,
    rows: *mut Row,
    remaining: c_int,
    pause: c_int,
) -> c_int {
    if tr.is_null() || word.is_null() || rows.is_null() {
        return 0;
    }
    let mut host = Word2Host {
        tr,
        word,
        rows,
        remaining,
        source_length: 0,
    };
    // SAFETY: serialized engine translators.
    let owners = unsafe {
        [
            tr,
            c::translator.cast(),
            c::translator2.cast(),
            c::translator3.cast(),
        ]
    };
    for owner in owners {
        if owner.is_null() {
            continue;
        }
        // SAFETY: a live translator's rule-text extent.
        let (base, length) = unsafe { ((*owner).rule_text_base, (*owner).rule_text_length) };
        if base.is_null() {
            continue;
        }
        let address = word as usize;
        if address >= base as usize && address - (base as usize) < length {
            host.source_length = length - (address - base as usize);
            break;
        }
    }
    if host.source_length == 0 {
        // SAFETY: the caller's terminated word.
        host.source_length = unsafe { libc::strlen(word.cast()) } + 1;
    }
    let table = word2::Callbacks {
        context: addr_of_mut!(host).cast(),
        value: Some(word2_value),
        store: Some(word2_store),
        byte: Some(word2_byte),
        write: Some(word2_write),
        entry: Some(list_entry),
        put: Some(list_put),
        command: Some(embedded_command),
        put_command: Some(embedded_put_command),
        embedded: Some(word2_embedded),
        encode: Some(word2_encode),
        translate: Some(word2_translate),
        combine: Some(word2_combine),
        dictionary: Some(word2_dictionary),
        restore_dictionary: Some(word2_restore_dictionary),
        alternate: Some(word2_alternate),
        select: Some(word2_select),
        phoneme: Some(word2_phoneme),
        alpha: Some(word2_alpha),
        invalid: Some(word2_invalid),
    };
    let mut flags = 0;
    // SAFETY: the table and host outlive the call.
    if unsafe { word2::espeak_rs_translate_word2(&table, pause, &mut flags) } == 0 {
        flags as c_int
    } else {
        0
    }
}

// Character conversion (RustCharacterConversion).
static mut CHARACTER_IGNORE: c_int = 0;
fn character_tr(opaque: *mut c_void) -> *mut Translator {
    // SAFETY: every callback receives the translator its entry passed.
    unsafe { *opaque.cast::<*mut Translator>() }
}
/// `CharacterLength`: the replacement table's extent in the dictionary.
fn character_length(tr: *mut Translator) -> i32 {
    // SAFETY: the live translator's dictionary extent.
    unsafe {
        let tr = &*tr;
        if tr.langopts.replace_chars.is_null() {
            return 0;
        }
        let (base, begin) = (
            tr.data_dictlist as usize,
            tr.langopts.replace_chars as usize,
        );
        if base == 0
            || begin < base
            || begin - base >= tr.data_dict_size
            || tr.data_dict_size > 134_217_728
        {
            return -1;
        }
        (tr.data_dict_size - (begin - base)) as i32
    }
}
unsafe extern "C" fn character_value(opaque: *mut c_void, field: u32) -> i32 {
    let tr = character_tr(opaque);
    match field {
        // SAFETY: the live translator.
        0 => unsafe { (*tr).translator_name },
        // SAFETY: serialized engine state.
        1 => unsafe { CHARACTER_IGNORE },
        2 => character_length(tr),
        _ => -1,
    }
}
unsafe extern "C" fn character_ignore(_: *mut c_void, count: i32) -> i32 {
    if count < 0 {
        return -1;
    }
    // SAFETY: serialized engine state.
    unsafe { CHARACTER_IGNORE = count };
    0
}
unsafe extern "C" fn character_byte(opaque: *mut c_void, kind: u32, index: usize) -> i32 {
    if kind == 0 {
        if index >= SOURCE {
            return -1;
        }
        // SAFETY: in bounds of the clause text.
        return i32::from(unsafe { *source().add(index) });
    }
    let tr = character_tr(opaque);
    let length = character_length(tr);
    if kind != 1 || length < 0 || index >= length as usize {
        return -1;
    }
    // SAFETY: in bounds of the replacement table.
    i32::from(unsafe { *(*tr).langopts.replace_chars.add(index) })
}
unsafe extern "C" fn character_write(_: *mut c_void, index: usize, value: u8) -> i32 {
    if index >= SOURCE {
        return -1;
    }
    // SAFETY: in bounds of the clause text.
    unsafe { *source().add(index) = value };
    0
}
unsafe extern "C" fn character_class(opaque: *mut c_void, code: u32, kind: u32) -> i32 {
    match kind {
        0 => i32::from(super::ucd_isupper(code) != 0),
        1 => i32::from(super::ucd_isalpha(code) != 0),
        // SAFETY: pure classifier.
        2 => unsafe { IsSpace(code) },
        // SAFETY: the live translator.
        3 => unsafe { IsVowel(character_tr(opaque).cast(), code as c_int) },
        _ => 0,
    }
}
unsafe extern "C" fn character_lower(opaque: *mut c_void, code: u32) -> u32 {
    // SAFETY: the live translator.
    unsafe { towlower2(code, character_tr(opaque).cast()) as u32 }
}
unsafe extern "C" fn character_trace(opaque: *mut c_void, from: usize, to: usize) -> i32 {
    let tr = character_tr(opaque);
    let length = character_length(tr);
    if length < 0 || from >= length as usize || to >= length as usize {
        return -1;
    }
    let length = length as usize;
    // SAFETY: the replacement table, read within its extent.
    unsafe {
        let base = (*tr).langopts.replace_chars;
        if terminated(base.add(from), length - from).is_none()
            || terminated(base.add(to), length - to).is_none()
        {
            return -1;
        }
        if c::tracing() {
            if c::f_trans.is_null() {
                return -1;
            }
            c::trace(&[
                b"Replace: ",
                c::text(base.add(from)),
                b" > ",
                c::text(base.add(to)),
                b"\n",
            ]);
        }
    }
    0
}
fn character_table(tr: &mut *mut Translator) -> character::Callbacks {
    character::Callbacks {
        context: (tr as *mut *mut Translator).cast(),
        value: Some(character_value),
        ignore: Some(character_ignore),
        byte: Some(character_byte),
        write: Some(character_write),
        classify: Some(character_class),
        lower: Some(character_lower),
        trace: Some(character_trace),
    }
}
/// `TranslateChar`.
unsafe fn translate_char(
    tr: *mut Translator,
    at: *mut u8,
    previous: c_int,
    code: u32,
    next: u32,
    insert: *mut c_int,
    flags: *mut c_int,
) -> c_int {
    let (begin, at) = (source() as usize, at as usize);
    if tr.is_null() || insert.is_null() || flags.is_null() || at < begin || at - begin >= SOURCE {
        return code as c_int;
    }
    let mut owner = tr;
    let table = character_table(&mut owner);
    // SAFETY: the caller's insert and flags; the table outlives the call.
    unsafe {
        let call = Call {
            index: at - begin,
            previous: previous as u32,
            code,
            next,
            insert: *insert as u32,
            flags: *flags as u32,
        };
        let mut result = ResultCode {
            code: 0,
            insert: 0,
            flags: 0,
        };
        if character::espeak_rs_character_translate(&table, &call, &mut result) != 0 {
            return code as c_int;
        }
        *insert = result.insert as c_int;
        *flags = result.flags as c_int;
        result.code as c_int
    }
}
/// `UpperCaseInWord`.
unsafe fn upper_case_in_word(tr: *mut Translator, word: *mut u8, code: c_int) -> c_int {
    if tr.is_null() || word.is_null() {
        return 0;
    }
    let mut before = [0u8; 3];
    // SAFETY: the three bytes before the word in the clause text.
    unsafe { ptr::copy_nonoverlapping(word.sub(3), before.as_mut_ptr(), 3) };
    let mut owner = tr;
    let table = character_table(&mut owner);
    let mut result = 0;
    // SAFETY: the table outlives the call.
    unsafe {
        character::espeak_rs_character_upper(&table, before.as_ptr(), code as u32, &mut result)
    };
    result
}

// The clause translator (RustTranslateClause).
struct ClauseHost {
    tr: *mut Translator,
    tone: *mut c_int,
    terminator: *mut c_int,
    voice: *mut *mut c_char,
}
static mut CLAUSE_VOICE: [u8; 40] = [0; 40];
fn clause_host<'a>(opaque: *mut c_void) -> &'a mut ClauseHost {
    // SAFETY: every callback receives the live host its entry made.
    unsafe { &mut *opaque.cast::<ClauseHost>() }
}
unsafe extern "C" fn clause_value(opaque: *mut c_void, field: u32) -> i32 {
    let h = clause_host(opaque);
    // SAFETY: the live translator and the engine's clause state.
    unsafe {
        let tr = &*h.tr;
        match field {
            0 => embedded_ix,
            1 => embedded_read,
            2 => c::pre_pause,
            3 => i32::from(any_stressed_words),
            4 => count_characters,
            5 => clause_start_char,
            6 => count_words,
            7 => clause_start_word,
            8 => max_clause_pause,
            9 => i32::from(new_sentence),
            10 => count_sentences,
            11 => skip_sentences,
            12 => i32::from(skipping_text),
            13 => post::n_ph_list2,
            14 => tr.prev_last_stress,
            15 => tr.prepause_timeout,
            16 => tr.expect_verb,
            17 => tr.expect_noun,
            18 => tr.expect_past,
            19 => tr.expect_verb_s,
            20 => tr.phonemes_repeat_count,
            21 => tr.end_stressed_vowel,
            22 => tr.prev_dict_flags[0],
            23 => tr.prev_dict_flags[1],
            24 => tr.clause_terminator,
            25 => i32::from(tr.langopts.tone_numbers as c_char),
            26 => option_sayas2,
            27 => count_sayas_digits,
            28 => option_phoneme_input,
            29 => tr.letter_bits_offset,
            30 => tr.langopts.param[CAPS_IN_WORD],
            31 => tr.translator_name,
            32 => tr.langopts.param[APOSTROPHE],
            33 => word_emphasis,
            34 => tr.langopts.break_numbers as i32,
            35 => tr.langopts.thousands_sep,
            36 => tr.langopts.decimal_sep,
            37 => tr.langopts.max_digits,
            38 => skip_words,
            39 => c::dictionary_skipwords,
            40 => option_endpause,
            41 => post::n_phoneme_list,
            42 => i32::from(tr.langopts.ideographs as c_char),
            43 => tr.langopts.alt_alphabet,
            _ => 0,
        }
    }
}
unsafe extern "C" fn clause_store(opaque: *mut c_void, field: u32, value: i32) -> i32 {
    let h = clause_host(opaque);
    if (field == 0 || field == 1) && !(0..=EMBEDDED_LIST as i32).contains(&value) {
        return -1;
    }
    if field == 13 && !(0..=PHONEME_LIST as i32).contains(&value) {
        return -1;
    }
    // SAFETY: the live translator and the engine's clause state.
    unsafe {
        let tr = &mut *h.tr;
        match field {
            0 => embedded_ix = value,
            1 => embedded_read = value,
            2 => c::pre_pause = value,
            3 => any_stressed_words = value != 0,
            4 => count_characters = value,
            5 => clause_start_char = value,
            6 => count_words = value,
            7 => clause_start_word = value,
            8 => max_clause_pause = value,
            9 => new_sentence = value != 0,
            10 => count_sentences = value,
            11 => skip_sentences = value,
            12 => skipping_text = value != 0,
            13 => post::n_ph_list2 = value,
            14 => tr.prev_last_stress = value,
            15 => tr.prepause_timeout = value,
            16 => tr.expect_verb = value,
            17 => tr.expect_noun = value,
            18 => tr.expect_past = value,
            19 => tr.expect_verb_s = value,
            20 => tr.phonemes_repeat_count = value,
            21 => tr.end_stressed_vowel = value,
            22 => tr.prev_dict_flags[0] = value,
            23 => tr.prev_dict_flags[1] = value,
            24 => tr.clause_terminator = value,
            26 => option_sayas2 = value,
            27 => count_sayas_digits = value,
            38 => skip_words = value,
            39 => c::dictionary_skipwords = value,
            44 => {
                if value != 0 {
                    return -1;
                }
                current_alphabet = ptr::null_mut();
            }
            _ => return -1,
        }
    }
    0
}
unsafe extern "C" fn clause_byte(_: *mut c_void, index: usize) -> i32 {
    if index >= SOURCE {
        return -1;
    }
    // SAFETY: in bounds of the clause text.
    i32::from(unsafe { *source().add(index) })
}
unsafe extern "C" fn clause_write(_: *mut c_void, index: usize, value: u8) -> i32 {
    if index >= SOURCE {
        return -1;
    }
    // SAFETY: in bounds of the clause text.
    unsafe { *source().add(index) = value };
    0
}
unsafe extern "C" fn clause_class(opaque: *mut c_void, code: u32, kind: u32) -> i32 {
    let h = clause_host(opaque);
    // SAFETY: pure classifiers; the live translator's terminated lists.
    unsafe {
        match kind {
            0 => IsAlpha(code),
            1 => IsSpace(code),
            2 => i32::from(super::ucd_isupper(code) != 0),
            3 => i32::from(super::ucd_islower(code) != 0),
            4 => i32::from(super::ucd_isdigit(code) != 0),
            5 => i32::from(super::ucd_isalnum(code) != 0),
            6 => IsBracket(code as c_int),
            7 => IsEmoji(code),
            8 => IsEmojiModifier(code),
            9 => IsRegionalIndicator(code),
            10 => IsEmojiTag(code),
            11 => i32::from(contains((*h.tr).punct_within_word, code)),
            12 => i32::from(contains((*h.tr).char_plus_apostrophe, code)),
            // lookupwchar(breaks, code): breaks is { '_', 0 }.
            13 => i32::from(code == u32::from(b'_')),
            14 => isspace2(code),
            15 => IsDigit(code),
            _ => 0,
        }
    }
}
unsafe extern "C" fn clause_lower(opaque: *mut c_void, code: u32) -> u32 {
    // SAFETY: the live translator.
    unsafe { towlower2(code, clause_host(opaque).tr.cast()) as u32 }
}
unsafe extern "C" fn clause_read(
    opaque: *mut c_void,
    positions: *mut i16,
    name: *mut u8,
    out: *mut Read,
) -> i32 {
    let h = clause_host(opaque);
    let (mut top, mut tone) = (0, 0);
    // SAFETY: the engine's clause text and voice name, the caller's outputs.
    unsafe {
        let voice = addr_of_mut!(CLAUSE_VOICE).cast::<u8>();
        let terminator = ReadClause(
            h.tr.cast(),
            source().cast(),
            positions,
            &mut top,
            SOURCE_BYTES as c_int,
            &mut tone,
            voice.cast(),
        );
        if top < 0 || top >= SOURCE_BYTES as c_int {
            return -1;
        }
        let Some(length) = terminated(voice, 40) else {
            return -1;
        };
        ptr::copy_nonoverlapping(voice, name, length + 1);
        *out = Read {
            terminator,
            tone,
            top: top as usize,
        };
    }
    0
}
unsafe extern "C" fn clause_embedded_command(
    _: *mut c_void,
    index: *mut usize,
    result: *mut i32,
) -> i32 {
    // SAFETY: the caller's index and result.
    unsafe {
        if *index >= SOURCE {
            return -1;
        }
        let mut ix = *index as u32;
        *result = embedded_command_at(&mut ix);
        *index = ix as usize;
        if ix as usize <= SOURCE {
            0
        } else {
            -1
        }
    }
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn clause_char(
    opaque: *mut c_void,
    index: usize,
    previous: u32,
    code: u32,
    next: u32,
    insert: *mut u32,
    flags: *mut u32,
    result: *mut u32,
) -> i32 {
    if index >= SOURCE {
        return -1;
    }
    let h = clause_host(opaque);
    // SAFETY: the caller's in-out scalars; the clause text in bounds.
    unsafe {
        let mut inserted = *insert as c_int;
        let mut word_flags = *flags as c_int;
        *result = translate_char(
            h.tr,
            source().add(index),
            previous as c_int,
            code,
            next,
            &mut inserted,
            &mut word_flags,
        ) as u32;
        *insert = inserted as u32;
        *flags = word_flags as u32;
    }
    0
}
unsafe extern "C" fn clause_upper(
    opaque: *mut c_void,
    text: *mut u8,
    index: usize,
    code: u32,
    result: *mut i32,
) -> i32 {
    if !(3..SOURCE_BYTES).contains(&index) {
        return -1;
    }
    // SAFETY: the caller's text and result.
    unsafe { *result = upper_case_in_word(clause_host(opaque).tr, text.add(index), code as c_int) };
    0
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn clause_translate(
    opaque: *mut c_void,
    text: *mut u8,
    end: usize,
    number: *mut u8,
    initialized: usize,
    offset: usize,
    rows: *mut Row,
    row: usize,
    remaining: usize,
    pause: i32,
    flags: *mut u32,
) -> i32 {
    let h = clause_host(opaque);
    if end >= SOURCE_BYTES - 1 || row >= CLAUSE_WORDS || remaining > CLAUSE_WORDS - row {
        return -1;
    }
    let (base, length) = if number.is_null() {
        (text, end + 2)
    } else {
        (number, initialized)
    };
    if (!number.is_null() && (initialized == 0 || initialized > 150)) || offset >= length {
        return -1;
    }
    // SAFETY: the live translator, whose clause end and rule source are the
    // clause's for the call; the clause's rows.
    unsafe {
        let saved = (*h.tr).clause_end;
        (*h.tr).clause_end = text.add(end);
        *flags = with_rule_source(h.tr, base, length, || {
            translate_word2(
                h.tr,
                base.add(offset),
                rows.add(row + 1),
                remaining as c_int,
                pause,
            )
        }) as u32;
        (*h.tr).clause_end = saved;
    }
    0
}
unsafe extern "C" fn clause_word_embedded(_: *mut c_void) -> i32 {
    // SAFETY: serialized engine.
    unsafe { word_embedded_command() };
    0
}
unsafe extern "C" fn clause_eof(_: *mut c_void) -> i32 {
    // SAFETY: serialized engine.
    unsafe { Eof() }
}
unsafe extern "C" fn clause_make_list(opaque: *mut c_void, pause: i32, sentence: i32) -> i32 {
    // SAFETY: the live translator.
    unsafe { MakePhonemeList(clause_host(opaque).tr.cast(), pause, sentence != 0) };
    0
}
unsafe extern "C" fn clause_output(_: *mut c_void, index: usize, field: u32, value: u32) -> i32 {
    if index > PHONEME_LIST {
        return -1;
    }
    // SAFETY: the engine's phoneme list (PHONEME_LIST+1 entries), in bounds.
    unsafe {
        let entry = &mut (*addr_of_mut!(post::phoneme_list)).0[index];
        match field {
            0 => {
                if value != 0 {
                    return -1;
                }
                entry.phoneme = ptr::null_mut();
            }
            1 => entry.source = value as u16,
            2 => entry.flags = value as u16,
            _ => return -1,
        }
    }
    0
}
unsafe extern "C" fn clause_result(
    opaque: *mut c_void,
    kind: u32,
    value: i32,
    name: *const u8,
) -> i32 {
    let h = clause_host(opaque);
    // SAFETY: the host's outputs; the clause's terminated voice name.
    unsafe {
        match kind {
            0 => {
                if !h.terminator.is_null() {
                    *h.terminator = value;
                }
            }
            1 => {
                if !h.tone.is_null() {
                    *h.tone = value;
                }
            }
            2 => {
                if value != 0 {
                    let Some(length) = terminated(name, 40) else {
                        return -1;
                    };
                    let voice = addr_of_mut!(CLAUSE_VOICE).cast::<u8>();
                    ptr::copy(name, voice, length + 1);
                    if !h.voice.is_null() {
                        *h.voice = voice.cast();
                    }
                } else if !h.voice.is_null() {
                    *h.voice = ptr::null_mut();
                }
            }
            _ => return -1,
        }
    }
    0
}
/// `TranslateClauseWithTerminator`.
///
/// # Safety
/// As the C function; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_clause_engine(
    tr: *mut Translator,
    tone: *mut c_int,
    voice: *mut *mut c_char,
    terminator: *mut c_int,
) {
    if tr.is_null() {
        return;
    }
    let mut host = ClauseHost {
        tr,
        tone,
        terminator,
        voice,
    };
    let table = clause::Callbacks {
        context: addr_of_mut!(host).cast(),
        value: Some(clause_value),
        store: Some(clause_store),
        byte: Some(clause_byte),
        write: Some(clause_write),
        classify: Some(clause_class),
        lower: Some(clause_lower),
        read: Some(clause_read),
        embedded_command: Some(clause_embedded_command),
        translate_char: Some(clause_char),
        upper_word: Some(clause_upper),
        entry: Some(list_entry),
        put: Some(list_put),
        command: Some(embedded_command),
        put_command: Some(embedded_put_command),
        translate: Some(clause_translate),
        word_embedded: Some(clause_word_embedded),
        eof: Some(clause_eof),
        make_list: Some(clause_make_list),
        output: Some(clause_output),
        result: Some(clause_result),
    };
    // SAFETY: the table and host outlive the call.
    unsafe { clause::espeak_rs_translate_clause(&table) };
}

/// `lookupwchar`: the 1-based position of `code` in a terminated list, or 0.
///
/// # Safety
/// `list` is a terminated list.
#[no_mangle]
unsafe extern "C" fn lookupwchar(list: *const u16, code: c_int) -> c_int {
    let mut index = 0;
    // SAFETY: caller contract.
    while unsafe { *list.add(index) } != 0 {
        // SAFETY: as above.
        if c_int::from(unsafe { *list.add(index) }) == code {
            return index as c_int + 1;
        }
        index += 1;
    }
    0
}
/// `strchr_w`: `strchr` for ASCII codes, null for any other.
///
/// # Safety
/// `text` is terminated.
#[no_mangle]
unsafe extern "C" fn strchr_w(text: *const c_char, code: c_int) -> *mut c_char {
    if code >= 0x80 {
        return ptr::null_mut();
    }
    // SAFETY: caller contract.
    unsafe { libc::strchr(text, code) }
}
