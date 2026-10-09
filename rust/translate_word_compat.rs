//! Serialized numeric source/effect ABI for complete native word translation.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::translate_word::{self, Error, Field, Host, Source, Store, PHONEMES, WORD};
use std::ffi::c_void;
#[repr(C)]
struct Callbacks {
    context: *mut c_void,
    byte: Option<unsafe extern "C" fn(*mut c_void, Source) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, Source, u8) -> i32>,
    value: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    store: Option<unsafe extern "C" fn(*mut c_void, u32, i32)>,
    locale: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    list: Option<
        unsafe extern "C" fn(*mut c_void, *mut Source, *mut u8, *mut u32, i32, *mut i32) -> i32,
    >,
    emoji: Option<unsafe extern "C" fn(*mut c_void, *mut Source, *mut u32) -> i32>,
    text: Option<unsafe extern "C" fn(*mut c_void, Source) -> i32>,
    dotted: Option<unsafe extern "C" fn(*mut c_void, Source, *mut i32) -> i32>,
    number_language: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    number:
        Option<unsafe extern "C" fn(*mut c_void, u32, Source, *mut u8, *mut u32, *mut i32) -> i32>,
    spell: Option<unsafe extern "C" fn(*mut c_void, *mut Source, *mut u8, i32, *mut i32) -> i32>,
    letter: Option<unsafe extern "C" fn(*mut c_void, Source, *mut u8, u32, *mut usize) -> i32>,
    unpronounceable: Option<unsafe extern "C" fn(*mut c_void, Source, i32, *mut i32) -> i32>,
    spelling_stress: Option<unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32>,
    rules: Option<
        unsafe extern "C" fn(*mut c_void, Source, *mut u8, *mut u8, u32, *mut u32, *mut i32) -> i32,
    >,
    remove: Option<unsafe extern "C" fn(*mut c_void, Source, i32, *mut u8, *mut i32) -> i32>,
    prefix: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut Source) -> i32>,
    trace_suffix: Option<unsafe extern "C" fn(*mut c_void, *const u8)>,
    append: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *const u8) -> i32>,
    plural: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    stress: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u32, i32, i32) -> i32>,
    snapshot: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    publish: Option<unsafe extern "C" fn(*mut c_void, *const u8, u32) -> i32>,
    change_stress: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    special: Option<unsafe extern "C" fn(*mut c_void, u32) -> i32>,
}
struct Engine<'a>(&'a Callbacks);
fn status(value: i32) -> Result<(), Error> {
    if value == 0 {
        Ok(())
    } else {
        Err(Error::State)
    }
}
impl Host for Engine<'_> {
    fn byte(&self, source: Source) -> Option<u8> {
        // SAFETY: checked numeric source projection, no borrowed foreign span.
        u8::try_from(unsafe { self.0.byte.unwrap()(self.0.context, source) }).ok()
    }
    fn write(&mut self, source: Source, byte: u8) -> Result<(), Error> {
        // SAFETY: checked exclusive source-byte store in the serialized owner.
        status(unsafe { self.0.write.unwrap()(self.0.context, source, byte) })
    }
    fn value(&self, field: Field, index: u32) -> i32 {
        // SAFETY: fresh checked scalar/row projection, retained live owner.
        unsafe { self.0.value.unwrap()(self.0.context, field as u32, index) }
    }
    fn store(&mut self, field: Store, value: i32) {
        // SAFETY: serialized scalar store, no foreign loan across effects.
        unsafe { self.0.store.unwrap()(self.0.context, field as u32, value) }
    }
    fn locale(&self, code: u32, digit: bool) -> bool {
        // SAFETY: raw host CRT classification of a scalar character.
        unsafe { self.0.locale.unwrap()(self.0.context, code, u32::from(digit)) != 0 }
    }
    fn list(
        &mut self,
        source: &mut Source,
        phonemes: &mut [u8; PHONEMES],
        flags: &mut [u32; 2],
        ending: i32,
    ) -> Result<bool, Error> {
        let mut found = 0;
        // SAFETY: disjoint owned initialized source descriptor, pronunciation,
        // flags and result. Callback admits returned source into its live slots.
        status(unsafe {
            self.0.list.unwrap()(
                self.0.context,
                source,
                phonemes.as_mut_ptr(),
                flags.as_mut_ptr(),
                ending,
                &mut found,
            )
        })?;
        Ok(found != 0)
    }
    fn emoji(&mut self, source: &mut Source, flags: &mut [u32; 2]) -> Result<(), Error> {
        // SAFETY: owned descriptor/flags; owner retains any replacement storage.
        status(unsafe { self.0.emoji.unwrap()(self.0.context, source, flags.as_mut_ptr()) })
    }
    fn text(&mut self, source: Source) -> Result<(), Error> {
        // SAFETY: owner copies bounded replacement to the admitted text output.
        status(unsafe { self.0.text.unwrap()(self.0.context, source) })
    }
    fn dotted(&mut self, source: Source) -> Result<i32, Error> {
        let mut found = 0;
        // SAFETY: admitted numeric source and disjoint initialized result.
        status(unsafe { self.0.dotted.unwrap()(self.0.context, source, &mut found) })?;
        Ok(found)
    }
    fn number_language(&mut self) -> Result<(), Error> {
        // SAFETY: named probe writes fresh owner output, retaining no loan.
        status(unsafe { self.0.number_language.unwrap()(self.0.context) })
    }
    fn number(
        &mut self,
        roman: bool,
        source: Source,
        phonemes: &mut [u8; PHONEMES],
        flags: &mut [u32; 2],
    ) -> Result<bool, Error> {
        let mut found = 0;
        // SAFETY: numeric source, disjoint owned pronunciation/flags/result.
        status(unsafe {
            self.0.number.unwrap()(
                self.0.context,
                u32::from(roman),
                source,
                phonemes.as_mut_ptr(),
                flags.as_mut_ptr(),
                &mut found,
            )
        })?;
        Ok(found != 0)
    }
    fn spell(
        &mut self,
        source: &mut Source,
        phonemes: &mut [u8; PHONEMES],
        mode: i32,
    ) -> Result<bool, Error> {
        let mut switched = 0;
        // SAFETY: owned initialized descriptor/phonemes/result; owner performs
        // its shared-output effects directly and admits any returned cursor.
        status(unsafe {
            self.0.spell.unwrap()(
                self.0.context,
                source,
                phonemes.as_mut_ptr(),
                mode,
                &mut switched,
            )
        })?;
        Ok(switched != 0)
    }
    fn letter(
        &mut self,
        source: Source,
        phonemes: &mut [u8; PHONEMES],
        non_initial: bool,
    ) -> Result<usize, Error> {
        let mut consumed = 0;
        // SAFETY: numeric source, owned initialized pronunciation and result.
        status(unsafe {
            self.0.letter.unwrap()(
                self.0.context,
                source,
                phonemes.as_mut_ptr(),
                u32::from(non_initial),
                &mut consumed,
            )
        })?;
        Ok(consumed)
    }
    fn unpronounceable(&mut self, source: Source, position: i32) -> Result<bool, Error> {
        let mut value = 0;
        // SAFETY: scalar source/position, disjoint initialized result.
        status(unsafe {
            self.0.unpronounceable.unwrap()(self.0.context, source, position, &mut value)
        })?;
        Ok(value != 0)
    }
    fn spelling_stress(
        &mut self,
        phonemes: &mut [u8; PHONEMES],
        position: i32,
    ) -> Result<(), Error> {
        // SAFETY: bounded initialized owned pronunciation; no retained pointer.
        status(unsafe {
            self.0.spelling_stress.unwrap()(self.0.context, phonemes.as_mut_ptr(), position)
        })
    }
    fn rules(
        &mut self,
        source: Source,
        phonemes: &mut [u8; PHONEMES],
        ending: Option<&mut [u8; PHONEMES]>,
        word_flags: u32,
        flags: &mut [u32; 2],
    ) -> Result<i32, Error> {
        let mut result = 0;
        let ending = ending.map_or(std::ptr::null_mut(), |p| p.as_mut_ptr());
        // SAFETY: numeric source, disjoint owned initialized pronunciation,
        // optional ending, flags and result; primitive retains no loan.
        status(unsafe {
            self.0.rules.unwrap()(
                self.0.context,
                source,
                phonemes.as_mut_ptr(),
                ending,
                word_flags,
                flags.as_mut_ptr(),
                &mut result,
            )
        })?;
        Ok(result)
    }
    fn remove(
        &mut self,
        source: Source,
        ending: i32,
        copy: Option<&mut [u8; WORD]>,
    ) -> Result<i32, Error> {
        let mut result = 0;
        let copy = copy.map_or(std::ptr::null_mut(), |p| p.as_mut_ptr());
        // SAFETY: numeric source, optional initialized owned 160-byte copy and
        // disjoint result; any source effects remain with the serialized owner.
        status(unsafe {
            self.0.remove.unwrap()(self.0.context, source, ending, copy, &mut result)
        })?;
        Ok(result)
    }
    fn prefix(&mut self, source: &[u8; 65]) -> Result<Source, Error> {
        let mut result = Source::default();
        // SAFETY: owned initialized prefix copied into retained owner storage;
        // no pointer to the Rust scratch survives this synchronous callback.
        status(unsafe { self.0.prefix.unwrap()(self.0.context, source.as_ptr(), &mut result) })?;
        Ok(result)
    }
    fn trace_suffix(&mut self, phonemes: &[u8; PHONEMES]) {
        // SAFETY: bounded owned pronunciation borrowed only through formatting.
        unsafe { self.0.trace_suffix.unwrap()(self.0.context, phonemes.as_ptr()) }
    }
    fn append(
        &mut self,
        phonemes: &mut [u8; PHONEMES],
        ending: &[u8; PHONEMES],
    ) -> Result<(), Error> {
        // SAFETY: disjoint initialized owned pronunciation and ending; native
        // leaf accesses fresh table/counters and retains no callback loan.
        status(unsafe {
            self.0.append.unwrap()(self.0.context, phonemes.as_mut_ptr(), ending.as_ptr())
        })
    }
    fn plural(&mut self, word_flags: u32, last: u32) -> Result<(), Error> {
        // SAFETY: scalar plural inputs, native primitive writes fresh word owner.
        status(unsafe { self.0.plural.unwrap()(self.0.context, word_flags, last) })
    }
    fn stress(
        &mut self,
        phonemes: Option<&mut [u8; PHONEMES]>,
        flags: &mut [u32; 2],
        position: i32,
        control: i32,
    ) -> Result<(), Error> {
        let phonemes = phonemes.map_or(std::ptr::null_mut(), |p| p.as_mut_ptr());
        // SAFETY: optional owned pronunciation (null selects fresh word output),
        // disjoint owned flags and scalar controls; no foreign loan retained.
        status(unsafe {
            self.0.stress.unwrap()(
                self.0.context,
                phonemes,
                flags.as_mut_ptr(),
                position,
                control,
            )
        })
    }
    fn snapshot(&self, phonemes: &mut [u8; PHONEMES]) -> Result<(), Error> {
        // SAFETY: owner copies only its initialized terminated prefix into owned
        // scratch, avoiding uninitialized tails and any persistent foreign loan.
        status(unsafe { self.0.snapshot.unwrap()(self.0.context, phonemes.as_mut_ptr()) })
    }
    fn publish(&mut self, phonemes: &[u8; PHONEMES], joined: bool) -> Result<(), Error> {
        // SAFETY: initialized owned terminated prefix; admitted 200-byte owner.
        status(unsafe {
            self.0.publish.unwrap()(self.0.context, phonemes.as_ptr(), u32::from(joined))
        })
    }
    fn change_stress(&mut self, level: i32) -> Result<(), Error> {
        // SAFETY: native leaf over fresh owner output and scalar stress level.
        status(unsafe { self.0.change_stress.unwrap()(self.0.context, level) })
    }
    fn special(&mut self, flags: u32) -> Result<(), Error> {
        // SAFETY: native leaf over fresh output/table and scalar dictionary flags.
        status(unsafe { self.0.special.unwrap()(self.0.context, flags) })
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_word(table: *const Callbacks, result: *mut u32) -> i32 {
    if table.is_null() || result.is_null() {
        return -1;
    }
    // SAFETY: immutable table and serialized live owners retained through call;
    // result is disjoint from every source, table, context and output owner.
    let table = unsafe { &*table };
    if table.context.is_null()
        || table.byte.is_none()
        || table.write.is_none()
        || table.value.is_none()
        || table.store.is_none()
        || table.locale.is_none()
        || table.list.is_none()
        || table.emoji.is_none()
        || table.text.is_none()
        || table.dotted.is_none()
        || table.number_language.is_none()
        || table.number.is_none()
        || table.spell.is_none()
        || table.letter.is_none()
        || table.unpronounceable.is_none()
        || table.spelling_stress.is_none()
        || table.rules.is_none()
        || table.remove.is_none()
        || table.prefix.is_none()
        || table.trace_suffix.is_none()
        || table.append.is_none()
        || table.plural.is_none()
        || table.stress.is_none()
        || table.snapshot.is_none()
        || table.publish.is_none()
        || table.change_stress.is_none()
        || table.special.is_none()
    {
        return -1;
    }
    let Ok(value) = translate_word::translate(&mut Engine(table)) else {
        return -1;
    };
    // SAFETY: publish initialized flags only after successful native control.
    unsafe {
        result.write(value);
    }
    0
}
