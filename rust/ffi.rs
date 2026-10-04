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
