//! Native list/symbol host over serialized typed state and primitive effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    lookup_list::{self, Error, WORD_BYTES},
    lookup_symbol,
    number_lookup::PHONEME_BYTES,
    translator::Translator,
};
use std::{
    ffi::{c_void, CStr},
    ptr,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct Context {
    tr: *mut Translator,
    source: *const u8,
    length: usize,
    wordptr: *mut *mut u8,
    end_flags: i32,
    words: *mut c_void,
    remaining: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Globals {
    skip: *mut i32,
    say_as: *mut i32,
    trace: *mut i32,
    phonemes: *const u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Effects {
    lookup: Option<
        unsafe extern "C" fn(*mut c_void, *const u8, *const u8, *mut u8, *mut u32) -> *const u8,
    >,
    accent: Option<unsafe extern "C" fn(*mut c_void, u32, usize, *mut u8)>,
    translate: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    trace: Option<unsafe extern "C" fn(*mut c_void, *const u8, *const u8)>,
}
struct Host {
    context: Context,
    globals: Globals,
    effects: Effects,
}
unsafe fn live_context(context: *const Context) -> bool {
    if context.is_null() {
        return false;
    }
    // SAFETY: caller supplies initialized context metadata; short copy only.
    let context = unsafe { context.read() };
    if context.tr.is_null() || context.source.is_null() || context.wordptr.is_null() {
        return false;
    }
    // SAFETY: initialized retained word slot after its null guard.
    !unsafe { context.wordptr.read() }.is_null()
}
impl Host {
    /// Caller retains initialized metadata, serialized typed owner, terminated
    /// source or its registered extent and readonly primitive callback tables.
    unsafe fn admit(
        context: *const Context,
        globals: *const Globals,
        effects: *const Effects,
        scoped: bool,
    ) -> Option<Self> {
        if context.is_null() || globals.is_null() || effects.is_null() {
            return None;
        }
        // SAFETY: initialized retained metadata; copy before any effect.
        let (mut context, globals, effects) =
            unsafe { (context.read(), globals.read(), effects.read()) };
        if context.tr.is_null()
            || context.source.is_null()
            || context.wordptr.is_null()
            || globals.skip.is_null()
            || globals.say_as.is_null()
            || globals.trace.is_null()
            || globals.phonemes.is_null()
            || effects.lookup.is_none()
            || effects.accent.is_none()
            || effects.translate.is_none()
            || effects.trace.is_none()
        {
            return None;
        }
        // SAFETY: initialized live cursor and fresh typed scalar metadata.
        if unsafe { context.wordptr.read() }.is_null() {
            return None;
        }
        // SAFETY: live serialized typed owner, copy source metadata only.
        let (base, extent) = unsafe { (*context.tr).rule_source() };
        let offset = (context.source as usize).checked_sub(base as usize);
        context.length = if scoped && !base.is_null() && offset.is_some_and(|v| v < extent) {
            extent - offset?
        } else {
            // SAFETY: outside the registered source, native callers retain NUL.
            unsafe { CStr::from_ptr(context.source.cast()) }
                .to_bytes()
                .len()
                .checked_add(1)?
        };
        if context.length > isize::MAX as usize {
            return None;
        }
        Some(Self {
            context,
            globals,
            effects,
        })
    }
    fn opaque(&mut self) -> *mut c_void {
        ptr::addr_of_mut!(self.context).cast()
    }
}
impl lookup_list::Host for Host {
    fn byte(&self, position: usize) -> Option<u8> {
        if position >= self.context.length {
            return None;
        }
        // SAFETY: admitted initialized original source; no loan retained.
        Some(unsafe { self.context.source.add(position).read() })
    }
    fn lookup(
        &mut self,
        key: &[u8; WORD_BYTES],
        next: usize,
        flags: &mut [u32; 2],
        phonemes: &mut [u8; PHONEME_BYTES],
    ) -> Result<Option<usize>, Error> {
        if next > self.context.length {
            return Err(Error::Source);
        }
        let callback = self.effects.lookup.expect("admitted lookup");
        // SAFETY: source pointer may be one-past; child obeys lookup predecessor
        // contract. Only owned initialized key/flags/scratch loans span effect.
        let found = unsafe {
            callback(
                self.opaque(),
                key.as_ptr(),
                self.context.source.add(next),
                phonemes.as_mut_ptr(),
                flags.as_mut_ptr(),
            )
        };
        if found.is_null() {
            return Ok(None);
        }
        (found as usize)
            .checked_sub(self.context.source as usize)
            .filter(|&v| v <= self.context.length)
            .map(Some)
            .ok_or(Error::Source)
    }
    fn repeat(&self, output: &mut [u8; 20]) -> i32 {
        // SAFETY: short serialized typed state loan ends before effects.
        let (text, count) = unsafe { (*self.context.tr).list_repeat() };
        *output = text;
        count
    }
    fn set_repeat(&mut self, output: &[u8; 20], count: i32) {
        // SAFETY: exclusive serialized typed state for this short projection.
        unsafe {
            (*self.context.tr).list_set_repeat(output, count);
        }
    }
    fn text_mode(&self) -> bool {
        // SAFETY: fresh serialized scalar after lookup effects.
        unsafe { (*self.context.tr).list_mode() }
    }
    fn skip_words(&mut self, count: i32) {
        // SAFETY: initialized exclusive serialized global slot.
        unsafe {
            self.globals.skip.write(count);
        }
    }
    fn accent(&mut self, code: u32, capacity: usize, phonemes: &mut [u8; PHONEME_BYTES]) {
        let callback = self.effects.accent.expect("admitted accent");
        // SAFETY: retained effect borrows only owned bounded initialized scratch.
        unsafe {
            callback(self.opaque(), code, capacity, phonemes.as_mut_ptr());
        }
    }
    fn replacement(&mut self, text: &[u8; WORD_BYTES]) {
        // SAFETY: short exclusive typed copy; returned retained pointer is
        // published after the owner reference ends, never across an effect.
        let replacement = unsafe { (*self.context.tr).list_replace(text) };
        // SAFETY: retained exclusive serialized caller cursor slot.
        unsafe {
            self.context.wordptr.write(replacement);
        }
    }
    fn trace_replacement(&mut self, matched: usize) {
        // SAFETY: fresh initialized global scalar, no retained reference.
        if unsafe { self.globals.trace.read() } & 8 == 0
            || matched >= WORD_BYTES
            || matched > self.context.length
        {
            return;
        }
        let mut word = [0; WORD_BYTES];
        // SAFETY: admitted original prefix copied to owned terminated scratch.
        unsafe {
            ptr::copy_nonoverlapping(self.context.source, word.as_mut_ptr(), matched);
        }
        let callback = self.effects.trace.expect("admitted trace");
        // SAFETY: fresh current cursor is retained terminated replacement; no
        // owner/source reference survives this readonly formatter effect.
        let replacement = unsafe { self.context.wordptr.read() };
        // SAFETY: readonly terminated replacement and owned original prefix.
        unsafe {
            callback(self.opaque(), word.as_ptr(), replacement);
        }
    }
}
pub(super) struct SourceGuard {
    tr: *mut Translator,
    base: *const u8,
    length: usize,
}
impl SourceGuard {
    /// Serialized live typed owner and initialized source retained through the
    /// guard; effects retain neither source nor owner loans and never free owner.
    pub(super) unsafe fn enter(tr: *mut Translator, source: *const u8, extent: usize) -> Self {
        // SAFETY: short typed source metadata projection on the retained owner.
        let (base, length) = unsafe { (*tr).rule_source() };
        // SAFETY: exclusive serialized source publication before child effects.
        unsafe {
            (*tr).rule_set_source(source, extent);
        }
        Self { tr, base, length }
    }
}
impl Drop for SourceGuard {
    fn drop(&mut self) {
        // SAFETY: serialized owner outlives the child and guard; nested source
        // metadata is restored even if a later Rust operation unwinds.
        unsafe {
            (*self.tr).rule_set_source(self.base, self.length);
        }
    }
}
impl lookup_symbol::Host for Host {
    type List = Self;
    fn list(&mut self) -> &mut Self {
        self
    }
    fn byte(&self, position: usize) -> Option<u8> {
        // SAFETY: fresh serialized slot and owner buffer address, no loans.
        let word = unsafe { self.context.wordptr.read() };
        // SAFETY: short serialized typed buffer-address projection.
        let replacement = unsafe { (*self.context.tr).list_source() };
        let bound = if word.cast_const() == self.context.source {
            self.context.length
        } else if word.cast_const() == replacement {
            WORD_BYTES - 2
        } else {
            return None;
        };
        if position >= bound {
            return None;
        }
        // SAFETY: admitted original or retained initialized replacement extent.
        Some(unsafe { word.add(position).read() })
    }
    fn say_as(&self) -> i32 {
        // SAFETY: initialized serialized global slot read fresh after lookup.
        unsafe { self.globals.say_as.read() }
    }
    fn set_say_as(&mut self, value: i32) {
        // SAFETY: initialized exclusive serialized global slot.
        unsafe {
            self.globals.say_as.write(value);
        }
    }
    fn translate(
        &mut self,
        text: &mut [u8; lookup_symbol::SOURCE_BYTES],
        phonemes: &mut [u8; PHONEME_BYTES],
    ) -> i32 {
        let base = text.as_mut_ptr();
        // SAFETY: short typed metadata projection; no owner loan spans child.
        let guard = unsafe { SourceGuard::enter(self.context.tr, base.cast_const(), text.len()) };
        let callback = self.effects.translate.expect("admitted translation");
        // SAFETY: child borrows initialized owned padded source through return,
        // may mutate serialized state/global pronunciation, retains no scratch.
        let result = unsafe { callback(self.opaque(), base.add(3)) };
        for (index, byte) in phonemes.iter_mut().enumerate() {
            // SAFETY: fresh global initialized pronunciation after child return;
            // copy only through NUL, leaving owned initialized tail untouched.
            *byte = unsafe { self.globals.phonemes.add(index).read() };
            if *byte == 0 {
                break;
            }
        }
        drop(guard);
        result
    }
}

/// Retained serialized context/effects per rust_lookup_symbol.h. Caller output
/// can be the global child pronunciation: never lend it through an effect.
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_list_host(
    context: *const Context,
    globals: *const Globals,
    effects: *const Effects,
    flags: *mut u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if output.is_null() || flags.is_null() || capacity == 0 {
        return 0;
    }
    // SAFETY: retained initialized metadata/slot; null native inputs preserve output.
    if !unsafe { live_context(context) } || globals.is_null() || effects.is_null() {
        return 0;
    }
    // SAFETY: metadata/owner/source/effects obey the native header contract.
    let host = unsafe { Host::admit(context, globals, effects, true) };
    let Some(mut host) = host.filter(|_| capacity <= PHONEME_BYTES) else {
        // SAFETY: admitted initialized caller output includes its first byte.
        unsafe {
            output.write(0);
        }
        return 0;
    };
    let mut scratch = [0; PHONEME_BYTES];
    // SAFETY: initialized two-flag array, copied before any callback.
    let mut selected = unsafe { [flags.read(), flags.add(1).read()] };
    let end_flags = host.context.end_flags as u32;
    match lookup_list::dictionary_list(
        &mut host,
        end_flags,
        &mut selected,
        &mut scratch[..capacity],
    ) {
        Ok(found) => {
            let length = scratch
                .iter()
                .position(|&b| b == 0)
                .expect("validated output");
            // SAFETY: exclusive actual caller extent, no effects remain; copy
            // only initialized validated prefix, preserving output tail.
            unsafe {
                ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1);
                flags.write(selected[0]);
                flags.add(1).write(selected[1]);
            }
            i32::from(found)
        }
        Err(_) => {
            // SAFETY: admitted caller output retains a writable first byte.
            unsafe {
                output.write(0);
            }
            0
        }
    }
}
/// Same native contract, symbol context and fresh nested translation globals.
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_symbol_host(
    context: *const Context,
    globals: *const Globals,
    effects: *const Effects,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if output.is_null() || capacity == 0 {
        return 0;
    }
    // SAFETY: retained initialized metadata/slot; null native inputs preserve output.
    if !unsafe { live_context(context) } || globals.is_null() || effects.is_null() {
        return 0;
    }
    // SAFETY: metadata/owner/source/effects obey the native header contract.
    let host = unsafe { Host::admit(context, globals, effects, true) };
    let Some(mut host) = host.filter(|_| capacity <= PHONEME_BYTES) else {
        // SAFETY: admitted caller output retains a writable first byte.
        unsafe {
            output.write(0);
        }
        return 0;
    };
    let mut scratch = [0; PHONEME_BYTES];
    match lookup_symbol::symbol(&mut host, &mut scratch[..capacity]) {
        Ok(result) => {
            let length = scratch
                .iter()
                .position(|&b| b == 0)
                .expect("validated output");
            // SAFETY: actual exclusive output extent after every child returns.
            unsafe {
                ptr::copy_nonoverlapping(scratch.as_ptr(), output, length + 1);
            }
            result
        }
        Err(_) => {
            // SAFETY: admitted caller output retains a writable first byte.
            unsafe {
                output.write(0);
            }
            0
        }
    }
}
/// Prefix queries use strlen admission, ordinary context and no word rows.
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_flags_host(
    context: *const Context,
    globals: *const Globals,
    effects: *const Effects,
    flags: *mut u32,
) -> i32 {
    if flags.is_null() {
        return 0;
    }
    // SAFETY: metadata/owner/source/effects obey native contract; no rule scope.
    let selected = unsafe { Host::admit(context, globals, effects, false) }
        .and_then(|mut host| lookup_symbol::flags(&mut host).ok())
        .unwrap_or([0; 2]);
    // SAFETY: exclusive initialized two-flag output; all effects have returned.
    unsafe {
        flags.write(selected[0]);
        flags.add(1).write(selected[1]);
    }
    selected[0] as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn lookup(
        _: *mut c_void,
        _: *const u8,
        next: *const u8,
        out: *mut u8,
        flags: *mut u32,
    ) -> *const u8 {
        // SAFETY: initialized owned scratch and two flags supplied by Host.
        unsafe {
            out.write(b'a');
            out.add(1).write(0);
            flags.write(1);
            flags.add(1).write(2);
        }
        next
    }
    unsafe extern "C" fn accent(_: *mut c_void, _: u32, _: usize, _: *mut u8) {}
    unsafe extern "C" fn trace(_: *mut c_void, _: *const u8, _: *const u8) {}
    unsafe extern "C" fn translate(ctx: *mut c_void, text: *mut u8) -> i32 {
        // SAFETY: readonly Host prefix and serialized owner, child loan live.
        let ctx = unsafe { &*ctx.cast::<Context>() };
        // SAFETY: fresh scalar projection; no owner reference retained.
        let (base, length) = unsafe { (*ctx.tr).rule_source() };
        assert_eq!(base, text.wrapping_sub(3));
        assert_eq!(length, lookup_symbol::SOURCE_BYTES);
        // SAFETY: nested serialized source mutation, restored by outer guard.
        unsafe {
            (*ctx.tr).rule_set_source(ptr::null(), 93);
        }
        i32::MIN
    }
    fn fixture(
        tr: *mut Translator,
        source: &[u8],
        cursor: &mut *mut u8,
        slots: &mut [i32; 3],
        phonemes: &[u8; PHONEME_BYTES],
    ) -> (Context, Globals, Effects) {
        *cursor = source.as_ptr().cast_mut();
        (
            Context {
                tr,
                source: source.as_ptr(),
                length: 0,
                wordptr: cursor,
                end_flags: 0,
                words: ptr::null_mut(),
                remaining: 0,
            },
            Globals {
                skip: slots.as_mut_ptr(),
                say_as: slots.as_mut_ptr().wrapping_add(1),
                trace: slots.as_mut_ptr().wrapping_add(2),
                phonemes: phonemes.as_ptr(),
            },
            Effects {
                lookup: Some(lookup),
                accent: Some(accent),
                translate: Some(translate),
                trace: Some(trace),
            },
        )
    }
    #[test]
    fn registered_source_bounds_list_but_prefix_retains_terminated_admission() {
        let mut owner = Translator::new(b"en").unwrap();
        let source = *b"cat \0";
        owner.rule_set_source(source.as_ptr(), 2);
        let mut cursor = ptr::null_mut();
        let mut slots = [77, 9, 0];
        let phonemes = [0; PHONEME_BYTES];
        let (ctx, globals, effects) = fixture(
            ptr::addr_of_mut!(*owner),
            &source,
            &mut cursor,
            &mut slots,
            &phonemes,
        );
        let mut flags = [7, 8];
        let mut out = [0x97; PHONEME_BYTES];
        // SAFETY: initialized disjoint source, owner, slots, metadata and outputs.
        let result = unsafe {
            espeak_rs_lookup_list_host(
                &ctx,
                &globals,
                &effects,
                flags.as_mut_ptr(),
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(result, 0);
        assert_eq!(flags, [7, 8]);
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 0x97);
        // SAFETY: same retained inputs; prefix ignores registered rule extent.
        let result =
            unsafe { espeak_rs_lookup_flags_host(&ctx, &globals, &effects, flags.as_mut_ptr()) };
        assert_eq!(result, 1);
        assert_eq!(flags, [1, 2]);
    }
    #[test]
    fn child_source_restores_and_pronunciation_copies_only_initialized_prefix() {
        let mut owner = Translator::new(b"en").unwrap();
        let source = *b"cat \0";
        owner.rule_set_source(source.as_ptr(), source.len());
        let mut cursor = ptr::null_mut();
        let mut slots = [77, 9, 0];
        let mut phonemes = [0x91; PHONEME_BYTES];
        phonemes[..3].copy_from_slice(b"xy\0");
        let (ctx, globals, effects) = fixture(
            ptr::addr_of_mut!(*owner),
            &source,
            &mut cursor,
            &mut slots,
            &phonemes,
        );
        // SAFETY: initialized retained metadata and callback contract.
        let mut host = unsafe { Host::admit(&ctx, &globals, &effects, true) }.unwrap();
        let mut text = [0; lookup_symbol::SOURCE_BYTES];
        text[1] = b' ';
        text[2] = b' ';
        let mut output = [0x97; PHONEME_BYTES];
        assert_eq!(
            lookup_symbol::Host::translate(&mut host, &mut text, &mut output),
            i32::MIN
        );
        assert_eq!(&output[..4], b"xy\0\x97");
        assert_eq!(owner.rule_source(), (source.as_ptr(), source.len()));
    }
    #[test]
    fn typed_repeat_and_replacement_keep_full_storage_and_cursor_offset() {
        let mut owner = Translator::new(b"en").unwrap();
        let source = *b"cat \0";
        let mut cursor = ptr::null_mut();
        let mut slots = [77, 9, 0];
        let phonemes = [0; PHONEME_BYTES];
        let (ctx, globals, effects) = fixture(
            ptr::addr_of_mut!(*owner),
            &source,
            &mut cursor,
            &mut slots,
            &phonemes,
        );
        // SAFETY: initialized serialized typed owner and retained source/tables.
        let mut host = unsafe { Host::admit(&ctx, &globals, &effects, true) }.unwrap();
        let repeat = [0x93; 20];
        lookup_list::Host::set_repeat(&mut host, &repeat, 4);
        let mut actual = [0; 20];
        assert_eq!(lookup_list::Host::repeat(&host, &mut actual), 4);
        assert_eq!(actual, repeat);
        let mut text = [0; WORD_BYTES];
        text[..8].copy_from_slice(b"\0 name \0");
        lookup_list::Host::replacement(&mut host, &text);
        assert_eq!(cursor.cast_const(), owner.list_source());
        assert_eq!(lookup_symbol::Host::byte(&host, 0), Some(b'n'));
        assert_eq!(lookup_symbol::Host::byte(&host, 158), None);
    }
    #[test]
    fn null_admission_preserves_output_and_missing_effect_clears_only_first_byte() {
        let mut owner = Translator::new(b"en").unwrap();
        let source = *b"cat \0";
        let mut cursor = ptr::null_mut();
        let mut slots = [77, 9, 0];
        let phonemes = [0; PHONEME_BYTES];
        let (ctx, globals, mut effects) = fixture(
            ptr::addr_of_mut!(*owner),
            &source,
            &mut cursor,
            &mut slots,
            &phonemes,
        );
        let mut output = [0x97; 8];
        // SAFETY: initialized outputs/tables; explicit null metadata rejects early.
        let result = unsafe {
            espeak_rs_lookup_symbol_host(
                ptr::null(),
                &globals,
                &effects,
                output.as_mut_ptr(),
                output.len(),
            )
        };
        assert_eq!(result, 0);
        assert_eq!(output, [0x97; 8]);
        effects.translate = None;
        // SAFETY: retained initialized metadata; missing effect rejects admission.
        let result = unsafe {
            espeak_rs_lookup_symbol_host(
                &ctx,
                &globals,
                &effects,
                output.as_mut_ptr(),
                output.len(),
            )
        };
        assert_eq!(result, 0);
        assert_eq!(output, [0, 0x97, 0x97, 0x97, 0x97, 0x97, 0x97, 0x97]);
    }
}
