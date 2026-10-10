//! Native dictionary lookup over serialized typed translator projections.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{espeak_rs_lookup_bucket, espeak_rs_transpose, RawLookupOutcome};
use crate::{lookup::WordInfo, translator::Translator};
use std::{ffi::CStr, mem::MaybeUninit, ptr};

#[repr(C)]
struct Word {
    flags: u32,
    start: u16,
    pre_pause: u8,
    source: u16,
    length: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Globals {
    main: *mut Translator,
    skip: *mut i32,
    trace: u32,
}
#[repr(C)]
struct Trace {
    kind: u32,
    word: *const u8,
    next: *const u8,
    phonetic: *const u8,
    flags: [u32; 2],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Effects {
    alpha: Option<unsafe extern "C" fn(u32) -> i32>,
    trace: Option<unsafe extern "C" fn(*const Trace)>,
}

/// Exclusive NUL-containing text and serialized live translator; its static
/// alphabet map/pair arrays remain immutable through return. No allocation.
#[no_mangle]
unsafe extern "C" fn espeak_rs_transpose_host(tr: *const Translator, text: *mut u8) -> i32 {
    if tr.is_null() || text.is_null() {
        return -1;
    }
    // SAFETY: retained NUL-containing initialized text; loan ends before mutation.
    let length = unsafe { CStr::from_ptr(text.cast()) }.to_bytes().len();
    if length > i32::MAX as usize {
        return -1;
    }
    // SAFETY: serialized typed owner; copy scalars and pointers only.
    let (min, max, map, pairs) = unsafe { (*tr).lookup_alphabet() };
    if min <= 0 || max < min || max > 0x10ffff {
        return length as i32;
    }
    let mut count = 0;
    if !pairs.is_null() {
        // SAFETY: configured pair table includes a sentinel in at most 128 live
        // initialized entries; malformed missing sentinel preserves the key.
        while count < 128 && unsafe { pairs.add(count).read() } != 0x7fff {
            count += 1;
        }
        if count == 128 {
            return length as i32;
        }
    }
    // SAFETY: exclusive terminated text, immutable disjoint configured arrays.
    let result = unsafe {
        espeak_rs_transpose(
            text,
            length + 1,
            min as u32,
            max as u32,
            map,
            (max - min + 1) as usize,
            pairs,
            count,
        )
    };
    if result < 0 {
        length as i32
    } else {
        result
    }
}

/// Initialized, disjoint flags[2] and >=160-byte pronunciation output; key is
/// terminated. Next retains an initialized predecessor and up to 255 readable
/// bytes through an earlier NUL; a NUL predecessor permits a one-past pointer.
/// Nonnull words have an initialized first record, plus min(remaining,20) live
/// records when remaining>0. Bucket and translator configuration are retained
/// and immutable while matching. Classification is pure; diagnostic callbacks
/// borrow readonly event/source/owned scratch, never retain or reenter them.
/// No foreign source/dictionary/translator reference spans a callback.
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_dict_host(
    tr: *mut Translator,
    original: *const u8,
    next: *const u8,
    phonetic: *mut u8,
    flags: *mut u32,
    end: i32,
    words: *const Word,
    remaining: i32,
    globals: *const Globals,
    effects: *const Effects,
) -> *const u8 {
    if tr.is_null()
        || original.is_null()
        || next.is_null()
        || phonetic.is_null()
        || flags.is_null()
        || globals.is_null()
        || effects.is_null()
    {
        return ptr::null();
    }
    // SAFETY: initialized retained immutable tables; copy before effects.
    let (globals, effects) = unsafe { (globals.read(), effects.read()) };
    let (Some(alpha), Some(trace)) = (effects.alpha, effects.trace) else {
        return ptr::null();
    };
    if globals.skip.is_null() {
        return ptr::null();
    }
    let mut scratch = [0_u8; 161];
    let mut key = original;
    // SAFETY: short scalar borrow of retained typed owner.
    let compressed = unsafe { (*tr).lookup_alphabet().0 } > 0;
    let descriptor = if compressed {
        for (index, byte) in scratch.iter_mut().enumerate().take(159) {
            // SAFETY: original key is initialized through NUL; bounded copy
            // stops there as strncpy0(160), without rescanning a longer key.
            *byte = unsafe { original.add(index).read() };
            if *byte == 0 {
                break;
            }
        }
        key = scratch.as_ptr();
        // SAFETY: initialized exclusive owned scratch and serialized translator.
        (unsafe { espeak_rs_transpose_host(tr, scratch.as_mut_ptr()) }) as usize
    } else {
        // SAFETY: terminated original key; count only, no retained loan.
        unsafe { CStr::from_ptr(original.cast()) }.to_bytes().len()
    };
    // SAFETY: key has its original NUL/hash tail even after binary compression.
    let hash = crate::dictionary::hash(unsafe { CStr::from_ptr(key.cast()) }.to_bytes());
    // SAFETY: fresh scalar dictionary pointers, no owner loan crosses effects.
    let (bucket, rules) = unsafe { (*tr).lookup_bucket(hash) };
    if bucket.is_null() {
        // SAFETY: exclusive initialized output flags; preserve second flag.
        unsafe {
            flags.write(0);
        }
        return ptr::null();
    }
    let Some(length) = (rules as usize)
        .checked_sub(bucket as usize)
        .filter(|&v| !rules.is_null() && v > 0 && v <= isize::MAX as usize)
    else {
        return ptr::null();
    };
    let mut snapshots = [WordInfo {
        flags: 0,
        length: 0,
    }; 20];
    let count = if words.is_null() {
        0
    } else {
        remaining.clamp(0, 20) as usize
    };
    for (index, item) in snapshots.iter_mut().enumerate().take(count) {
        // SAFETY: admitted initialized word fields; never read foreign padding.
        unsafe {
            item.flags = (*words.add(index)).flags;
            item.length = u32::from((*words.add(index)).length);
        }
        if item.length == 0 {
            break;
        }
    }
    // SAFETY: first record is readable even for a nonpositive remaining count.
    let word_flags = if words.is_null() {
        0
    } else {
        // SAFETY: admitted initialized first record, including remaining<=0.
        unsafe { (*words).flags }
    };
    // SAFETY: initialized flags and fresh typed scalar state; copy all values.
    let mut context = unsafe {
        (*tr).lookup_context(
            end as u32,
            word_flags,
            flags.add(1).read() & 0x40000000,
            tr == globals.main,
        )
    };
    let textmode = if globals.main.is_null() {
        0
    } else {
        // SAFETY: fresh serialized main translator scalar metadata.
        let (sentence, extent, mode) = unsafe { (*globals.main).lookup_clause(next) };
        context.sentence = sentence;
        context.clause_remaining = extent;
        mode
    };
    // SAFETY: terminated key; read only bounded decoder head. End this loan
    // before the classifier, then reborrow raw inputs only inside lookup_bucket.
    let (code, width) =
        crate::word_key::decode(unsafe { CStr::from_ptr(key.cast()) }.to_bytes(), 0);
    // SAFETY: decoder width reaches at most the readable NUL; first-byte
    // short circuit also handles an empty/compressed-NUL key without a tail read.
    let single = unsafe { key.read() != 0 && key.add(width).read() == 0 };
    // SAFETY: retained pure classifier; no foreign reference spans this call.
    context.single_symbol = u32::from(single && unsafe { alpha(code) } == 0);
    let mut next_length = 0;
    // SAFETY: initialized predecessor; a NUL permits next to be one-past.
    if unsafe { next.wrapping_sub(1).read() } != 0 {
        // SAFETY: admitted at-most-255-byte next-word window through an earlier NUL.
        while next_length < 255 && unsafe { next.add(next_length).read() } != 0 {
            next_length += 1;
        }
    }
    let mut outcome = MaybeUninit::<RawLookupOutcome>::uninit();
    // SAFETY: immutable live bucket/key/window and copied context/word snapshot;
    // uninitialized outcome is written only on success by the Rust bucket ABI.
    let status = unsafe {
        espeak_rs_lookup_bucket(
            bucket,
            length,
            key.cast(),
            descriptor,
            next.cast(),
            next_length,
            &context,
            if count == 0 {
                ptr::null()
            } else {
                snapshots.as_ptr()
            },
            count,
            outcome.as_mut_ptr(),
        )
    };
    if status != 0 {
        return ptr::null();
    }
    // SAFETY: successful bucket call initialized every meaningful outcome field.
    let outcome = unsafe { outcome.assume_init() };
    if outcome.copied != 0 {
        if outcome.phonemes_length >= 200 {
            return ptr::null();
        }
        // SAFETY: bucket matcher validated this disjoint pronunciation prefix
        // (<160 bytes) and caller retains initialized writable output storage.
        unsafe {
            ptr::copy_nonoverlapping(
                bucket.add(outcome.phonemes_offset),
                phonetic,
                outcome.phonemes_length,
            );
            phonetic.add(outcome.phonemes_length).write(0);
        }
    }
    if outcome.skipwords >= 0 {
        // SAFETY: exclusive live scalar global; preserve historical early effect.
        unsafe {
            globals.skip.write(outcome.skipwords);
        }
    }
    if outcome.has_flags == 0 {
        return ptr::null();
    }
    // SAFETY: exclusive two-flag output, copied state contains no foreign loan.
    unsafe {
        ptr::copy_nonoverlapping(outcome.flags.as_ptr(), flags, 2);
    }
    if globals.trace & 8 != 0
        && (outcome.found == 0 || u32::from(outcome.flags[0] & 0x20000000 != 0) == textmode)
    {
        let mut event = Trace {
            kind: u32::from(outcome.found != 0),
            word: original,
            next: ptr::null(),
            phonetic,
            flags: outcome.trace_flags,
        };
        if outcome.found != 0 && outcome.flags[0] & 0x80 != 0 && !words.is_null() {
            let length = outcome.word_end.min(160);
            // SAFETY: validated next-word prefix; disjoint initialized owned trace
            // scratch. End its copied snippet at the historical final separator.
            unsafe {
                ptr::copy_nonoverlapping(next, scratch.as_mut_ptr(), length);
            }
            scratch[length.saturating_sub(1)] = 0;
            event.next = scratch.as_ptr();
        }
        // SAFETY: readonly initialized event and retained original/output/owned
        // snippet pointers; no foreign dictionary/translator/source reference.
        unsafe {
            trace(&event);
        }
    }
    if outcome.found != 0 {
        next.wrapping_add(outcome.word_end)
    } else {
        ptr::null()
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_word_layout(out: *mut usize) {
    if out.is_null() {
        return;
    }
    let layout = [
        size_of::<Word>(),
        std::mem::offset_of!(Word, flags),
        std::mem::offset_of!(Word, start),
        std::mem::offset_of!(Word, pre_pause),
        std::mem::offset_of!(Word, source),
        std::mem::offset_of!(Word, length),
    ];
    // SAFETY: caller reserves six exclusive aligned usize entries.
    unsafe {
        ptr::copy_nonoverlapping(layout.as_ptr(), out, 6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn alpha(_: u32) -> i32 {
        1
    }
    unsafe extern "C" fn trace(_: *const Trace) {}
    #[test]
    fn missing_effect_preserves_outputs_but_absent_bucket_clears_only_first_flag() {
        let mut owner = Translator::new(b"en").unwrap();
        let tr = ptr::addr_of_mut!(*owner);
        let next = *b" \0";
        let mut phonetic = [0x97; 160];
        let mut flags = [19, 0x40000000];
        let mut skip = 77;
        let globals = Globals {
            main: tr,
            skip: &mut skip,
            trace: 8,
        };
        let mut effects = Effects {
            alpha: None,
            trace: Some(trace),
        };
        // SAFETY: initialized disjoint outputs, retained typed owner/tables and
        // terminated key/next with its predecessor. Missing effect rejects early.
        let result = unsafe {
            espeak_rs_lookup_dict_host(
                tr,
                c"cat".as_ptr().cast(),
                next.as_ptr().wrapping_add(1),
                phonetic.as_mut_ptr(),
                flags.as_mut_ptr(),
                0,
                ptr::null(),
                0,
                &globals,
                &effects,
            )
        };
        assert!(result.is_null());
        assert_eq!(flags, [19, 0x40000000]);
        effects.alpha = Some(alpha);
        // SAFETY: same live inputs; absent bucket has no source or trace effect.
        let result = unsafe {
            espeak_rs_lookup_dict_host(
                tr,
                c"cat".as_ptr().cast(),
                next.as_ptr().wrapping_add(1),
                phonetic.as_mut_ptr(),
                flags.as_mut_ptr(),
                0,
                ptr::null(),
                0,
                &globals,
                &effects,
            )
        };
        assert!(result.is_null());
        assert_eq!(flags, [0, 0x40000000]);
        assert_eq!(phonetic, [0x97; 160]);
        assert_eq!(skip, 77);
    }
}
