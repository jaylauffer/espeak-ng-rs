//! Checked match-result conversion around the serialized raw matcher effect.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{rule_match_host_compat::Record, translate_rules_host_compat::Context};
use crate::translate_rules::Match;
use std::{ffi::c_void, ptr};

pub(super) type RawMatch =
    unsafe extern "C" fn(*mut c_void, usize, usize, u32, u32, *mut *mut u8, *mut Record) -> i32;

/// Metadata and scratch are initialized, aligned and disjoint; source and typed
/// translator owners are retained and serialized. The effect borrows the
/// immutable Context prefix and writable cursor/record/scratch only through
/// return. Every returned nonnull pronunciation has a readable first byte;
/// nonempty foreign pronunciation must be dictionary-owned and NUL-terminated
/// within the bounded extent. No owner reference survives the effect.
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_rules_match_convert(
    context: *const Context,
    group: usize,
    width: usize,
    flags: u32,
    dictionary: u32,
    output: *mut Match,
    effect: Option<RawMatch>,
) -> i32 {
    let Some(effect) = effect else {
        return -1;
    };
    if context.is_null() || output.is_null() || width > 4 {
        return -1;
    }
    // SAFETY: immutable retained metadata and initialized disjoint scratch.
    let ctx = unsafe { &*context };
    if ctx.translator.is_null() {
        return -1;
    }
    // SAFETY: copy scalar fields only; no scratch/translator loan crosses effect.
    let (position, deletion, points, ending) = unsafe {
        (
            (*output).cursor,
            (*output).delete,
            (*output).points,
            (*output).ending,
        )
    };
    if position > isize::MAX as usize {
        return -1;
    }
    let mut cursor = ctx.pointer(position as isize);
    let deleted = if deletion == isize::MIN {
        ptr::null_mut()
    } else {
        ctx.pointer(deletion)
    };
    if cursor.is_null() || (deletion != isize::MIN && deleted.is_null()) {
        return -1;
    }
    // SAFETY: serialized typed owner; this scalar borrow ends before the effect.
    let (_, size) = unsafe { (*ctx.translator).match_dictionary() };
    if group != usize::MAX && group >= size {
        return -1;
    }
    // SAFETY: address of initialized owned scratch, no reference is created.
    let scratch = unsafe { ptr::addr_of_mut!((*output).phonemes).cast::<u8>() };
    let mut record = Record {
        points,
        phonemes: scratch,
        ending,
        delete: deleted,
    };
    // SAFETY: retained immutable prefix, scoped writable cursor/record/scratch;
    // any foreign source/translator effects have exclusive serialized access.
    if unsafe {
        effect(
            context.cast_mut().cast(),
            group,
            width,
            flags,
            dictionary,
            &mut cursor,
            &mut record,
        )
    } != 0
    {
        return -1;
    }
    let Some(position) = (cursor as usize)
        .checked_sub(ctx.source as usize)
        .filter(|&v| v <= isize::MAX as usize && !ctx.pointer(v as isize).is_null())
    else {
        return -1;
    };
    // SAFETY: disjoint output; preserve historical partial publication order.
    unsafe {
        (*output).cursor = position;
        (*output).points = record.points;
        (*output).ending = record.ending;
        (*output).delete = isize::MIN;
    }
    if !record.delete.is_null() {
        let address = record.delete as usize;
        let start = ctx.source as usize;
        let distance = address.abs_diff(start);
        if !address
            .checked_sub(ctx.base as usize)
            .is_some_and(|v| v < ctx.length)
            || distance > isize::MAX as usize
        {
            return -1;
        }
        let offset = if address >= start {
            distance as isize
        } else {
            -(distance as isize)
        };
        if ctx.pointer(offset) != record.delete {
            return -1;
        }
        // SAFETY: checked representable source offset in initialized output.
        unsafe {
            (*output).delete = offset;
        }
    }
    // SAFETY: raw matcher guarantees a readable first byte for nonnull results.
    if record.phonemes.is_null() || unsafe { record.phonemes.read() } == 0 {
        // SAFETY: initialized 200-byte owned scratch.
        unsafe {
            scratch.write(0);
        }
        return 0;
    }
    if record.phonemes == scratch {
        return 0;
    }
    // SAFETY: fresh scalar view after all nested effects; no retained owner loan.
    let (base, size) = unsafe { (*ctx.translator).match_dictionary() };
    let Some(offset) = (record.phonemes as usize)
        .checked_sub(base as usize)
        .filter(|&v| v < size)
    else {
        return -1;
    };
    let available = (size - offset).min(200);
    // SAFETY: dictionary-owned immutable bytes in the checked live extent.
    let Some(length) = (0..available).find(|&i| unsafe { record.phonemes.add(i).read() } == 0)
    else {
        return -1;
    };
    // SAFETY: dictionary and owned scratch are disjoint, checked prefix plus NUL.
    unsafe {
        ptr::copy_nonoverlapping(record.phonemes, scratch, length + 1);
    }
    0
}
