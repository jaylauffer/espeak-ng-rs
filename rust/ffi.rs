//! Legacy C ABI adapter. The decoder borrows C buffers without copying them.
//!
//! Callers must supply live, readable buffers of the declared size, keep them
//! alive until rebinding/destroying the decoder, and serialize decoder access.
//! Mnemonic tables must end with a NULL name. Strings must be NUL-terminated.
//! All exported integer discriminants are validated before using Rust enums.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::{Encoding, Mode, State};
use crate::unicode::{self, Category};
use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;

const UNKNOWN_ENCODING: c_int = 0x100010ff;
const INVALID_ARGUMENT: c_int = 22;

type RulePredicate = unsafe extern "C" fn(*mut c_void, u32, u32, usize, u32) -> i32;
type PrefixFlags = unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut u32);
type RuleTrace = unsafe extern "C" fn(*mut c_void, usize, usize, i32);
#[repr(C)]
struct RawLetters {
    bits: *const [u8; 256],
    groups: *const [*const c_void; 8],
    lengths: *const [usize; 8],
    offset: i32,
    wide_bytes: u32,
}
impl RawLetters {
    /// Host retains aligned, immutable configuration and wide lists throughout
    /// the synchronous call. Lengths exclude each list's terminating NUL.
    unsafe fn borrow(&self) -> Option<crate::letters::LetterSet<'_>> {
        if self.bits.is_null()
            || self.groups.is_null()
            || self.lengths.is_null()
            || !matches!(self.wide_bytes, 2 | 4)
        {
            return None;
        }
        // SAFETY: host supplies three live arrays of the declared fixed sizes.
        let (bits, pointers, lengths) = unsafe { (&*self.bits, &*self.groups, &*self.lengths) };
        let mut groups = [None; 8];
        for (index, &pointer) in pointers.iter().enumerate() {
            if pointer.is_null() {
                continue;
            }
            let length = lengths[index];
            if length > isize::MAX as usize / self.wide_bytes as usize {
                return None;
            }
            // SAFETY: each non-null pointer has length aligned readable units
            // of the host's wchar_t width, immutable until this borrow ends.
            groups[index] = Some(unsafe {
                if self.wide_bytes == 2 {
                    crate::letters::WideLetters::U16(std::slice::from_raw_parts(
                        pointer.cast(),
                        length,
                    ))
                } else {
                    crate::letters::WideLetters::U32(std::slice::from_raw_parts(
                        pointer.cast(),
                        length,
                    ))
                }
            });
        }
        Some(crate::letters::LetterSet {
            bits,
            offset: self.offset,
            groups,
        })
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_is_letter(
    letters: *const RawLetters,
    letter: i32,
    group: u32,
) -> c_int {
    if letters.is_null() {
        return 0;
    }
    // SAFETY: host supplies one aligned configuration valid for this call.
    unsafe { (&*letters).borrow() }.map_or(0, |set| c_int::from(set.mask(letter, group)))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_letter_group(
    patterns: *const u8,
    length: usize,
    text: *const u8,
    text_length: usize,
    position: usize,
    backwards: c_int,
) -> c_int {
    if patterns.is_null()
        || text.is_null()
        || length > isize::MAX as usize
        || text_length > isize::MAX as usize
    {
        return -1;
    }
    // SAFETY: the synchronous host declares readable resident patterns and text
    // of these lengths; neither buffer is modified during matching.
    let (patterns, text) = unsafe {
        (
            std::slice::from_raw_parts(patterns, length),
            std::slice::from_raw_parts(text, text_length),
        )
    };
    crate::rule_match::letter_group(patterns, text, position, backwards != 0)
        .ok()
        .flatten()
        .and_then(|n| c_int::try_from(n).ok())
        .unwrap_or(-1)
}
struct RuleHost<'a> {
    letters: crate::letters::LetterSet<'a>,
    opaque: *mut c_void,
    predicate: RulePredicate,
    prefix: PrefixFlags,
    trace: RuleTrace,
}
impl crate::rule_match::Environment for RuleHost<'_> {
    fn is_letter(&mut self, code: u32, group: u8) -> bool {
        self.letters.is_letter(code, group)
    }
    fn letter_group(
        &mut self,
        _text: &[u8],
        position: usize,
        group: u8,
        backwards: bool,
    ) -> Option<usize> {
        // SAFETY: matcher bounds-checks positions in the declared host text.
        let result = unsafe {
            (self.predicate)(
                self.opaque,
                1,
                u32::from(backwards),
                position,
                u32::from(group),
            )
        };
        usize::try_from(result).ok()
    }
    fn prefix_flags(&mut self, prefix: &[u8]) -> [u32; 2] {
        let mut flags = [0; 2];
        // SAFETY: prefix is a live NUL-terminated stack slice for this callback;
        // flags reserves two exclusively borrowed output words.
        unsafe {
            (self.prefix)(
                self.opaque,
                prefix.as_ptr(),
                prefix.len(),
                flags.as_mut_ptr(),
            )
        };
        flags
    }
    fn trace(&mut self, template: usize, phonemes: Option<usize>, points: i32) {
        // SAFETY: offsets borrow the live rule storage held by the C host.
        unsafe {
            (self.trace)(
                self.opaque,
                template,
                phonemes.unwrap_or(usize::MAX),
                points,
            )
        };
    }
}
#[repr(C)]
struct RawRuleMatch {
    phonemes: usize,
    delete_offset: usize,
    advance: usize,
    points: i32,
    ending: i32,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_match_group(
    rules: *const u8,
    rules_length: usize,
    text: *const u8,
    text_length: usize,
    position: usize,
    group_length: usize,
    context: *const crate::rule_match::Context,
    letters: *const RawLetters,
    opaque: *mut c_void,
    predicate: Option<RulePredicate>,
    prefix: Option<PrefixFlags>,
    trace: Option<RuleTrace>,
    out: *mut RawRuleMatch,
) -> c_int {
    let (Some(predicate), Some(prefix), Some(trace)) = (predicate, prefix, trace) else {
        return 2;
    };
    if rules.is_null()
        || text.is_null()
        || context.is_null()
        || letters.is_null()
        || out.is_null()
        || rules_length > isize::MAX as usize
        || text_length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: caller declares live, readable and immutable text/rule buffers
    // and aligned context, valid throughout this synchronous execution.
    let (rules, text, context) = unsafe {
        (
            std::slice::from_raw_parts(rules, rules_length),
            std::slice::from_raw_parts(text, text_length),
            &*context,
        )
    };
    // SAFETY: caller retains immutable language arrays throughout matching.
    let Some(letters) = (unsafe { (&*letters).borrow() }) else {
        return 2;
    };
    let mut host = RuleHost {
        letters,
        opaque,
        predicate,
        prefix,
        trace,
    };
    let Ok(result) =
        crate::rule_match::match_group(rules, text, position, group_length, context, &mut host)
    else {
        return 2;
    };
    // SAFETY: caller provides one aligned, exclusive output record.
    unsafe {
        out.write(RawRuleMatch {
            phonemes: result.phonemes.unwrap_or(usize::MAX),
            delete_offset: result.delete.unwrap_or(usize::MAX),
            advance: result.advance,
            points: result.points,
            ending: result.ending,
        })
    };
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_transpose(
    text: *mut u8,
    length: usize,
    min: u32,
    max: u32,
    map: *const u8,
    map_length: usize,
    pairs: *const i16,
    pairs_length: usize,
) -> c_int {
    if text.is_null()
        || length > isize::MAX as usize
        || map_length > isize::MAX as usize
        || pairs_length > isize::MAX as usize / 2
    {
        return -1;
    }
    // SAFETY: adapter declares exclusive NUL-containing text storage and live
    // map/pair arrays. All three allocations are disjoint.
    let text = unsafe { std::slice::from_raw_parts_mut(text, length) };
    let map = if map.is_null() {
        None
    } else {
        // SAFETY: the map has map_length readable byte entries.
        Some(unsafe { std::slice::from_raw_parts(map, map_length) })
    };
    let pairs = if pairs.is_null() {
        &[]
    } else {
        // SAFETY: pairs contains pairs_length aligned readable i16 entries.
        unsafe { std::slice::from_raw_parts(pairs, pairs_length) }
    };
    crate::word_key::Alphabet {
        min,
        max,
        map,
        pairs,
    }
    .transpose(text)
    .map_or(-1, |length| length as c_int)
}

#[repr(C)]
struct RawLookupOutcome {
    phonemes_offset: usize,
    phonemes_length: usize,
    word_end: usize,
    flags: [u32; 2],
    trace_flags: [u32; 2],
    copied: u32,
    has_flags: u32,
    found: u32,
    skipwords: i32,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_bucket(
    bucket: *const u8,
    length: usize,
    key: *const c_char,
    descriptor: usize,
    next: *const c_char,
    next_length: usize,
    context: *const crate::lookup::Context,
    words: *const crate::lookup::WordInfo,
    word_count: usize,
    out: *mut RawLookupOutcome,
) -> c_int {
    if bucket.is_null()
        || key.is_null()
        || next.is_null()
        || context.is_null()
        || out.is_null()
        || length > isize::MAX as usize
        || next_length > isize::MAX as usize
        || word_count > 20
    {
        return 2;
    }
    // SAFETY: the C adapter borrows a validated resident bucket of this length.
    let bucket = unsafe { std::slice::from_raw_parts(bucket, length) };
    // SAFETY: compressed keys contain embedded NULs. The adapter guarantees
    // the descriptor's byte-count prefix is readable; it must not be shortened
    // by CStr. Next words are an independently bounded window.
    let (key, next, context) = unsafe {
        (
            std::slice::from_raw_parts(key.cast::<u8>(), descriptor & 0x3f),
            std::slice::from_raw_parts(next.cast::<u8>(), next_length),
            &*context,
        )
    };
    let words = if words.is_null() {
        None
    } else {
        // SAFETY: the word snapshot contains word_count initialized entries.
        Some(unsafe { std::slice::from_raw_parts(words, word_count) })
    };
    let Ok(result) = crate::lookup::lookup_bucket(bucket, key, descriptor, next, context, words)
    else {
        return 2;
    };
    let phonemes = result.phonemes.unwrap_or(&[]);
    let offset = if phonemes.is_empty() {
        0
    } else {
        phonemes.as_ptr() as usize - bucket.as_ptr() as usize
    };
    // SAFETY: caller reserves one exclusive output record of the C-declared layout.
    unsafe {
        out.write(RawLookupOutcome {
            phonemes_offset: offset,
            phonemes_length: phonemes.len(),
            word_end: result.word_end.unwrap_or(0),
            flags: result.flags.unwrap_or([0; 2]),
            trace_flags: result.trace_flags.unwrap_or([0; 2]),
            copied: u32::from(result.phonemes.is_some()),
            has_flags: u32::from(result.flags.is_some()),
            found: u32::from(result.word_end.is_some()),
            skipwords: result.skipwords.map_or(-1, |count| count as i32),
        })
    };
    0
}

#[no_mangle]
unsafe extern "C" fn HashDictionary(word: *const c_char) -> c_int {
    if word.is_null() {
        return 0;
    }
    // SAFETY: legacy API supplies a readable NUL-terminated word.
    crate::dictionary::hash(unsafe { CStr::from_ptr(word) }.to_bytes()) as c_int
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_dictionary_index(
    bytes: *const u8,
    length: usize,
    rules: *mut crate::rules::RuleIndex,
    buckets: *mut usize,
    rules_offset: *mut usize,
) -> c_int {
    if bytes.is_null()
        || rules.is_null()
        || buckets.is_null()
        || rules_offset.is_null()
        || length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: the adapter supplies length readable bytes and exclusive outputs.
    let data = unsafe { std::slice::from_raw_parts(bytes, length) };
    let Ok(dict) = crate::dictionary::Dictionary::parse(data) else {
        return 2;
    };
    let Ok(index) = dict.rule_index() else {
        return 2;
    };
    // SAFETY: outputs have the declared layout, with 1024 bucket slots.
    unsafe {
        rules.write(index);
        ptr::copy_nonoverlapping(dict.bucket_offsets().as_ptr(), buckets, 1024);
        rules_offset.write(dict.rules_offset());
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_create(
    bytes: *const u8,
    length: usize,
    tables: *mut crate::phoneme_data::TableMeta,
    count: *mut c_int,
) -> *mut crate::phoneme_data::TableIndex {
    if bytes.is_null() || tables.is_null() || count.is_null() || length > isize::MAX as usize {
        return ptr::null_mut();
    }
    // SAFETY: the adapter provides a resident buffer of length readable bytes.
    let Ok(index) = crate::phoneme_data::TableIndex::parse(unsafe {
        std::slice::from_raw_parts(bytes, length)
    }) else {
        return ptr::null_mut();
    };
    // SAFETY: caller reserves 150 metadata slots and one count.
    unsafe {
        ptr::copy_nonoverlapping(index.tables().as_ptr(), tables, index.tables().len());
        count.write(index.tables().len() as c_int);
    }
    Box::into_raw(Box::new(index))
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_destroy(index: *mut crate::phoneme_data::TableIndex) {
    if !index.is_null() {
        // SAFETY: the adapter frees each handle returned by create exactly once.
        drop(unsafe { Box::from_raw(index) });
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_select(
    index: *const crate::phoneme_data::TableIndex,
    bytes: *const u8,
    length: usize,
    number: c_int,
    slots: *mut usize,
) -> c_int {
    if index.is_null()
        || bytes.is_null()
        || slots.is_null()
        || number < 0
        || length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: live create handle, resident bytes and exclusive 256-slot output.
    let selected = unsafe { &*index }.select(
        unsafe { std::slice::from_raw_parts(bytes, length) },
        number as usize,
    );
    let Ok(selected) = selected else {
        return 2;
    };
    // SAFETY: exactly 256 exclusive usize output slots, as declared in the C header.
    unsafe { ptr::copy_nonoverlapping(selected.as_ptr(), slots, 256) };
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_lookup(
    index: *const crate::phoneme_data::TableIndex,
    name: *const c_char,
) -> c_int {
    if index.is_null() || name.is_null() {
        return -1;
    }
    // SAFETY: live index handle and readable NUL-terminated table name.
    unsafe { &*index }
        .lookup(unsafe { CStr::from_ptr(name) }.to_bytes())
        .map_or(-1, |number| number as c_int)
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phondata_header(
    bytes: *const u8,
    length: usize,
    fields: *mut u32,
) -> c_int {
    if bytes.is_null() || fields.is_null() || length > isize::MAX as usize {
        return 2;
    }
    // SAFETY: the adapter supplies length readable bytes and two exclusive u32 outputs.
    let Ok(header) =
        crate::phoneme_data::header(unsafe { std::slice::from_raw_parts(bytes, length) })
    else {
        return 2;
    };
    // SAFETY: the C adapter reserves exactly two u32 fields.
    unsafe { ptr::copy_nonoverlapping(header.as_ptr(), fields, 2) };
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_sample_rate(bytes: *const u8, length: usize) -> c_int {
    if bytes.is_null() || length > isize::MAX as usize {
        return -1;
    }
    // SAFETY: caller supplies length readable bytes of phondata.
    crate::phoneme_data::sample_rate(unsafe { std::slice::from_raw_parts(bytes, length) })
        .map_or(-1, |rate| rate as c_int)
}

#[no_mangle]
unsafe extern "C" fn phoneme_feature_from_string(name: *const c_char) -> u32 {
    if name.is_null() {
        return 0;
    }
    // SAFETY: caller supplies a NUL-terminated feature string.
    crate::phoneme::feature_from_name(unsafe { CStr::from_ptr(name) }.to_bytes())
}
#[no_mangle]
unsafe extern "C" fn phoneme_add_feature(
    phoneme: *mut crate::phoneme::Phoneme,
    feature: u32,
) -> c_int {
    if phoneme.is_null() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: caller supplies an aligned, exclusive PHONEME_TAB record.
    unsafe { &mut *phoneme }
        .add_feature(feature)
        .map_or(0x10000fff, |()| 0)
}

struct RawDecoder {
    input: *const u8,
    length: usize,
    state: State,
}

impl RawDecoder {
    unsafe fn read(&mut self) -> Option<u32> {
        if self.input.is_null() || self.state.offset >= self.length {
            return None;
        }
        // SAFETY: C's borrowed-buffer contract guarantees length readable bytes.
        let bytes = unsafe { std::slice::from_raw_parts(self.input, self.length) };
        self.state.read(bytes)
    }
}

#[no_mangle]
extern "C" fn create_text_decoder() -> *mut RawDecoder {
    // SAFETY: correct layout; allocation failure is checked before writing.
    let result = unsafe { alloc(Layout::new::<RawDecoder>()) }.cast::<RawDecoder>();
    if !result.is_null() {
        // SAFETY: newly allocated, aligned storage of the correct size.
        unsafe {
            result.write(RawDecoder {
                input: ptr::null(),
                length: 0,
                state: State::new(Encoding::UsAscii, Mode::Bytes).expect("known encoding"),
            });
        }
    }
    result
}

#[no_mangle]
unsafe extern "C" fn destroy_text_decoder(decoder: *mut RawDecoder) {
    if !decoder.is_null() {
        // SAFETY: pointer comes from create_text_decoder and is freed once.
        unsafe {
            dealloc(decoder.cast(), Layout::new::<RawDecoder>());
        }
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_ng_EncodingFromName(name: *const c_char) -> u32 {
    if name.is_null() {
        return Encoding::Unknown as u32;
    }
    // SAFETY: C supplies a live NUL-terminated string.
    unsafe { CStr::from_ptr(name) }
        .to_str()
        .map_or(0, |s| Encoding::from_name(s) as u32)
}

unsafe fn bind(
    decoder: *mut RawDecoder,
    input: *const u8,
    length: c_int,
    encoding: u32,
    mode: Mode,
) -> c_int {
    let Some(encoding) = Encoding::from_id(encoding) else {
        return UNKNOWN_ENCODING;
    };
    let Ok(state) = State::new(encoding, mode) else {
        return UNKNOWN_ENCODING;
    };
    if decoder.is_null() {
        return INVALID_ARGUMENT;
    }
    let length = if input.is_null() {
        0
    } else if length < 0 {
        // SAFETY: for a negative length the C contract requires a NUL string.
        unsafe { CStr::from_ptr(input.cast()) }
            .to_bytes_with_nul()
            .len()
    } else {
        length as usize
    };
    // SAFETY: decoder is a live, exclusively accessed instance from create.
    unsafe {
        *decoder = RawDecoder {
            input,
            length,
            state,
        };
    }
    0
}

#[no_mangle]
unsafe extern "C" fn text_decoder_decode_string(
    decoder: *mut RawDecoder,
    input: *const c_char,
    length: c_int,
    encoding: u32,
) -> c_int {
    // SAFETY: forwards the same borrowed-buffer contract.
    unsafe { bind(decoder, input.cast(), length, encoding, Mode::Bytes) }
}
#[no_mangle]
unsafe extern "C" fn text_decoder_decode_string_auto(
    decoder: *mut RawDecoder,
    input: *const c_char,
    length: c_int,
    encoding: u32,
) -> c_int {
    // SAFETY: forwards the same borrowed-buffer contract.
    unsafe { bind(decoder, input.cast(), length, encoding, Mode::Auto) }
}

#[cfg(windows)]
type WChar = u16;
#[cfg(not(windows))]
type WChar = u32;

#[no_mangle]
unsafe extern "C" fn text_decoder_decode_wstring(
    decoder: *mut RawDecoder,
    input: *const WChar,
    length: c_int,
) -> c_int {
    if decoder.is_null() {
        return INVALID_ARGUMENT;
    }
    let units = if input.is_null() {
        0
    } else if length < 0 {
        let mut count = 0;
        // SAFETY: C promises a live, NUL-terminated wchar_t array.
        while unsafe { input.add(count).read() } != 0 {
            count += 1;
        }
        count + 1
    } else {
        length as usize
    };
    let Some(length) = units
        .checked_mul(std::mem::size_of::<WChar>())
        .filter(|len| *len <= isize::MAX as usize)
    else {
        return INVALID_ARGUMENT;
    };
    let mode = if std::mem::size_of::<WChar>() == 2 {
        Mode::Wide16
    } else {
        Mode::Wide32
    };
    let state = State::new(Encoding::UsAscii, mode).expect("known encoding");
    // SAFETY: live, exclusively accessed decoder; input extent checked above.
    unsafe {
        *decoder = RawDecoder {
            input: input.cast(),
            length,
            state,
        };
    }
    0
}

#[no_mangle]
unsafe extern "C" fn text_decoder_decode_string_multibyte(
    decoder: *mut RawDecoder,
    input: *const c_void,
    encoding: u32,
    flags: c_int,
) -> c_int {
    // SAFETY: each mode inherits the corresponding C API buffer contract.
    unsafe {
        match flags & 7 {
            0 => text_decoder_decode_string_auto(decoder, input.cast(), -1, encoding),
            1 => text_decoder_decode_string(decoder, input.cast(), -1, Encoding::Utf8 as u32),
            2 => text_decoder_decode_string(decoder, input.cast(), -1, encoding),
            3 => text_decoder_decode_wstring(decoder, input.cast(), -1),
            4 => text_decoder_decode_string(decoder, input.cast(), -1, Encoding::Ucs2 as u32),
            _ => UNKNOWN_ENCODING,
        }
    }
}

#[no_mangle]
unsafe extern "C" fn text_decoder_eof(decoder: *mut RawDecoder) -> c_int {
    if decoder.is_null() {
        return 1;
    }
    // SAFETY: C supplies a live decoder with serialized access.
    let d = unsafe { &*decoder };
    c_int::from(d.state.offset >= d.length)
}
#[no_mangle]
unsafe extern "C" fn text_decoder_getc(decoder: *mut RawDecoder) -> u32 {
    if decoder.is_null() {
        return 0;
    }
    // SAFETY: C supplies a live decoder and retains its bound input.
    unsafe { (&mut *decoder).read() }.unwrap_or(0)
}
#[no_mangle]
unsafe extern "C" fn text_decoder_peekc(decoder: *mut RawDecoder) -> u32 {
    if decoder.is_null() {
        return 0;
    }
    // SAFETY: C supplies a live decoder with serialized access.
    let d = unsafe { &mut *decoder };
    let offset = d.state.offset;
    // SAFETY: C retains the input bound to this decoder.
    let result = unsafe { d.read() }.unwrap_or(0);
    d.state.offset = offset; // intentionally retains AUTO fallback selected by read
    result
}
#[no_mangle]
unsafe extern "C" fn text_decoder_get_buffer(decoder: *mut RawDecoder) -> *const c_void {
    if decoder.is_null() {
        return ptr::null();
    }
    // SAFETY: C supplies a live decoder with serialized access.
    let d = unsafe { &*decoder };
    if d.input.is_null() || d.state.offset >= d.length {
        return ptr::null();
    }
    // SAFETY: cursor is bounded by the bound input's extent.
    unsafe { d.input.add(d.state.offset) }.cast()
}

#[repr(C)]
struct Mnem {
    name: *const c_char,
    value: c_int,
}

#[no_mangle]
unsafe extern "C" fn LookupMnem(mut table: *const Mnem, name: *const c_char) -> c_int {
    if table.is_null() {
        return 0;
    }
    let name = if name.is_null() {
        None
    } else {
        // SAFETY: caller supplies a NUL-terminated name.
        Some(unsafe { CStr::from_ptr(name) }.to_bytes())
    };
    loop {
        // SAFETY: C supplies a readable table with a NULL-name sentinel.
        let item = unsafe { &*table };
        if item.name.is_null() {
            return item.value;
        }
        // SAFETY: each non-sentinel name is NUL-terminated.
        if name == Some(unsafe { CStr::from_ptr(item.name) }.to_bytes()) {
            return item.value;
        }
        // SAFETY: the sentinel contract permits visiting the next entry.
        table = unsafe { table.add(1) };
    }
}
#[no_mangle]
unsafe extern "C" fn LookupMnemName(mut table: *const Mnem, value: c_int) -> *const c_char {
    if !table.is_null() {
        loop {
            // SAFETY: C supplies a readable table with a NULL-name sentinel.
            let item = unsafe { &*table };
            if item.name.is_null() {
                break;
            }
            if item.value == value {
                return item.name;
            }
            // SAFETY: the sentinel contract permits visiting the next entry.
            table = unsafe { table.add(1) };
        }
    }
    c"".as_ptr()
}
#[no_mangle]
unsafe extern "C" fn ieee_extended_to_double(input: *const u8) -> f64 {
    if input.is_null() {
        return f64::NAN;
    }
    // SAFETY: caller supplies ten readable bytes; no alignment requirement.
    let bytes = unsafe { input.cast::<[u8; 10]>().read() };
    crate::ieee80::extended_to_double(&bytes)
}

macro_rules! unicode_exports {
    ($( $name:ident => $rust:ident ),* $(,)?) => { $(
        #[no_mangle]
        extern "C" fn $name(c: u32) -> c_int { c_int::from(unicode::$rust(c)) }
    )* };
}
unicode_exports! {
    ucd_isalnum => is_alnum, ucd_isalpha => is_alpha, ucd_isblank => is_blank,
    ucd_iscntrl => is_control, ucd_isdigit => is_digit, ucd_isgraph => is_graph,
    ucd_islower => is_lower, ucd_isprint => is_print, ucd_ispunct => is_punct,
    ucd_isspace => is_space, ucd_isupper => is_upper, ucd_isxdigit => is_hex_digit,
}
#[no_mangle]
extern "C" fn ucd_toupper(c: u32) -> u32 {
    unicode::to_upper(c)
}
#[no_mangle]
extern "C" fn ucd_tolower(c: u32) -> u32 {
    unicode::to_lower(c)
}
#[no_mangle]
extern "C" fn ucd_totitle(c: u32) -> u32 {
    unicode::to_title(c)
}
#[no_mangle]
extern "C" fn ucd_lookup_category(c: u32) -> u32 {
    unicode::category(c) as u32
}
#[no_mangle]
extern "C" fn ucd_lookup_category_group(c: u32) -> u32 {
    unicode::category(c).group()
}
#[no_mangle]
extern "C" fn ucd_lookup_script(c: u32) -> u32 {
    unicode::script(c)
}
#[no_mangle]
extern "C" fn ucd_get_category_group_for_category(cat: u32) -> u32 {
    Category::from_id(cat).map_or(1, Category::group)
}
#[no_mangle]
extern "C" fn ucd_properties(c: u32, cat: u32) -> u64 {
    Category::from_id(cat).map_or(0, |cat| unicode::properties(c, cat))
}
#[no_mangle]
extern "C" fn ucd_get_category_string(cat: u32) -> *const c_char {
    unicode::category_c_string(cat).as_ptr().cast()
}
#[no_mangle]
extern "C" fn ucd_get_script_string(id: u32) -> *const c_char {
    unicode::script_c_string(id).as_ptr().cast()
}
#[no_mangle]
extern "C" fn ucd_get_category_group_string(id: u32) -> *const c_char {
    [c"C", c"I", c"L", c"M", c"N", c"P", c"S", c"Z"]
        .get(id as usize)
        .copied()
        .unwrap_or(c"-")
        .as_ptr()
}
