//! Typed translator admission and publication around the bounded rule VM.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{espeak_rs_letter_group, espeak_rs_match_group, PrefixFlags, RawRuleMatch, RuleTrace};
use crate::translator::Translator;
use std::{ffi::c_void, ptr};

#[repr(C)]
#[derive(Clone, Copy)]
struct Input {
    translator: *mut Translator,
    main: *mut Translator,
    word_start: *mut u8,
    rules: *mut u8,
    group_length: i32,
    word_flags: i32,
    dictionary_flags: i32,
    text_length: usize,
    trace: u32,
    signed_bytes: u32,
}
/// Read-only metadata prefix retained by C's prefix lookup and diagnostic trace.
#[repr(C)]
struct Host {
    translator: *mut Translator,
    base: *mut u8,
    group: *mut u8,
    rules: *mut u8,
    group_length: i32,
    word_flags: i32,
    text_length: usize,
    rules_length: usize,
}
#[repr(C)]
struct Effects {
    prefix: Option<PrefixFlags>,
    trace: Option<RuleTrace>,
}
#[repr(C)]
pub(super) struct Record {
    pub(super) points: i32,
    pub(super) phonemes: *const u8,
    pub(super) ending: i32,
    pub(super) delete: *mut u8,
}
impl Host {
    unsafe fn scope(input: &Input, current: *mut u8) -> Option<Self> {
        if input.word_start.is_null()
            || input.text_length == 0
            || input.text_length > isize::MAX as usize
        {
            return None;
        }
        let mut host = Self {
            translator: input.translator,
            base: input.word_start.wrapping_sub(1),
            group: current,
            rules: input.rules,
            group_length: input.group_length,
            word_flags: input.word_flags,
            text_length: input.text_length,
            rules_length: 0,
        };
        for owner in [input.translator, input.main] {
            if owner.is_null() {
                continue;
            }
            // SAFETY: live serialized typed candidate; copy metadata only.
            let (base, length) = unsafe { (*owner).rule_source() };
            if base.is_null() {
                continue;
            }
            if let Some(offset) = (input.word_start as usize).checked_sub(base as usize) {
                if offset > 0 && offset < length && input.text_length - 1 <= length - offset {
                    host.base = base.cast_mut();
                    host.text_length = offset + input.text_length - 1;
                    break;
                }
            }
        }
        if host.text_length > isize::MAX as usize {
            return None;
        }
        let position = (current as usize).checked_sub(host.base as usize)?;
        let start = (input.word_start as usize).checked_sub(host.base as usize)?;
        (position < host.text_length && u32::try_from(start).is_ok()).then_some(host)
    }
}
unsafe extern "C" fn predicate(
    opaque: *mut c_void,
    kind: u32,
    backwards: u32,
    position: usize,
    group: u32,
) -> i32 {
    if kind != 1 {
        return -1;
    }
    // SAFETY: retained immutable host metadata; translator resource views are
    // live and dictionary/language/text stay immutable throughout VM effects.
    let host = unsafe { &*opaque.cast::<Host>() };
    // SAFETY: short typed projection, no translator reference crosses matching.
    let Some((patterns, length)) = (unsafe { (*host.translator).match_patterns(group) }) else {
        return -1;
    };
    // SAFETY: checked resident patterns and admitted source; pure bounded
    // string-group matching performs no foreign callback or mutation.
    unsafe {
        espeak_rs_letter_group(
            patterns,
            length,
            host.base,
            host.text_length,
            position,
            i32::from(backwards != 0),
        )
    }
}
unsafe fn reset(out: *mut Record) {
    // SAFETY: exclusive aligned record, fields may initially be uninitialized;
    // initialize meaningful fields without copying or reading foreign padding.
    unsafe {
        ptr::addr_of_mut!((*out).points).write(0);
        ptr::addr_of_mut!((*out).phonemes).write(c"".as_ptr().cast());
        ptr::addr_of_mut!((*out).ending).write(0);
        ptr::addr_of_mut!((*out).delete).write(ptr::null_mut());
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_match_rule_host(
    input: *const Input,
    word: *mut *mut u8,
    out: *mut Record,
    effects: *const Effects,
) -> i32 {
    if input.is_null() || word.is_null() || out.is_null() || effects.is_null() {
        return -1;
    }
    // SAFETY: initialized retained input/effect metadata and aligned live
    // cursor slot, all disjoint from source/dictionary/result publication.
    let (input, current, effects) = unsafe { (input.read(), word.read(), &*effects) };
    if current.is_null() {
        return -1;
    }
    if input.rules.is_null() {
        // SAFETY: legacy absent-group path changes only points and advances one
        // initialized source byte; other record fields may remain uninitialized.
        unsafe {
            ptr::addr_of_mut!((*out).points).write(0);
            word.write(current.wrapping_add(1));
        }
        return 0;
    }
    if input.translator.is_null() || effects.prefix.is_none() || effects.trace.is_none() {
        return -1;
    }
    // SAFETY: live serialized typed translator; copy dictionary metadata only.
    let (dictionary, length) = unsafe { (*input.translator).match_dictionary() };
    let offset = (input.rules as usize)
        .checked_sub(dictionary as usize)
        .filter(|&v| !dictionary.is_null() && v < length && length <= isize::MAX as usize);
    let Some(offset) = offset else {
        // SAFETY: same legacy checked-invalid-group progress, no callback run.
        unsafe {
            reset(out);
            word.write(current.wrapping_add(1));
        }
        return 0;
    };
    // SAFETY: current/main source owners stay live; initial fallback includes a
    // readable predecessor, NUL and the declared bytes in one allocation.
    let host = unsafe { Host::scope(&input, current) };
    let Some(mut host) = host else {
        return -1;
    };
    host.rules_length = length - offset;
    // SAFETY: admitted exclusive result; subsequent legacy VM failures retain
    // this default and deterministic progress, with executed effects preserved.
    unsafe {
        reset(out);
    }
    let Ok(width) = usize::try_from(input.group_length) else {
        // SAFETY: legacy signed-negative width fails the VM and advances one.
        unsafe {
            word.write(current.wrapping_add(1));
        }
        return 0;
    };
    // SAFETY: short typed scalar/config projections; no whole-translator loan
    // crosses prefix lookup or trace. Language tables remain immutable.
    let (mut context, letters) = unsafe {
        (
            (*input.translator).match_context(
                input.word_flags as u32,
                input.dictionary_flags as u32,
                input.trace,
                input.signed_bytes,
            ),
            (*input.translator).match_letters(),
        )
    };
    context.word_start = ((input.word_start as usize) - (host.base as usize)) as u32;
    let position = (current as usize) - (host.base as usize);
    let mut result = RawRuleMatch {
        phonemes: usize::MAX,
        delete_offset: usize::MAX,
        advance: 0,
        points: 0,
        ending: 0,
    };
    // SAFETY: resident dictionary bytes, source extent and language arrays are
    // immutable and retained through this VM call and its two scoped effects.
    // Effects may change unrelated runtime counters, but must not rebind these
    // resources or retain/mutate the Rust-owned host metadata or input loans.
    let status = unsafe {
        espeak_rs_match_group(
            input.rules,
            length - offset,
            host.base,
            host.text_length,
            position,
            width,
            &context,
            &letters,
            ptr::addr_of_mut!(host).cast(),
            Some(predicate),
            effects.prefix,
            effects.trace,
            &mut result,
        )
    };
    if status != 0 {
        // SAFETY: existing failure progress, matching C without replaying the VM.
        unsafe {
            word.write(current.wrapping_add(width.max(1)));
        }
        return 0;
    }
    // SAFETY: VM returns only checked offsets into immutable admitted spans;
    // publication derives pointers with original source/rule provenance.
    unsafe {
        ptr::addr_of_mut!((*out).points).write(result.points);
        ptr::addr_of_mut!((*out).ending).write(result.ending);
        if result.phonemes != usize::MAX {
            ptr::addr_of_mut!((*out).phonemes).write(input.rules.add(result.phonemes));
        }
        if result.delete_offset != usize::MAX {
            ptr::addr_of_mut!((*out).delete).write(host.base.add(result.delete_offset));
        }
        word.write(current.add(result.advance));
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_group_preserves_other_record_fields_and_advances_once() {
        let mut source = *b" a \0";
        let mut word = source.as_mut_ptr().wrapping_add(1);
        let input = Input {
            translator: ptr::null_mut(),
            main: ptr::null_mut(),
            word_start: word,
            rules: ptr::null_mut(),
            group_length: 4,
            word_flags: -1,
            dictionary_flags: -1,
            text_length: 4,
            trace: 1,
            signed_bytes: 1,
        };
        let mut out = Record {
            points: 19,
            phonemes: source.as_ptr(),
            ending: 13,
            delete: source.as_mut_ptr(),
        };
        let effects = Effects {
            prefix: None,
            trace: None,
        };
        // SAFETY: initialized disjoint retained inputs, writable source/cursor/result.
        let status = unsafe { espeak_rs_match_rule_host(&input, &mut word, &mut out, &effects) };
        assert_eq!(status, 0);
        assert_eq!(word, source.as_mut_ptr().wrapping_add(2));
        assert_eq!((out.points, out.ending), (0, 13));
        assert_eq!(out.phonemes, source.as_ptr());
        assert_eq!(out.delete, source.as_mut_ptr());
    }
    #[test]
    fn source_scope_rejects_outside_cursor_without_fabricating_pointers() {
        let mut source = *b" a \0";
        let input = Input {
            translator: ptr::null_mut(),
            main: ptr::null_mut(),
            word_start: source.as_mut_ptr().wrapping_add(1),
            rules: ptr::null_mut(),
            group_length: 1,
            word_flags: 0,
            dictionary_flags: 0,
            text_length: 4,
            trace: 0,
            signed_bytes: 1,
        };
        // SAFETY: declared fallback's predecessor and four bytes are live.
        let host = unsafe { Host::scope(&input, input.word_start) }.unwrap();
        assert_eq!(host.base, source.as_mut_ptr());
        assert_eq!(host.text_length, 4);
        // SAFETY: only numeric membership is inspected; outside pointer is never read.
        assert!(unsafe { Host::scope(&input, source.as_mut_ptr().wrapping_sub(1)) }.is_none());
        // SAFETY: one-past allocation is only compared, never dereferenced.
        assert!(unsafe { Host::scope(&input, source.as_mut_ptr().wrapping_add(4)) }.is_none());
    }
}
