//! Admit serialized initialized number source, dictionary and state primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::number_frontend::{self, Host};
use crate::number_lookup::{Error, PHONEME_BYTES};
use std::ffi::c_void;
#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    byte: Option<unsafe extern "C" fn(*mut c_void, isize) -> u8>,
    write: Option<unsafe extern "C" fn(*mut c_void, usize, u8)>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
    word: Option<unsafe extern "C" fn(*mut c_void, usize) -> u32>,
    lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    list: Option<unsafe extern "C" fn(*mut c_void, isize, *mut u8, *mut u32) -> i32>,
    text: Option<unsafe extern "C" fn(*mut c_void, u32) -> *const u8>,
    store_text: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8, usize) -> i32>,
    classify: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    translate: Option<unsafe extern "C" fn(*mut c_void, usize) -> u32>,
    missing: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    skip: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    phoneme_type: Option<unsafe extern "C" fn(*mut c_void, u8) -> i32>,
}
pub(super) struct Engine<'a>(&'a Callbacks);
impl<'a> Engine<'a> {
    /// SAFETY: pointer admits a live immutable table, serialized context and
    /// primitive contracts described in rust_number_frontend.h through return.
    pub(super) unsafe fn admit(table: *const Callbacks) -> Option<Self> {
        if table.is_null() {
            return None;
        }
        // SAFETY: caller admits live immutable callback storage.
        let table = unsafe { &*table };
        if table.context.is_null()
            || table.byte.is_none()
            || table.write.is_none()
            || table.value.is_none()
            || table.word.is_none()
            || table.lookup.is_none()
            || table.list.is_none()
            || table.text.is_none()
            || table.store_text.is_none()
            || table.classify.is_none()
            || table.translate.is_none()
            || table.missing.is_none()
            || table.skip.is_none()
            || table.phoneme_type.is_none()
        {
            None
        } else {
            Some(Self(table))
        }
    }
    pub(super) fn context(&self) -> *mut c_void {
        self.0.context
    }
}
impl Host for Engine<'_> {
    fn byte(&self, offset: isize) -> u8 {
        // SAFETY: admitted primitive bounds reads to the initialized source extent.
        unsafe { self.0.byte.unwrap()(self.0.context, offset) }
    }
    fn write_byte(&mut self, offset: usize, value: u8) {
        // SAFETY: admitted primitive bounds source mutation, no loan held.
        unsafe { self.0.write.unwrap()(self.0.context, offset, value) }
    }
    fn value(&self, field: u32) -> i32 {
        // SAFETY: admitted live scalar projection, known field in 0..=9.
        unsafe { self.0.value.unwrap()(self.0.context, field) }
    }
    fn word_flags(&self, index: usize) -> u32 {
        // SAFETY: primitive projects only initialized admitted word rows.
        unsafe { self.0.word.unwrap()(self.0.context, index) }
    }
    fn lookup(&mut self, key: &[u8], out: &mut [u8; PHONEME_BYTES]) -> i32 {
        // SAFETY: terminated key and initialized disjoint 200-byte scratch;
        // trusted dictionary writes a terminated string within this extent.
        unsafe { self.0.lookup.unwrap()(self.0.context, key.as_ptr(), out.as_mut_ptr()) }
    }
    fn list(&mut self, offset: isize, out: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2]) -> i32 {
        // SAFETY: primitive bounds original source; writable initialized scratch
        // and two flags are disjoint. No foreign source loan spans translation.
        unsafe {
            self.0.list.unwrap()(self.0.context, offset, out.as_mut_ptr(), flags.as_mut_ptr())
        }
    }
    fn text(&self, kind: u32, out: &mut [u8; PHONEME_BYTES]) -> Result<(), Error> {
        // SAFETY: fresh initialized prefix projection, known kind 0..=2.
        let pointer = unsafe { self.0.text.unwrap()(self.0.context, kind) };
        if pointer.is_null() {
            return if kind == 2 {
                out[0] = 0;
                Ok(())
            } else {
                Err(Error::Phonemes)
            };
        }
        let limit = if kind == 2 { 32 } else { 12 };
        for (i, b) in out[..limit].iter_mut().enumerate() {
            // SAFETY: read only initialized prefix, stopping at its terminator.
            *b = unsafe { pointer.add(i).read() };
            if *b == 0 {
                return Ok(());
            }
        }
        if kind == 2 {
            // A >=32-byte indicator cannot equal the <=29-byte suffix. Keep
            // a 31-byte unmatched prefix without examining its unused tail.
            out[31] = 0;
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn store_text(&mut self, kind: u32, text: &[u8]) -> Result<(), Error> {
        // SAFETY: validated <=12-byte initialized terminated prefix; primitive
        // stores it into the admitted ordinal slot without retaining the loan.
        let result =
            unsafe { self.0.store_text.unwrap()(self.0.context, kind, text.as_ptr(), text.len()) };
        if result == 0 {
            Ok(())
        } else {
            Err(Error::Phonemes)
        }
    }
    fn classify(&self, code: u32, kind: u32) -> bool {
        // SAFETY: admitted engine/Unicode/byte classification, kind 0..=3.
        unsafe { self.0.classify.unwrap()(self.0.context, code, kind) != 0 }
    }
    fn translate(&mut self, offset: usize) -> u32 {
        // SAFETY: bounds-checked original source owner; serialized callback.
        unsafe { self.0.translate.unwrap()(self.0.context, offset) }
    }
    fn missing(&mut self, value: i32) {
        // SAFETY: admitted serialized persistent state store, no foreign loan.
        unsafe { self.0.missing.unwrap()(self.0.context, value) }
    }
    fn skip_words(&mut self, value: i32) {
        // SAFETY: admitted serialized persistent skip-word state store.
        unsafe { self.0.skip.unwrap()(self.0.context, value) }
    }
    fn phoneme_type(&self, code: u8) -> Result<i32, Error> {
        // SAFETY: checked unsigned-code primitive returns -1 for absent slots.
        let kind = unsafe { self.0.phoneme_type.unwrap()(self.0.context, code) };
        if kind < 0 {
            Err(Error::Phonemes)
        } else {
            Ok(kind)
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_number(
    table: *const Callbacks,
    length: usize,
    remaining: i32,
    control: i32,
    output: *mut u8,
    capacity: usize,
    flags: *mut u32,
) -> i32 {
    if table.is_null()
        || output.is_null()
        || flags.is_null()
        || length == 0
        || length > 800
        || !(0..=300).contains(&remaining)
        || capacity == 0
        || capacity > PHONEME_BYTES
    {
        return -1;
    }
    // SAFETY: immutable callback table and primitive contracts admitted.
    let Some(mut engine) = (unsafe { Engine::admit(table) }) else {
        return -1;
    };
    let mut scratch = [0; PHONEME_BYTES];
    // SAFETY: two initialized caller flags admitted, disjoint from all loans.
    let mut copied_flags = unsafe { [flags.read(), flags.add(1).read()] };
    let Ok(result) = number_frontend::translate(
        &mut engine,
        remaining as usize,
        control,
        &mut scratch[..capacity],
        &mut copied_flags,
    ) else {
        return -1;
    };
    // SAFETY: publish only initialized validated output prefix and two flags.
    // No foreign unfilled tail is borrowed; rejection preserves both outputs.
    unsafe {
        if let Some(written) = result.written {
            std::ptr::copy_nonoverlapping(scratch.as_ptr(), output, written + 1);
        }
        flags.write(copied_flags[0]);
        flags.add(1).write(copied_flags[1]);
    }
    i32::from(result.recognized)
}
/// `TranslateNumber`'s primitives against the engine (from numbers.c): the
/// word being translated, its translator and word rows, the C options and
/// dictionary lookups, and the ordinal suffixes it keeps between numbers.
#[cfg(espeak_native_host)]
pub(super) mod engine {
    use super::{super::engine_c as c, Callbacks};
    use crate::{translate_clause::Row, translator::Translator};
    use std::ffi::{c_char, c_int, c_void};

    /// One number's translation.
    pub(crate) struct Call {
        pub(crate) tr: *mut Translator,
        pub(crate) word: *mut u8,
        pub(crate) length: usize,
        pub(crate) words: *mut Row,
        pub(crate) remaining: c_int,
    }
    // Kept between calls, as numbers.c's statics were.
    static mut MISSING_THOUSANDS: c_int = 0;
    static mut ORDINAL: [u8; 12] = [0; 12];
    static mut ORDINAL_ALTERNATE: [u8; 12] = [0; 12];

    fn call<'a>(context: *mut c_void) -> &'a Call {
        // SAFETY: every primitive receives the live Call its entry made.
        unsafe { &*context.cast::<Call>() }
    }
    fn tr<'a>(context: *mut c_void) -> &'a mut Translator {
        // SAFETY: the call's translator, live and serialized for the call.
        unsafe { &mut *call(context).tr }
    }
    /// The bytes after `word` that may be read: to the end of the rule text
    /// it lies in (this translator's or the current one's), else to its NUL.
    ///
    /// # Safety
    /// `tr` is null or live; `word` is terminated.
    pub(crate) unsafe fn length(tr: *mut Translator, word: *const u8) -> usize {
        // SAFETY: caller contract; the current translator is null or live.
        let owners = [tr, unsafe { c::translator.cast::<Translator>() }];
        for owner in owners {
            // SAFETY: null or live translators.
            let Some(owner) = (unsafe { owner.as_ref() }) else {
                continue;
            };
            if owner.rule_text_base.is_null() {
                continue;
            }
            let (base, cursor) = (owner.rule_text_base as usize, word as usize);
            if cursor >= base && cursor - base < owner.rule_text_length {
                return owner.rule_text_length - (cursor - base);
            }
        }
        // SAFETY: caller contract.
        unsafe { std::ffi::CStr::from_ptr(word.cast()) }
            .to_bytes()
            .len()
            + 1
    }
    unsafe extern "C" fn byte(context: *mut c_void, index: isize) -> u8 {
        let c = call(context);
        if index >= -3 && (index < 0 || (index as usize) < c.length) {
            // SAFETY: up to three bytes before the word, or within its extent.
            unsafe { c.word.offset(index).read() }
        } else {
            0
        }
    }
    unsafe extern "C" fn write(context: *mut c_void, index: usize, value: u8) {
        let c = call(context);
        if index < c.length {
            // SAFETY: within the word's extent.
            unsafe { c.word.add(index).write(value) };
        }
    }
    unsafe extern "C" fn value(context: *mut c_void, field: u32) -> i32 {
        let tr = tr(context);
        // SAFETY: serialized engine options and the current translator.
        unsafe {
            match field {
                0 => tr.langopts.numbers,
                1 => (*c::translator.cast::<Translator>()).langopts.numbers2,
                2 => tr.langopts.numbers2,
                3 => tr.translator_name,
                4 => MISSING_THOUSANDS,
                5 => c::option_sayas,
                6 => tr.langopts.decimal_sep,
                7 => tr.langopts.thousands_sep,
                8 => tr.prev_dict_flags[0],
                _ => c_int::from(c_char::MIN != 0),
            }
        }
    }
    unsafe extern "C" fn word(context: *mut c_void, index: usize) -> u32 {
        let c = call(context);
        if index == 0 || index < c.remaining.max(0) as usize {
            // SAFETY: the call's initialized word rows.
            unsafe { (*c.words.add(index)).flags }
        } else {
            0
        }
    }
    unsafe extern "C" fn lookup(context: *mut c_void, key: *const u8, output: *mut u8) -> i32 {
        // SAFETY: terminated key and 200-byte output.
        unsafe { c::LookupBounded(call(context).tr.cast(), key.cast(), output.cast(), 200) }
    }
    unsafe extern "C" fn list(
        context: *mut c_void,
        offset: isize,
        output: *mut u8,
        flags: *mut u32,
    ) -> i32 {
        let c = call(context);
        if offset < -3 || (offset >= 0 && offset as usize >= c.length) {
            return 0;
        }
        // SAFETY: a cursor within the word's readable extent; 200-byte output.
        unsafe {
            let mut cursor = c.word.offset(offset).cast::<c_char>();
            c::LookupDictListBounded(
                c.tr.cast(),
                &mut cursor,
                output.cast(),
                flags,
                0x04,
                c.words.cast(),
                c.remaining,
                200,
            )
        }
    }
    unsafe extern "C" fn text(context: *mut c_void, kind: u32) -> *const u8 {
        match kind {
            0 => std::ptr::addr_of!(ORDINAL).cast(),
            1 => std::ptr::addr_of!(ORDINAL_ALTERNATE).cast(),
            _ => tr(context).langopts.ordinal_indicator,
        }
    }
    unsafe extern "C" fn store_text(
        _: *mut c_void,
        kind: u32,
        text: *const u8,
        length: usize,
    ) -> i32 {
        // SAFETY: the front end's initialized text of `length` bytes.
        let text = unsafe { std::slice::from_raw_parts(text, length) };
        if kind > 1 || length == 0 || length > 12 || text[length - 1] != 0 {
            return -1;
        }
        // SAFETY: serialized engine state, not borrowed across calls.
        unsafe {
            let slot = if kind == 0 {
                &mut *std::ptr::addr_of_mut!(ORDINAL)
            } else {
                &mut *std::ptr::addr_of_mut!(ORDINAL_ALTERNATE)
            };
            slot[..length].copy_from_slice(text);
        }
        0
    }
    unsafe extern "C" fn classify(_: *mut c_void, code: u32, kind: u32) -> i32 {
        match kind {
            // SAFETY: pure classifications.
            0 => unsafe { super::super::espeak_rs_common_class(code, 0) },
            1 => super::super::ucd_isdigit(code),
            2 => super::super::ucd_isalpha(code),
            // SAFETY: C's narrow classifier of the low byte, as isspace((unsigned char)c).
            _ => c_int::from(unsafe { libc::isspace(c_int::from(code as u8)) } != 0),
        }
    }
    unsafe extern "C" fn translate(context: *mut c_void, index: usize) -> u32 {
        let c = call(context);
        if index < c.length {
            // SAFETY: a word within the call's buffer.
            unsafe {
                c::TranslateWord(
                    c.tr.cast(),
                    c.word.add(index).cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ) as u32
            }
        } else {
            0
        }
    }
    unsafe extern "C" fn missing(_: *mut c_void, value: i32) {
        // SAFETY: serialized engine state.
        unsafe { MISSING_THOUSANDS = value };
    }
    unsafe extern "C" fn skip(_: *mut c_void, value: i32) {
        // SAFETY: serialized engine state.
        unsafe { c::dictionary_skipwords = value };
    }
    unsafe extern "C" fn phoneme_type(_: *mut c_void, code: u8) -> i32 {
        // SAFETY: the loaded phoneme table's entries are null or live.
        unsafe {
            let table = &*std::ptr::addr_of!(super::super::synthdata_compat::phoneme_tab);
            table.0[usize::from(code)]
                .as_ref()
                .map_or(-1, |phoneme| i32::from(phoneme.kind))
        }
    }
    /// The front end's table over `call`.
    pub(crate) fn table(call: &mut Call) -> Callbacks {
        Callbacks {
            context: (call as *mut Call).cast(),
            byte: Some(byte),
            write: Some(write),
            value: Some(value),
            word: Some(word),
            lookup: Some(lookup),
            list: Some(list),
            text: Some(text),
            store_text: Some(store_text),
            classify: Some(classify),
            translate: Some(translate),
            missing: Some(missing),
            skip: Some(skip),
            phoneme_type: Some(phoneme_type),
        }
    }
    /// `TranslateNumber`.
    ///
    /// # Safety
    /// As the C function: `word` terminated (or within the rule text), output
    /// of `capacity` bytes, two flags, `remaining` word rows; serialized engine.
    #[no_mangle]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "C" fn espeak_rs_translate_number_engine(
        tr: *mut Translator,
        word: *mut u8,
        output: *mut u8,
        capacity: usize,
        flags: *mut u32,
        words: *mut Row,
        remaining: c_int,
        control: c_int,
    ) -> c_int {
        let mut call = Call {
            tr,
            word,
            // SAFETY: caller contract.
            length: unsafe { length(tr, word) },
            words,
            remaining,
        };
        let length = call.length;
        let table = table(&mut call);
        // SAFETY: the table's context outlives the call.
        let result = unsafe {
            super::espeak_rs_translate_number(
                &table, length, remaining, control, output, capacity, flags,
            )
        };
        result.max(0)
    }
}
