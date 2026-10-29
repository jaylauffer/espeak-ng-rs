//! The word translator's hosts against the engine (from translateword.c):
//! the main word, emoji, spelling and plural, isolated-letter and admission
//! hosts the Rust word translator calls, and `TranslateWord3`. A host lends
//! source bytes as numeric (slot, offset) identities checked against the
//! spans it admitted, so no Rust loan spans a nested translator call.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]
use super::{
    emoji_fallback_compat as emoji, engine_c as c,
    letter_host_compat::engine::espeak_rs_lookup_letter_engine, synthdata_compat as synth,
    translate_letter_compat as letter, translate_word_compat as word,
    word_admission_compat as admission, word_helpers_compat as helpers,
};
use crate::{translate_clause::Row, translate_word::Source, translator::Translator};
use std::{
    ffi::{c_char, c_int, c_uint, c_void},
    ptr::{self, addr_of, addr_of_mut},
};

/// `N_WORD_BYTES` and `N_WORD_PHONEMES` (translate.h).
const WORD_BYTES: usize = 160;
const WORD_PHONEMES: usize = 200;
// LOPT_PREFIXES, LOPT_UNPRONOUNCABLE and LOPT_ALT (translate.h).
const PREFIXES: usize = 2;
const UNPRONOUNCABLE: usize = 4;
const ALTERNATE: usize = 11;
// FLAG_TEXTMODE, FLAG_ALLOW_TEXTMODE and FLAG_UNPRON_TEST (translate.h).
const TEXT_MODE: u32 = 0x2000_0000;
const ALLOW_TEXT_MODE: c_int = 0x02;
const UNPRONOUNCEABLE_TEST: u32 = 0x8000_0000;

unsafe extern "C" {
    static mut option_tone_flags: c_int;
    fn TranslateRoman(
        tr: *mut c_void,
        word: *mut c_char,
        output: *mut c_char,
        end: *mut c_char,
        rows: *mut c_void,
        remaining: c_int,
    ) -> c_int;
    #[allow(clippy::too_many_arguments)]
    fn TranslateNumber(
        tr: *mut c_void,
        word: *mut c_char,
        output: *mut c_char,
        end: *mut c_char,
        flags: *mut u32,
        rows: *mut c_void,
        remaining: c_int,
        control: c_int,
    ) -> c_int;
    fn RemoveEnding(
        tr: *mut c_void,
        word: *mut c_char,
        end_type: c_int,
        copy: *mut c_char,
    ) -> c_int;
    fn SetSpellingStress(tr: *mut c_void, phonemes: *mut c_char, control: c_int, count: c_int);
    fn DecodePhonemes(input: *const c_char, output: *mut c_char);
    fn IsVowel(tr: *mut c_void, letter: c_int) -> c_int;
    fn EncodePhonemesBounded(
        text: *const c_char,
        output: *mut c_char,
        capacity: usize,
        bad: *mut c_int,
    ) -> *const c_char;
    fn utf8_in(code: *mut c_int, text: *const c_char) -> c_int;
}

/// `ALPHABET` (translate.h), read for `range_min`.
#[repr(C)]
pub(super) struct Alphabet {
    name: *const c_char,
    offset: c_int,
    range_min: c_uint,
    range_max: c_uint,
    language: c_int,
    flags: c_int,
}

#[derive(Clone, Copy)]
struct Span {
    base: *mut u8,
    origin: *mut u8,
    length: usize,
}
const NO_SPAN: Span = Span {
    base: ptr::null_mut(),
    origin: ptr::null_mut(),
    length: 0,
};
/// `RustWordHost`: one word translation's owners and admitted source spans.
struct Host {
    tr: *mut Translator,
    rows: *mut Row,
    remaining: c_int,
    any_stressed: *mut bool,
    alphabet: *const Alphabet,
    output: *mut u8,
    text_output: *mut u8,
    spans: [Span; 3],
    prefix: [u8; 65],
}
impl Host {
    fn new(tr: *mut Translator) -> Self {
        Self {
            tr,
            rows: ptr::null_mut(),
            remaining: 0,
            any_stressed: ptr::null_mut(),
            alphabet: ptr::null(),
            output: ptr::null_mut(),
            text_output: ptr::null_mut(),
            spans: [NO_SPAN; 3],
            prefix: [0; 65],
        }
    }
    /// `WordPointer`: the byte a source identity names, or null.
    fn pointer(&self, source: Source) -> *mut u8 {
        let Some(span) = self.spans.get(source.slot as usize).copied() else {
            return ptr::null_mut();
        };
        if span.base.is_null() {
            return ptr::null_mut();
        }
        let mut address = span.origin as usize;
        if source.offset < 0 {
            let before = (-(source.offset + 1)) as usize + 1;
            if before > address {
                return ptr::null_mut();
            }
            address -= before;
        } else {
            let Some(after) = address.checked_add(source.offset as usize) else {
                return ptr::null_mut();
            };
            address = after;
        }
        let base = span.base as usize;
        if address >= base && address - base < span.length {
            span.base.wrapping_add(address - base)
        } else {
            ptr::null_mut()
        }
    }
    /// `WordSource`: the identity of an admitted byte.
    fn source(&mut self, pointer: *mut u8, source: &mut Source) -> c_int {
        let address = pointer as usize;
        for (slot, span) in self.spans.iter().enumerate() {
            let (base, origin) = (span.base as usize, span.origin as usize);
            if span.base.is_null() || address < base || address - base >= span.length {
                continue;
            }
            let delta = address.abs_diff(origin);
            if delta > isize::MAX as usize {
                return -1;
            }
            *source = Source {
                slot: slot as u32,
                offset: if address >= origin {
                    delta as isize
                } else {
                    -(delta as isize)
                },
            };
            return 0;
        }
        // SAFETY: the live translator's replacement buffer, address only.
        let replacement = unsafe { addr_of_mut!((*self.tr).rust_list_replacement) }.cast::<u8>();
        let base = replacement as usize;
        if address >= base && address - base < WORD_BYTES {
            self.spans[1] = Span {
                base: replacement,
                origin: replacement.wrapping_add(2),
                length: WORD_BYTES,
            };
            return self.source(pointer, source);
        }
        -1
    }
    /// `WordWindow`: admit `word` within a translator's rule text, else
    /// with its predecessor and two bytes past its NUL.
    ///
    /// # Safety
    /// `word` is null or terminated with the bytes described.
    unsafe fn window(&mut self, word: *mut u8) -> c_int {
        if word.is_null() {
            return -1;
        }
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
            let address = word as usize;
            if address >= base as usize && address - (base as usize) < length {
                self.spans[0] = Span {
                    base: base.cast_mut(),
                    origin: word,
                    length,
                };
                return 0;
            }
        }
        // Internal callers retain an initialized predecessor and NUL lookahead.
        self.spans[0] = Span {
            base: word.wrapping_sub(1),
            origin: word,
            // SAFETY: caller contract.
            length: unsafe { libc::strlen(word.cast()) } + 3,
        };
        0
    }
}
fn host<'a>(opaque: *mut c_void) -> &'a mut Host {
    // SAFETY: every callback receives the live Host its entry made.
    unsafe { &mut *opaque.cast::<Host>() }
}
/// A terminated string's length within `capacity` bytes.
///
/// # Safety
/// `text` has `capacity` readable bytes or a NUL before them.
unsafe fn terminated(text: *const u8, capacity: usize) -> Option<usize> {
    // SAFETY: caller contract.
    let end = unsafe { libc::memchr(text.cast(), 0, capacity) };
    (!end.is_null()).then(|| end as usize - text as usize)
}

// The main word host (RustTranslateWord).
unsafe extern "C" fn word_byte(opaque: *mut c_void, source: Source) -> i32 {
    let p = host(opaque).pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted byte.
    i32::from(unsafe { *p })
}
unsafe extern "C" fn word_write(opaque: *mut c_void, source: Source, byte: u8) -> i32 {
    let p = host(opaque).pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted writable byte.
    unsafe { *p = byte };
    0
}
unsafe extern "C" fn word_value(opaque: *mut c_void, field: u32, index: u32) -> i32 {
    let host = host(opaque);
    // SAFETY: the live translator, its rows and the engine's options.
    unsafe {
        let tr = &*host.tr;
        match field {
            0 => i32::from(!tr.data_dictlist.is_null()),
            1 => {
                if index == 0 || (host.remaining > 0 && index < host.remaining as u32) {
                    (*host.rows.add(index as usize)).flags as i32
                } else {
                    0
                }
            }
            2 => host.remaining,
            3 => c::option_sayas,
            4 => c::dictionary_skipwords,
            5 => tr.langopts.numbers,
            6 => tr.langopts.numbers2,
            7 => option_tone_flags,
            8 => tr.clause_lower_count,
            9 => tr.clause_upper_count,
            10 => tr.langopts.param[PREFIXES],
            11 => i32::from(c::tracing()),
            12 => tr.langopts.stress_flags,
            13 => i32::from(!host.any_stressed.is_null()),
            14 => tr.translator_name,
            15 => tr.langopts.param[ALTERNATE],
            16 => tr.expect_verb,
            17 => tr.expect_verb_s,
            18 => tr.expect_noun,
            19 => tr.expect_past,
            _ => 0,
        }
    }
}
unsafe extern "C" fn word_store(opaque: *mut c_void, field: u32, value: i32) {
    let host = host(opaque);
    // SAFETY: the live translator and the engine's options.
    unsafe {
        let tr = &mut *host.tr;
        match field {
            0 => c::dictionary_skipwords = value,
            1 => tr.expect_verb = value,
            2 => tr.expect_verb_s = value,
            3 => tr.expect_noun = value,
            4 => tr.expect_past = value,
            _ => {}
        }
    }
}
unsafe extern "C" fn word_locale(_: *mut c_void, code: u32, digit: u32) -> i32 {
    let class = if digit != 0 {
        super::ucd_isdigit(code)
    } else {
        super::ucd_isalpha(code)
    };
    i32::from(class != 0)
}
unsafe extern "C" fn word_list(
    opaque: *mut c_void,
    source: *mut Source,
    phonemes: *mut u8,
    flags: *mut u32,
    ending: i32,
    found: *mut i32,
) -> i32 {
    let host = host(opaque);
    // SAFETY: the translator's outputs and an admitted source.
    unsafe {
        let mut p = host.pointer(*source);
        if p.is_null() {
            return -1;
        }
        *found = c::LookupDictListBounded(
            host.tr.cast(),
            addr_of_mut!(p).cast(),
            phonemes.cast(),
            flags,
            ending,
            host.rows.cast(),
            host.remaining,
            WORD_PHONEMES,
        );
        host.source(p, &mut *source)
    }
}
unsafe extern "C" fn word_emoji(opaque: *mut c_void, source: *mut Source, flags: *mut u32) -> i32 {
    let host = host(opaque);
    // SAFETY: the translator's flags and an admitted source.
    unsafe {
        let mut p = host.pointer(*source);
        if p.is_null() {
            return -1;
        }
        if lookup_emoji_base_sequence(host.tr, &mut p, flags, host.rows, host.remaining) {
            host.spans[1] = Span {
                base: p.wrapping_sub(2),
                origin: p,
                length: WORD_BYTES,
            };
        }
        host.source(p, &mut *source)
    }
}
unsafe extern "C" fn word_text(opaque: *mut c_void, source: Source) -> i32 {
    let host = host(opaque);
    if host.text_output.is_null() {
        return 0;
    }
    let mut count = 0usize;
    loop {
        if count > WORD_BYTES || source.offset > isize::MAX - count as isize {
            return -1;
        }
        let p = host.pointer(Source {
            slot: source.slot,
            offset: source.offset + count as isize,
        });
        if p.is_null() {
            return -1;
        }
        // SAFETY: an admitted byte.
        if unsafe { *p } == 0 {
            break;
        }
        count += 1;
    }
    // SAFETY: count+1 admitted bytes and the caller's text output.
    unsafe { ptr::copy_nonoverlapping(host.pointer(source), host.text_output, count + 1) };
    0
}
unsafe extern "C" fn word_dotted(opaque: *mut c_void, source: Source, result: *mut i32) -> i32 {
    let p = host(opaque).pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the caller's result.
    unsafe { *result = check_dotted_abbrev(p) };
    0
}
unsafe extern "C" fn word_number_language(opaque: *mut c_void) -> i32 {
    let host = host(opaque);
    // SAFETY: the live translator and its 200-byte output.
    unsafe {
        c::LookupBounded(
            host.tr.cast(),
            c"_0lang".as_ptr(),
            host.output.cast(),
            WORD_PHONEMES,
        );
    }
    0
}
unsafe extern "C" fn word_number(
    opaque: *mut c_void,
    roman: u32,
    source: Source,
    phonemes: *mut u8,
    flags: *mut u32,
    found: *mut i32,
) -> i32 {
    let host = host(opaque);
    let p = host.pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the translator's outputs.
    unsafe {
        let end = phonemes.add(WORD_PHONEMES).cast();
        *found = if roman != 0 {
            TranslateRoman(
                host.tr.cast(),
                p.cast(),
                phonemes.cast(),
                end,
                host.rows.cast(),
                host.remaining,
            )
        } else {
            TranslateNumber(
                host.tr.cast(),
                p.cast(),
                phonemes.cast(),
                end,
                flags,
                host.rows.cast(),
                host.remaining,
                0,
            )
        };
    }
    0
}
unsafe extern "C" fn word_spell(
    opaque: *mut c_void,
    source: *mut Source,
    phonemes: *mut u8,
    mode: i32,
    switched: *mut i32,
) -> i32 {
    let host = host(opaque);
    // SAFETY: an admitted word and the translator's outputs.
    unsafe {
        let p = host.pointer(*source);
        if p.is_null() {
            return -1;
        }
        let p = speak_individual_letters(host.tr, p, phonemes, mode, host.alphabet, host.output);
        *switched = i32::from(p.is_null());
        if p.is_null() {
            0
        } else {
            host.source(p, &mut *source)
        }
    }
}
unsafe extern "C" fn word_letter(
    opaque: *mut c_void,
    source: Source,
    phonemes: *mut u8,
    non_initial: u32,
    consumed: *mut usize,
) -> i32 {
    let host = host(opaque);
    let p = host.pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the translator's outputs.
    unsafe {
        let count = translate_letter(host.tr, p, phonemes, non_initial as c_int, host.alphabet);
        if count < 0 {
            return -1;
        }
        *consumed = count as usize;
    }
    0
}
unsafe extern "C" fn word_unpronounceable(
    opaque: *mut c_void,
    source: Source,
    position: i32,
    result: *mut i32,
) -> i32 {
    let host = host(opaque);
    let p = host.pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the caller's result.
    unsafe { *result = unpronounceable(host.tr, p, position) };
    0
}
unsafe extern "C" fn word_spelling_stress(
    opaque: *mut c_void,
    phonemes: *mut u8,
    position: i32,
) -> i32 {
    // SAFETY: the live translator and the caller's phonemes.
    unsafe { SetSpellingStress(host(opaque).tr.cast(), phonemes.cast(), 0, position) };
    0
}
unsafe extern "C" fn word_rules(
    opaque: *mut c_void,
    source: Source,
    phonemes: *mut u8,
    ending: *mut u8,
    word_flags: u32,
    flags: *mut u32,
    result: *mut i32,
) -> i32 {
    let host = host(opaque);
    let p = host.pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the translator's outputs.
    unsafe {
        *result = c::TranslateRules(
            host.tr.cast(),
            p.cast(),
            phonemes.cast(),
            WORD_PHONEMES as c_int,
            ending.cast(),
            word_flags as c_int,
            flags,
        );
    }
    0
}
unsafe extern "C" fn word_remove(
    opaque: *mut c_void,
    source: Source,
    ending: i32,
    copy: *mut u8,
    result: *mut i32,
) -> i32 {
    let host = host(opaque);
    let p = host.pointer(source);
    if p.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the translator's copy.
    unsafe { *result = RemoveEnding(host.tr.cast(), p.cast(), ending, copy.cast()) };
    0
}
unsafe extern "C" fn word_prefix(
    opaque: *mut c_void,
    prefix: *const u8,
    result: *mut Source,
) -> i32 {
    let host = host(opaque);
    // SAFETY: the translator's 65-byte prefix and the caller's result.
    unsafe {
        ptr::copy_nonoverlapping(prefix, host.prefix.as_mut_ptr(), host.prefix.len());
        let base = host.prefix.as_mut_ptr();
        host.spans[2] = Span {
            base,
            origin: base.wrapping_add(1),
            length: host.prefix.len(),
        };
        *result = Source { slot: 2, offset: 0 };
    }
    0
}
unsafe extern "C" fn word_trace_suffix(_: *mut c_void, phonemes: *const u8) {
    let mut decoded = [0u8; WORD_PHONEMES];
    // SAFETY: the translator's terminated phonemes.
    unsafe { DecodePhonemes(phonemes.cast(), decoded.as_mut_ptr().cast()) };
    let length = decoded
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(decoded.len());
    c::trace(&[b"  suffix [", &decoded[..length], b"]\n\n"]);
}
unsafe extern "C" fn word_append(opaque: *mut c_void, phonemes: *mut u8, ending: *const u8) -> i32 {
    // SAFETY: the live translator and the translator's phonemes.
    unsafe {
        c::AppendPhonemes(
            host(opaque).tr.cast(),
            phonemes.cast(),
            WORD_PHONEMES as c_int,
            ending.cast(),
        )
    };
    0
}
unsafe extern "C" fn word_plural(opaque: *mut c_void, flags: u32, last: u32) -> i32 {
    let host = host(opaque);
    // SAFETY: the live translator and its output.
    unsafe { add_plural_suffixes(flags as c_int, host.tr, last as u8, host.output) };
    0
}
unsafe extern "C" fn word_stress(
    opaque: *mut c_void,
    phonemes: *mut u8,
    flags: *mut u32,
    position: i32,
    control: i32,
) -> i32 {
    let host = host(opaque);
    let phonemes = if phonemes.is_null() {
        host.output
    } else {
        phonemes
    };
    // SAFETY: the live translator and its phonemes.
    unsafe { c::SetWordStress(host.tr.cast(), phonemes.cast(), flags, position, control) };
    0
}
unsafe extern "C" fn word_snapshot(opaque: *mut c_void, phonemes: *mut u8) -> i32 {
    let host = host(opaque);
    // SAFETY: the host's 200-byte output and the translator's buffer.
    unsafe {
        let Some(length) = terminated(host.output, WORD_PHONEMES) else {
            return -1;
        };
        ptr::copy_nonoverlapping(host.output, phonemes, length + 1);
    }
    0
}
unsafe extern "C" fn word_publish(opaque: *mut c_void, phonemes: *const u8, joined: u32) -> i32 {
    let host = host(opaque);
    // SAFETY: the translator's phonemes and the host's 200-byte output.
    unsafe {
        let Some(length) = terminated(phonemes, WORD_PHONEMES) else {
            return -1;
        };
        ptr::copy_nonoverlapping(phonemes, host.output, length + 1);
        if joined != 0 {
            *host.output.add(WORD_PHONEMES - 1) = 0;
        }
    }
    0
}
unsafe extern "C" fn word_change_stress(opaque: *mut c_void, level: i32) -> i32 {
    let host = host(opaque);
    // SAFETY: the live translator and its output.
    unsafe { change_word_stress(host.tr, host.output, level) };
    0
}
unsafe extern "C" fn word_special(opaque: *mut c_void, flags: u32) -> i32 {
    let host = host(opaque);
    // SAFETY: the live translator and its output.
    unsafe { apply_special_attribute2(host.tr, host.output, flags) };
    0
}
/// `ApplySpecialAttribute2`.
unsafe fn apply_special_attribute2(tr: *mut Translator, phonemes: *mut u8, flags: u32) {
    // SAFETY: the caller's terminated phonemes and the live translator.
    unsafe {
        super::espeak_rs_special_attribute(
            phonemes,
            libc::strlen(phonemes.cast()) + 1,
            addr_of!(synth::phoneme_tab).cast(),
            synth::n_phoneme_tab as usize,
            (*tr).langopts.param[ALTERNATE],
            flags,
            u32::from(c_char::MIN != 0),
        );
    }
}

/// `TranslateWord3`.
///
/// # Safety
/// As the C function; serialized engine.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn espeak_rs_translate_word3_engine(
    tr: *mut Translator,
    word_start: *mut u8,
    rows: *mut Row,
    remaining: c_int,
    word_out: *mut u8,
    any_stressed: *mut bool,
    alphabet: *const Alphabet,
    phonemes: *mut u8,
    size: usize,
) -> c_int {
    if tr.is_null() || phonemes.is_null() || size < WORD_PHONEMES {
        return 0;
    }
    let mut empty = [Row::default(); 8];
    let mut host = Host {
        rows: if rows.is_null() {
            empty.as_mut_ptr()
        } else {
            rows
        },
        remaining: if rows.is_null() { 0 } else { remaining },
        any_stressed,
        alphabet,
        output: phonemes,
        text_output: word_out,
        ..Host::new(tr)
    };
    // SAFETY: the live translator; the caller's terminated word.
    if unsafe { !(*tr).data_dictlist.is_null() && host.window(word_start) != 0 } {
        return 0;
    }
    let table = word::Callbacks {
        context: addr_of_mut!(host).cast(),
        byte: Some(word_byte),
        write: Some(word_write),
        value: Some(word_value),
        store: Some(word_store),
        locale: Some(word_locale),
        list: Some(word_list),
        emoji: Some(word_emoji),
        text: Some(word_text),
        dotted: Some(word_dotted),
        number_language: Some(word_number_language),
        number: Some(word_number),
        spell: Some(word_spell),
        letter: Some(word_letter),
        unpronounceable: Some(word_unpronounceable),
        spelling_stress: Some(word_spelling_stress),
        rules: Some(word_rules),
        remove: Some(word_remove),
        prefix: Some(word_prefix),
        trace_suffix: Some(word_trace_suffix),
        append: Some(word_append),
        plural: Some(word_plural),
        stress: Some(word_stress),
        snapshot: Some(word_snapshot),
        publish: Some(word_publish),
        change_stress: Some(word_change_stress),
        special: Some(word_special),
    };
    let mut result = 0u32;
    // SAFETY: the table and host outlive the call.
    if unsafe { word::espeak_rs_translate_word(&table, &mut result) } == 0 {
        result as c_int
    } else {
        0
    }
}

/// `ChangeWordStress`.
unsafe fn change_word_stress(tr: *mut Translator, word: *mut u8, level: c_int) {
    // SAFETY: the caller's 200-byte phonemes and the live translator.
    unsafe {
        if let Some(length) = terminated(word, WORD_PHONEMES) {
            super::espeak_rs_change_stress(
                word,
                length + 1,
                addr_of!(synth::phoneme_tab).cast(),
                (*tr).langopts.stress_flags as u32,
                level,
            );
        }
    }
}

// The emoji host (RustEmojiFallback).
struct EmojiHost {
    source: Host,
    wordptr: *mut *mut u8,
}
fn emoji_host<'a>(opaque: *mut c_void) -> &'a mut EmojiHost {
    // SAFETY: every callback receives the live EmojiHost its entry made.
    unsafe { &mut *opaque.cast::<EmojiHost>() }
}
unsafe extern "C" fn emoji_byte(opaque: *mut c_void, offset: usize) -> i32 {
    if offset > isize::MAX as usize {
        return -1;
    }
    let host = emoji_host(opaque);
    // SAFETY: the host's word source.
    unsafe {
        word_byte(
            addr_of_mut!(host.source).cast(),
            Source {
                slot: 0,
                offset: offset as isize,
            },
        )
    }
}
unsafe extern "C" fn emoji_list(
    opaque: *mut c_void,
    input: *mut u8,
    phonemes: *mut u8,
    flags: *mut u32,
    text: *mut u8,
) -> i32 {
    let host = emoji_host(opaque);
    // SAFETY: the fallback's buffers and the live translator.
    unsafe {
        let mut word = input;
        c::LookupDictListBounded(
            host.source.tr.cast(),
            addr_of_mut!(word).cast(),
            phonemes.cast(),
            flags,
            ALLOW_TEXT_MODE,
            host.source.rows.cast(),
            host.source.remaining,
            WORD_PHONEMES,
        );
        if *flags & TEXT_MODE == 0 {
            return 0;
        }
        let address = word as usize;
        let mut base = input as usize;
        let available = if address >= base && address - base < WORD_BYTES + 2 {
            WORD_BYTES + 2 - (address - base)
        } else {
            base = addr_of!((*host.source.tr).rust_list_replacement) as usize;
            if address < base || address - base >= WORD_BYTES {
                return -1;
            }
            WORD_BYTES - (address - base)
        };
        let Some(length) = terminated(word, available) else {
            return -1;
        };
        ptr::copy_nonoverlapping(word, text, length + 1);
    }
    0
}
unsafe extern "C" fn emoji_write(
    opaque: *mut c_void,
    offset: usize,
    bytes: *const u8,
    length: usize,
) -> i32 {
    if offset > WORD_BYTES || length > WORD_BYTES - offset {
        return -1;
    }
    let host = emoji_host(opaque);
    // SAFETY: the translator's replacement buffer, in bounds.
    unsafe {
        ptr::copy_nonoverlapping(
            bytes,
            addr_of_mut!((*host.source.tr).rust_emoji_replacement)
                .cast::<u8>()
                .add(offset),
            length,
        );
    }
    0
}
unsafe extern "C" fn emoji_tracing(_: *mut c_void) -> i32 {
    i32::from(c::tracing())
}
unsafe extern "C" fn emoji_snapshot(opaque: *mut c_void, output: *mut u8) -> i32 {
    let host = emoji_host(opaque);
    // SAFETY: the translator's replacement buffer and the fallback's output.
    unsafe {
        ptr::copy_nonoverlapping(
            addr_of!((*host.source.tr).rust_emoji_replacement).cast::<u8>(),
            output,
            WORD_BYTES,
        );
    }
    0
}
unsafe extern "C" fn emoji_trace(_: *mut c_void, source: *const u8, replacement: *const u8) {
    // SAFETY: the fallback's terminated strings.
    let (source, replacement) = unsafe { (c::text(source), c::text(replacement.add(2))) };
    c::trace(&[b"Replace: ", source, b"  ", replacement, b"\n"]);
}
unsafe extern "C" fn emoji_finish(opaque: *mut c_void) -> i32 {
    let host = emoji_host(opaque);
    // SAFETY: the caller's cursor and the translator's replacement buffer.
    unsafe {
        *host.wordptr = addr_of_mut!((*host.source.tr).rust_emoji_replacement)
            .cast::<u8>()
            .add(2);
    }
    0
}
/// `LookupEmojiBaseSequence`.
unsafe fn lookup_emoji_base_sequence(
    tr: *mut Translator,
    wordptr: *mut *mut u8,
    flags: *mut u32,
    rows: *mut Row,
    remaining: c_int,
) -> bool {
    // SAFETY: the cursor slot is read only after its null check.
    if tr.is_null() || wordptr.is_null() || unsafe { (*wordptr).is_null() } || flags.is_null() {
        return false;
    }
    let mut host = EmojiHost {
        source: Host {
            rows,
            remaining,
            ..Host::new(tr)
        },
        wordptr,
    };
    // SAFETY: the caller's terminated word.
    if unsafe { host.source.window(*wordptr) } != 0 {
        return false;
    }
    let table = emoji::Callbacks {
        context: addr_of_mut!(host).cast(),
        byte: Some(emoji_byte),
        list: Some(emoji_list),
        write: Some(emoji_write),
        tracing: Some(emoji_tracing),
        snapshot: Some(emoji_snapshot),
        trace: Some(emoji_trace),
        finish: Some(emoji_finish),
    };
    // SAFETY: the table and host outlive the call.
    unsafe { emoji::espeak_rs_lookup_emoji(&table, flags) == 1 }
}

// The spelling and plural host (RustWordHelpers).
struct HelpersHost {
    source: Host,
    phonemes: *mut u8,
    alphabet: *const Alphabet,
}
fn helpers_host<'a>(opaque: *mut c_void) -> &'a mut HelpersHost {
    // SAFETY: every callback receives the live HelpersHost its entry made.
    unsafe { &mut *opaque.cast::<HelpersHost>() }
}
unsafe extern "C" fn helpers_byte(opaque: *mut c_void, source: Source) -> i32 {
    // SAFETY: the host's word source.
    unsafe { word_byte(addr_of_mut!(helpers_host(opaque).source).cast(), source) }
}
unsafe extern "C" fn helpers_letter(
    opaque: *mut c_void,
    source: Source,
    control: u32,
    count: *mut usize,
) -> i32 {
    let host = helpers_host(opaque);
    let word = host.source.pointer(source);
    if word.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the host's phonemes.
    unsafe {
        let consumed = translate_letter(
            host.source.tr,
            word,
            host.phonemes,
            control as c_int,
            host.alphabet,
        );
        if consumed < 0 {
            return -1;
        }
        *count = consumed as usize;
    }
    0
}
unsafe extern "C" fn helpers_leading(opaque: *mut c_void) -> u8 {
    // SAFETY: the host's phonemes.
    unsafe { *helpers_host(opaque).phonemes }
}
unsafe extern "C" fn helpers_copy_switch(opaque: *mut c_void) -> i32 {
    let host = helpers_host(opaque);
    // SAFETY: the host's 200-byte phonemes and output.
    unsafe {
        let Some(length) = terminated(host.phonemes, WORD_PHONEMES) else {
            return -1;
        };
        ptr::copy(host.phonemes, host.source.output, length + 1);
    }
    0
}
unsafe extern "C" fn helpers_stress(opaque: *mut c_void, mode: i32, count: i32) -> i32 {
    let host = helpers_host(opaque);
    // SAFETY: the live translator and the host's phonemes.
    unsafe { SetSpellingStress(host.source.tr.cast(), host.phonemes.cast(), mode, count) };
    0
}
unsafe extern "C" fn helpers_plural(opaque: *mut c_void, source: *mut u8) -> i32 {
    let host = helpers_host(opaque);
    // SAFETY: the live translator, whose rule source is the 5-byte plural
    // text for the call and restored after it.
    unsafe {
        let owner = &mut *host.source.tr;
        let saved = (owner.rule_text_base, owner.rule_text_length);
        owner.rule_text_base = source;
        owner.rule_text_length = 5;
        c::TranslateRules(
            host.source.tr.cast(),
            source.add(2).cast(),
            host.source.output.cast(),
            WORD_PHONEMES as c_int,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        );
        let owner = &mut *host.source.tr;
        (owner.rule_text_base, owner.rule_text_length) = saved;
    }
    0
}
fn helpers_table(host: &mut HelpersHost) -> helpers::Callbacks {
    helpers::Callbacks {
        context: (host as *mut HelpersHost).cast(),
        byte: Some(helpers_byte),
        letter: Some(helpers_letter),
        leading: Some(helpers_leading),
        copy_switch: Some(helpers_copy_switch),
        stress: Some(helpers_stress),
        plural: Some(helpers_plural),
    }
}
/// `SpeakIndividualLetters`: the word after the spelled letters, or null
/// when the spelling switched language.
unsafe fn speak_individual_letters(
    tr: *mut Translator,
    word: *mut u8,
    phonemes: *mut u8,
    spell: c_int,
    alphabet: *const Alphabet,
    output: *mut u8,
) -> *mut u8 {
    if tr.is_null() || phonemes.is_null() || output.is_null() {
        return ptr::null_mut();
    }
    let mut host = HelpersHost {
        source: Host {
            output,
            ..Host::new(tr)
        },
        phonemes,
        alphabet,
    };
    // SAFETY: the caller's terminated word.
    if unsafe { host.source.window(word) } != 0 {
        return ptr::null_mut();
    }
    let table = helpers_table(&mut host);
    let mut result = Source::default();
    let mut switched = 0;
    // SAFETY: the table and host outlive the call.
    if unsafe { helpers::espeak_rs_word_spell(&table, spell, &mut result, &mut switched) } != 0
        || switched != 0
    {
        return ptr::null_mut();
    }
    host.source.pointer(result)
}
/// `addPluralSuffixes`.
unsafe fn add_plural_suffixes(flags: c_int, tr: *mut Translator, last: u8, output: *mut u8) {
    if tr.is_null() || output.is_null() {
        return;
    }
    let mut host = HelpersHost {
        source: Host {
            output,
            ..Host::new(tr)
        },
        phonemes: ptr::null_mut(),
        alphabet: ptr::null(),
    };
    let table = helpers_table(&mut host);
    // SAFETY: the table and host outlive the call.
    unsafe { helpers::espeak_rs_word_plural(&table, flags as u32, last) };
}

// The isolated-letter host (RustTranslateLetter).
struct IsolatedHost {
    tr: *mut Translator,
    output: *mut u8,
}
fn isolated_host<'a>(opaque: *mut c_void) -> &'a IsolatedHost {
    // SAFETY: every callback receives the live IsolatedHost its entry made.
    unsafe { &*opaque.cast::<IsolatedHost>() }
}
fn isolated_owner(opaque: *mut c_void, which: u32) -> *mut Translator {
    // SAFETY: serialized engine translators.
    unsafe {
        match which {
            0 => isolated_host(opaque).tr,
            1 => c::translator.cast(),
            2 => c::translator3.cast(),
            _ => ptr::null_mut(),
        }
    }
}
unsafe extern "C" fn isolated_value(opaque: *mut c_void, field: u32) -> i32 {
    let tr = isolated_host(opaque).tr;
    // SAFETY: the live translators (the main one checked on entry).
    unsafe {
        let main = &*c::translator.cast::<Translator>();
        let own = &*tr;
        match field {
            0 => main.phoneme_tab_ix,
            1 => i32::from(c::translator.cast::<Translator>() == tr),
            2 => main.letter_bits_offset,
            3 => main.langopts.alt_alphabet,
            4 => main.langopts.our_alphabet,
            5 => own.translator_name,
            6 => own.phoneme_tab_ix,
            7 => own.langopts.accents,
            8 => i32::from(own.langopts.dotless_i),
            9 => i32::from(!c::translator3.is_null()),
            10 => main.langopts.alt_alphabet_lang,
            _ => 0,
        }
    }
}
unsafe extern "C" fn isolated_classify(_: *mut c_void, code: u32, kind: u32) -> i32 {
    let class = match kind {
        0 => super::ucd_isupper(code),
        1 => super::ucd_isalpha(code),
        _ => super::ucd_isspace(code),
    };
    i32::from(class != 0)
}
unsafe extern "C" fn isolated_named(
    opaque: *mut c_void,
    which: u32,
    key: *const u8,
    capacity: usize,
    output: *mut u8,
    flags: *mut i32,
) -> i32 {
    let owner = isolated_owner(opaque, which);
    if owner.is_null() || capacity == 0 || capacity > 200 {
        return -1;
    }
    // SAFETY: a live translator, the host's terminated key and output.
    unsafe { *flags = c::LookupBounded(owner.cast(), key.cast(), output.cast(), capacity) };
    0
}
unsafe extern "C" fn isolated_letter(
    opaque: *mut c_void,
    which: u32,
    code: u32,
    next: i32,
    control: u32,
    capacity: usize,
    output: *mut u8,
) -> i32 {
    let owner = isolated_owner(opaque, which);
    if owner.is_null() || capacity == 0 || capacity > 200 {
        return -1;
    }
    // SAFETY: a live translator and the host's output.
    unsafe {
        espeak_rs_lookup_letter_engine(owner, code, next, control as i32, 0, output, capacity)
    };
    0
}
unsafe extern "C" fn isolated_secondary(_: *mut c_void, name: *const u8) -> i32 {
    // SAFETY: serialized engine; the host's terminated name.
    unsafe { c::SetTranslator3(name.cast()) }
}
unsafe extern "C" fn isolated_restore(_: *mut c_void) {
    // SAFETY: serialized engine voice.
    unsafe {
        synth::SelectPhonemeTable((*c::voice.cast::<crate::voice::Voice>()).phoneme_table);
    }
}
unsafe extern "C" fn isolated_hangul(_: *mut c_void, source: *mut u8, output: *mut u8) -> i32 {
    // SAFETY: serialized engine; the secondary translator's rule source is
    // the 12-byte Hangul text for the call and restored after it.
    unsafe {
        let owner = c::translator3.cast::<Translator>();
        if owner.is_null() {
            return -1;
        }
        let saved = ((*owner).rule_text_base, (*owner).rule_text_length);
        (*owner).rule_text_base = source;
        (*owner).rule_text_length = 12;
        c::TranslateRules(
            owner.cast(),
            source.add(1).cast(),
            output.cast(),
            77,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        );
        ((*owner).rule_text_base, (*owner).rule_text_length) = saved;
        if c::translator3.is_null() {
            return -1;
        }
        c::SetWordStress(c::translator3, output.cast(), ptr::null_mut(), -1, 0);
    }
    0
}
unsafe extern "C" fn isolated_encode(_: *mut c_void, text: *const u8, output: *mut u8) -> i32 {
    // SAFETY: the host's terminated text and 200-byte output.
    if unsafe { EncodePhonemesBounded(text.cast(), output.cast(), 200, ptr::null_mut()) }.is_null()
    {
        -1
    } else {
        0
    }
}
unsafe extern "C" fn isolated_publish(opaque: *mut c_void, replace: u32, text: *const u8) -> i32 {
    let output = isolated_host(opaque).output;
    // SAFETY: the host's text (200 bytes) and phoneme output (200 bytes).
    unsafe {
        let Some(size) = terminated(text, 200) else {
            return -1;
        };
        if replace != 0 {
            ptr::copy_nonoverlapping(text, output, size + 1);
            return 0;
        }
        let Some(used) = terminated(output, WORD_PHONEMES) else {
            return -1;
        };
        if used + size < WORD_PHONEMES {
            ptr::copy_nonoverlapping(text, output.add(used), size + 1);
        }
    }
    0
}
/// `TranslateLetter`: the bytes of `word` the letter used.
unsafe fn translate_letter(
    tr: *mut Translator,
    word: *mut u8,
    phonemes: *mut u8,
    control: c_int,
    alphabet: *const Alphabet,
) -> c_int {
    // SAFETY: serialized engine translator.
    if tr.is_null() || unsafe { c::translator.is_null() } || word.is_null() || phonemes.is_null() {
        return 0;
    }
    let mut code = 0;
    // SAFETY: the caller's terminated word.
    let consumed = unsafe { utf8_in(&mut code, word.cast()) };
    let mut result = consumed;
    let mut host = IsolatedHost {
        tr,
        output: phonemes,
    };
    let table = letter::Callbacks {
        context: addr_of_mut!(host).cast(),
        value: Some(isolated_value),
        classify: Some(isolated_classify),
        named: Some(isolated_named),
        letter: Some(isolated_letter),
        secondary: Some(isolated_secondary),
        restore: Some(isolated_restore),
        hangul: Some(isolated_hangul),
        encode: Some(isolated_encode),
        publish: Some(isolated_publish),
    };
    // SAFETY: the table and host outlive the call; the word's next byte.
    unsafe {
        let next = c_int::from(*word.add(consumed as usize).cast::<c_char>());
        let current = if alphabet.is_null() {
            u32::MAX
        } else {
            (*alphabet).range_min
        };
        letter::espeak_rs_translate_letter(
            &table,
            code as u32,
            next,
            control as u32,
            current,
            consumed,
            &mut result,
        );
    }
    result
}

// The admission host (RustWordAdmission).
unsafe extern "C" fn admission_value(opaque: *mut c_void, field: u32) -> i32 {
    let tr = host(opaque).tr;
    if tr.is_null() {
        return 0;
    }
    // SAFETY: the live translator.
    unsafe {
        let tr = &*tr;
        match field {
            0 => tr.letter_bits_offset,
            1 => tr.langopts.param[UNPRONOUNCABLE],
            2 => i32::from(tr.langopts.max_initial_consonants),
            _ => 0,
        }
    }
}
unsafe extern "C" fn admission_alpha(_: *mut c_void, code: u32) -> i32 {
    i32::from(super::ucd_isalpha(code) != 0)
}
unsafe extern "C" fn admission_vowel(opaque: *mut c_void, code: u32) -> i32 {
    let tr = host(opaque).tr;
    if tr.is_null() {
        return 0;
    }
    // SAFETY: the live translator.
    unsafe { IsVowel(tr.cast(), code as c_int) }
}
unsafe extern "C" fn admission_probe(
    opaque: *mut c_void,
    scratch: *mut u8,
    result: *mut i32,
) -> i32 {
    let host = host(opaque);
    let source = host.pointer(Source { slot: 0, offset: 0 });
    if host.tr.is_null() || source.is_null() {
        return -1;
    }
    // SAFETY: an admitted word and the host's scratch.
    unsafe {
        *result = c::TranslateRules(
            host.tr.cast(),
            source.cast(),
            scratch.cast(),
            WORD_PHONEMES as c_int,
            ptr::null_mut(),
            UNPRONOUNCEABLE_TEST as c_int,
            ptr::null_mut(),
        );
    }
    0
}
unsafe extern "C" fn admission_skip(_: *mut c_void, value: i32) {
    // SAFETY: serialized engine option.
    unsafe { c::dictionary_skipwords = value };
}
fn admission_table(host: &mut Host) -> admission::Callbacks {
    admission::Callbacks {
        context: (host as *mut Host).cast(),
        byte: Some(word_byte),
        write: Some(word_write),
        value: Some(admission_value),
        alpha: Some(admission_alpha),
        vowel: Some(admission_vowel),
        probe: Some(admission_probe),
        skip: Some(admission_skip),
    }
}
/// `CheckDottedAbbrev`.
unsafe fn check_dotted_abbrev(word: *mut u8) -> c_int {
    let mut host = Host::new(ptr::null_mut());
    // SAFETY: the caller's terminated word.
    if unsafe { host.window(word) } != 0 {
        return 0;
    }
    let table = admission_table(&mut host);
    let mut result = 0;
    // SAFETY: the table and host outlive the call.
    if unsafe { admission::espeak_rs_word_dotted(&table, &mut result) } == 0 {
        result
    } else {
        0
    }
}
/// `Unpronouncable`.
unsafe fn unpronounceable(tr: *mut Translator, word: *mut u8, position: c_int) -> c_int {
    if tr.is_null() {
        return 0;
    }
    let mut host = Host::new(tr);
    // SAFETY: the caller's terminated word.
    if unsafe { host.window(word) } != 0 {
        return 0;
    }
    let table = admission_table(&mut host);
    let mut result = 0;
    // SAFETY: the table and host outlive the call.
    if unsafe { admission::espeak_rs_word_unpronounceable(&table, position, &mut result) } == 0 {
        result
    } else {
        0
    }
}
