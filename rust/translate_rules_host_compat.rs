//! Native rule host over typed, serialized translator and source owners.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::rule_conversion_compat::{espeak_rs_rules_match_convert, RawMatch};
use super::translate_rules_compat::{espeak_rs_translate_rules, Callbacks};
use crate::{translate_rules::Match, translator::Translator};
use std::{
    ffi::{c_void, CStr},
    ptr,
};

/// Prefix borrowed read-only by the remaining primitive callbacks. They may
/// mutate the pointed-to owners, but must not retain, mutate or reenter this
/// metadata. No translator/source/output reference survives an effect call.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Context {
    pub(super) translator: *mut Translator,
    pub(super) source: *mut u8,
    pub(super) output: *mut u8,
    pub(super) ending: *mut u8,
    pub(super) base: *mut u8,
    pub(super) length: usize,
    pub(super) match_length: usize,
    pub(super) capacity: usize,
    pub(super) flags: *mut u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Globals {
    say_as: *mut i32,
    phonemes: *mut i32,
    pre_pause: *mut i32,
    signed_bytes: i32,
}
#[repr(C)]
struct Host {
    context: Context,
    globals: Globals,
    matched: Option<RawMatch>,
}
#[repr(C)]
struct Effects {
    locale: Option<unsafe extern "C" fn(*mut c_void, u32, u32) -> i32>,
    matched: Option<RawMatch>,
    symbol: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    letter: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u8) -> i32>,
    append: Option<unsafe extern "C" fn(*mut c_void, *const u8) -> i32>,
    trace: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8)>,
}
impl Context {
    pub(super) fn pointer(&self, position: isize) -> *mut u8 {
        let Some(address) = (self.source as usize).checked_add_signed(position) else {
            return ptr::null_mut();
        };
        if address
            .checked_sub(self.base as usize)
            .is_some_and(|v| v < self.length)
        {
            // Keep the original source allocation's provenance, including its
            // initialized predecessor and any admitted parent source extent.
            self.source.wrapping_offset(position)
        } else {
            ptr::null_mut()
        }
    }
    unsafe fn admit(&mut self, main: *mut Translator) -> bool {
        if self.translator.is_null()
            || self.source.is_null()
            || self.output.is_null()
            || self.capacity == 0
            || self.capacity > 200
        {
            return false;
        }
        // SAFETY: serialized live typed owner, scalar borrow ends here.
        if unsafe { (*self.translator).rule_value(0, 0) } == 0 {
            return false;
        }
        // SAFETY: caller's initialized source has a NUL terminator and a live
        // predecessor in the same allocation, as required by TranslateRules.
        // This replaces the old strlen without allocating or holding a loan.
        let count = unsafe { CStr::from_ptr(self.source.cast()) }
            .to_bytes()
            .len();
        let Some(length) = count.checked_add(2).filter(|&v| v <= isize::MAX as usize) else {
            return false;
        };
        if (self.source as usize).checked_sub(1).is_none() {
            return false;
        }
        self.base = self.source.wrapping_sub(1);
        self.length = length;
        self.match_length = length;
        for owner in [self.translator, main] {
            if owner.is_null() {
                continue;
            }
            // SAFETY: live serialized candidate; copy metadata only, no loan
            // crosses the rule controller or any nested primitive call.
            let (base, extent) = unsafe { (*owner).rule_source() };
            if base.is_null() {
                continue;
            }
            if let Some(offset) = (self.source as usize).checked_sub(base as usize) {
                if offset > 0 && offset < extent && length - 1 <= extent - offset {
                    self.base = base.cast_mut();
                    self.length = extent;
                    break;
                }
            }
        }
        true
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_rules_pointer(context: *const Context, position: isize) -> *mut u8 {
    if context.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: immutable live metadata admitted by the native driver; pure
    // pointer calculation, caller controls the actual read/write duration.
    unsafe { (*context).pointer(position) }
}
unsafe extern "C" fn byte(context: *mut c_void, position: isize) -> i32 {
    // SAFETY: callback context is the retained native Host prefix.
    let pointer = unsafe { espeak_rs_rules_pointer(context.cast(), position) };
    if pointer.is_null() {
        -1
    } else {
        // SAFETY: initialized byte inside the admitted live source allocation.
        i32::from(unsafe { pointer.read() })
    }
}
unsafe extern "C" fn write(context: *mut c_void, position: isize, value: u8) -> i32 {
    // SAFETY: callback context is the retained native Host prefix.
    let pointer = unsafe { espeak_rs_rules_pointer(context.cast(), position) };
    if pointer.is_null() {
        return -1;
    }
    // SAFETY: serialized writable byte, no source reference remains borrowed.
    unsafe {
        pointer.write(value);
    }
    0
}
unsafe extern "C" fn value(context: *mut c_void, field: u32, index: u32) -> i32 {
    // SAFETY: immutable native metadata; all scalar owners remain live. These
    // short reads end before the next nested primitive may mutate the owners.
    unsafe {
        let host = &*context.cast::<Host>();
        match field {
            3 => host.globals.say_as.read(),
            7 => i32::from(host.globals.phonemes.read() & 8 != 0),
            10 => host.globals.pre_pause.read(),
            14 => {
                if host.context.flags.is_null() {
                    0
                } else {
                    host.context.flags.read() as i32
                }
            }
            15 => host.globals.signed_bytes,
            _ => (*host.context.translator).rule_value(field, index),
        }
    }
}
unsafe extern "C" fn store(context: *mut c_void, field: u32, value: i32) {
    // SAFETY: retained immutable metadata, serialized live scalar owners.
    // No translator or flags loan crosses any foreign primitive.
    unsafe {
        let host = &*context.cast::<Host>();
        match field {
            3 => host.globals.pre_pause.write(value),
            4 => {
                if !host.context.flags.is_null() {
                    host.context.flags.write(value as u32);
                }
            }
            _ => (*host.context.translator).rule_store(field, value),
        }
    }
}
unsafe extern "C" fn group(context: *mut c_void, kind: u32, index: u32) -> usize {
    // SAFETY: fresh scalar dictionary/group views in retained serialized owner.
    unsafe { (*(*context.cast::<Host>()).context.translator).rule_group(kind, index) }
}
unsafe extern "C" fn publish(context: *mut c_void, kind: u32, text: *const u8) -> i32 {
    // SAFETY: controller passes initialized owned disjoint 200-byte scratch;
    // destination is live serialized storage of the admitted capacity.
    unsafe {
        let host = &*context.cast::<Host>();
        let (destination, capacity) = if kind == 0 {
            (host.context.output, host.context.capacity)
        } else {
            (host.context.ending, 200)
        };
        if destination.is_null() {
            return -1;
        }
        let Some(length) = (0..200).find(|&i| text.add(i).read() == 0) else {
            return -1;
        };
        if length >= capacity {
            return -1;
        }
        ptr::copy_nonoverlapping(text, destination, length + 1);
    }
    0
}
unsafe extern "C" fn has_ending(context: *mut c_void) -> i32 {
    // SAFETY: retained immutable metadata; no output read needed.
    i32::from(!unsafe { (*context.cast::<Host>()).context.ending }.is_null())
}
unsafe extern "C" fn matched(
    context: *mut c_void,
    group: usize,
    width: usize,
    flags: u32,
    dictionary: u32,
    output: *mut Match,
) -> i32 {
    // SAFETY: retained immutable metadata and checked raw matcher effect; no
    // source/translator/output reference spans the nested primitive call.
    let effect = unsafe { (*context.cast::<Host>()).matched };
    // SAFETY: admitted prefix, initialized disjoint owned match scratch and
    // retained primitive. Converter publishes checked fields without replay.
    unsafe {
        espeak_rs_rules_match_convert(
            context.cast(),
            group,
            width,
            flags,
            dictionary,
            output,
            effect,
        )
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_translate_rules_host(
    context: *const Context,
    main: *mut Translator,
    globals: *const Globals,
    effects: *const Effects,
    flags: u32,
    output: *mut i32,
) -> i32 {
    if context.is_null() || globals.is_null() || effects.is_null() || output.is_null() {
        return -1;
    }
    // SAFETY: caller initializes disjoint input metadata/table, retained live
    // owners and result through return. Copy pointers before nested effects.
    let (mut host, effects) = unsafe {
        (
            Host {
                context: context.read(),
                globals: globals.read(),
                matched: (*effects).matched,
            },
            &*effects,
        )
    };
    if host.globals.say_as.is_null()
        || host.globals.phonemes.is_null()
        || host.globals.pre_pause.is_null()
        || effects.locale.is_none()
        || effects.matched.is_none()
        || effects.symbol.is_none()
        || effects.letter.is_none()
        || effects.append.is_none()
        || effects.trace.is_none()
    {
        return -1;
    }
    // SAFETY: pointer lifetimes and serialized source extent contract above.
    if !unsafe { host.context.admit(main) } {
        return -1;
    }
    let table = Callbacks {
        context: ptr::addr_of_mut!(host).cast(),
        byte: Some(byte),
        write: Some(write),
        value: Some(value),
        store: Some(store),
        locale: effects.locale,
        group: Some(group),
        matched: Some(matched),
        symbol: effects.symbol,
        letter: effects.letter,
        publish: Some(publish),
        has_ending: Some(has_ending),
        append: effects.append,
        trace: effects.trace,
    };
    // SAFETY: owned immutable table and metadata remain live; callbacks retain
    // no loans or metadata pointers. Failure preserves result and prior effects.
    unsafe { espeak_rs_translate_rules(&table, flags, output) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(source: &mut [u8], output: &mut [u8]) -> Context {
        Context {
            translator: ptr::null_mut(),
            source: source.as_mut_ptr().wrapping_add(1),
            output: output.as_mut_ptr(),
            ending: ptr::null_mut(),
            base: source.as_mut_ptr(),
            length: source.len(),
            match_length: source.len(),
            capacity: output.len(),
            flags: ptr::null_mut(),
        }
    }
    #[test]
    fn signed_offsets_preserve_source_extent_and_predecessor() {
        let mut source = *b" abc \0";
        let mut output = [0; 8];
        let c = context(&mut source, &mut output);
        assert_eq!(c.pointer(-1), source.as_mut_ptr());
        assert_eq!(c.pointer(4), source.as_mut_ptr().wrapping_add(5));
        for offset in [-2, 5, isize::MIN, isize::MAX] {
            assert!(c.pointer(offset).is_null());
        }
    }
    #[test]
    fn publications_copy_only_valid_prefix_and_preserve_failure() {
        let mut source = *b" a \0";
        let mut output = [0x97; 8];
        let mut host = Host {
            context: context(&mut source, &mut output),
            globals: Globals {
                say_as: ptr::null_mut(),
                phonemes: ptr::null_mut(),
                pre_pause: ptr::null_mut(),
                signed_bytes: 1,
            },
            matched: None,
        };
        let raw = ptr::addr_of_mut!(host).cast();
        let mut scratch = [b'x'; 200];
        // SAFETY: initialized retained host and disjoint complete scratch/output.
        unsafe {
            assert_eq!(publish(raw, 0, scratch.as_ptr()), -1);
            scratch[8] = 0;
            assert_eq!(publish(raw, 0, scratch.as_ptr()), -1);
            assert_eq!(output, [0x97; 8]);
            scratch[3] = 0;
            assert_eq!(publish(raw, 0, scratch.as_ptr()), 0);
            assert_eq!(output, [b'x', b'x', b'x', 0, 0x97, 0x97, 0x97, 0x97]);
            assert_eq!(publish(raw, 1, scratch.as_ptr()), -1);
        }
    }
}
