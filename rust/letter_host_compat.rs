//! Connected native letter host over typed state and primitive engine effects.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::list_host_compat::SourceGuard;
use crate::{
    letter_lookup::{self, Host},
    number_lookup::PHONEME_BYTES,
    translator::Translator,
    voice::Voice,
};
use std::{ffi::c_void, ptr};
#[repr(C)]
#[derive(Clone, Copy)]
struct Globals {
    secondary: *const *mut Translator,
    voice: *const *mut Voice,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Effects {
    lookup: Option<unsafe extern "C" fn(*mut c_void, *const u8, *mut u8) -> i32>,
    space: Option<unsafe extern "C" fn(u32) -> i32>,
    rules: Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, u32, *mut u8)>,
    setup: Option<unsafe extern "C" fn()>,
    restore: Option<unsafe extern "C" fn(i32)>,
    stress: Option<unsafe extern "C" fn(*mut c_void, *mut u8, *mut u32, i32)>,
}
struct Engine {
    tr: *mut Translator,
    globals: Globals,
    effects: Effects,
    failed: bool,
}
impl Host for Engine {
    fn lookup(
        &mut self,
        source: &mut [u8; 10],
        start: usize,
        secondary: bool,
        output: &mut [u8; PHONEME_BYTES],
    ) -> i32 {
        if start >= source.len() {
            self.failed = true;
            output[0] = 0;
            return 0;
        }
        let tr = if secondary {
            // SAFETY: initialized retained serialized secondary slot, read fresh
            // after setup; null means absent secondary and clears scratch only.
            unsafe { self.globals.secondary.read() }
        } else {
            self.tr
        };
        if tr.is_null() {
            output[0] = 0;
            return 0;
        }
        let lookup = self.effects.lookup.expect("admitted lookup");
        // SAFETY: owned initialized NUL-containing source and 200-byte scratch;
        // selected typed owner retained, no foreign reference spans callback.
        unsafe {
            lookup(
                tr.cast(),
                source.as_mut_ptr().add(start).cast_const(),
                output.as_mut_ptr(),
            )
        }
    }
    fn named(&mut self, key: &[u8], output: &mut [u8; PHONEME_BYTES]) -> i32 {
        let lookup = self.effects.lookup.expect("admitted lookup");
        // SAFETY: controller's static terminated name and owned initialized
        // scratch; retained serialized owner, no source/owner loan across effect.
        unsafe { lookup(self.tr.cast(), key.as_ptr(), output.as_mut_ptr()) }
    }
    fn value(&self, field: u32) -> i32 {
        // SAFETY: fresh short typed scalar projection between callbacks.
        unsafe { (*self.tr).letter_value(field) }
    }
    fn space(&self, code: u32) -> bool {
        // SAFETY: retained pure platform classification primitive.
        unsafe { self.effects.space.expect("admitted classifier")(code) != 0 }
    }
    fn rules(
        &mut self,
        source: &mut [u8; 10],
        start: usize,
        capacity: usize,
        flags: u32,
        output: &mut [u8; PHONEME_BYTES],
    ) {
        if start >= source.len() || capacity == 0 || capacity > output.len() {
            self.failed = true;
            return;
        }
        let base = source.as_mut_ptr();
        // SAFETY: serialized live owner and owned initialized source retained
        // until guard restores metadata; no foreign loan spans nested rules.
        let guard = unsafe { SourceGuard::enter(self.tr, base.cast_const(), source.len()) };
        let rules = self.effects.rules.expect("admitted rules");
        // SAFETY: initialized owned padded source and scratch; preserve the
        // dictionary prefix for no-op/appending child rules, without replay.
        unsafe {
            rules(
                self.tr.cast(),
                base.add(start),
                capacity,
                flags,
                output.as_mut_ptr(),
            );
        }
        drop(guard);
    }
    fn select(&mut self, restore: bool) {
        if restore {
            // SAFETY: retained initialized serialized global pointer slot;
            // children may replace the voice, so read pointer/table fresh.
            let voice = unsafe { self.globals.voice.read() };
            if voice.is_null() {
                self.failed = true;
                return;
            }
            // SAFETY: live acoustic C-layout snapshot; field scalar only, never
            // lend the full voice object through selection effect.
            let table = unsafe { (*voice).phoneme_table };
            // SAFETY: serialized primitive receives only the copied table.
            unsafe {
                self.effects.restore.expect("admitted restore")(table);
            }
        } else {
            // SAFETY: retained serialized default-secondary setup effect; later
            // lookup reads its published secondary slot, not a cached pointer.
            unsafe {
                self.effects.setup.expect("admitted setup")();
            }
        }
    }
    fn stress(&mut self, output: &mut [u8; PHONEME_BYTES], flags: &mut [u32; 2], control: i32) {
        let stress = self.effects.stress.expect("admitted stress");
        // SAFETY: initialized terminated owned scratch/two flags; serialized
        // owner survives effect, and neither owner/output loan spans it.
        unsafe {
            stress(
                self.tr.cast(),
                output.as_mut_ptr(),
                flags.as_mut_ptr(),
                control,
            );
        }
    }
}
/// Native contract in rust_letter_lookup.h. Output is borrowed only at final
/// raw publication; all nested effects use owned initialized bounded scratch.
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_letter_host(
    tr: *mut Translator,
    globals: *const Globals,
    effects: *const Effects,
    letter: u32,
    next: i32,
    control: i32,
    accent: i32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if tr.is_null()
        || globals.is_null()
        || effects.is_null()
        || output.is_null()
        || capacity == 0
        || capacity > PHONEME_BYTES
        || !(0..=1).contains(&accent)
    {
        return -1;
    }
    // SAFETY: retained initialized immutable metadata; copy before any effect.
    let (globals, effects) = unsafe { (globals.read(), effects.read()) };
    if globals.secondary.is_null()
        || globals.voice.is_null()
        || effects.lookup.is_none()
        || effects.space.is_none()
        || effects.rules.is_none()
        || effects.setup.is_none()
        || effects.restore.is_none()
        || effects.stress.is_none()
    {
        return -1;
    }
    let mut engine = Engine {
        tr,
        globals,
        effects,
        failed: false,
    };
    let mut scratch = [0; PHONEME_BYTES];
    let result = if accent == 1 {
        letter_lookup::accented(&mut engine, letter, &mut scratch[..capacity])
    } else {
        letter_lookup::letter(&mut engine, letter, next, control, &mut scratch[..capacity])
            .map(Some)
    };
    if engine.failed {
        return -1;
    }
    match result {
        Ok(Some(written)) => {
            // SAFETY: exclusive actual caller output extent after all callbacks;
            // copy only validated initialized terminated prefix, preserving tail.
            unsafe {
                ptr::copy_nonoverlapping(scratch.as_ptr(), output, written + 1);
            }
            1
        }
        Ok(None) => 0,
        Err(_) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    static RESTORED: AtomicI32 = AtomicI32::new(-1);
    unsafe extern "C" fn lookup(tr: *mut c_void, _: *const u8, out: *mut u8) -> i32 {
        // SAFETY: retained serialized typed owner; short scalar projection.
        let language = unsafe { (*tr.cast::<Translator>()).letter_value(0) };
        // SAFETY: initialized owned 200-byte scratch admitted by engine.
        unsafe {
            out.write(language as u8);
            out.add(1).write(0);
        }
        language
    }
    unsafe extern "C" fn space(_: u32) -> i32 {
        0
    }
    unsafe extern "C" fn rules(
        tr: *mut c_void,
        text: *mut u8,
        capacity: usize,
        _: u32,
        out: *mut u8,
    ) {
        assert_eq!(capacity, 20);
        let tr = tr.cast::<Translator>();
        // SAFETY: retained typed owner; metadata was scoped before child call.
        let (base, length) = unsafe { (*tr).rule_source() };
        assert_eq!(base, text.wrapping_sub(2));
        assert_eq!(length, 10);
        // SAFETY: mutable initialized owned text/scratch, serialized owner; a
        // nested child may alter source metadata, which outer guard restores.
        unsafe {
            text.write(b'y');
            out.add(1).write(b'b');
            out.add(2).write(0);
            (*tr).rule_set_source(ptr::null(), 93);
        }
    }
    unsafe extern "C" fn setup() {}
    unsafe extern "C" fn restore(table: i32) {
        RESTORED.store(table, Ordering::SeqCst);
    }
    unsafe extern "C" fn stress(_: *mut c_void, _: *mut u8, _: *mut u32, _: i32) {}
    fn effects() -> Effects {
        Effects {
            lookup: Some(lookup),
            space: Some(space),
            rules: Some(rules),
            setup: Some(setup),
            restore: Some(restore),
            stress: Some(stress),
        }
    }
    #[test]
    fn null_secondary_clears_only_scratch_head_and_lookup_reads_replaced_slot() {
        let mut owner = Translator::new(b"en").unwrap();
        let mut second = Translator::new(b"fr").unwrap();
        let mut secondary = ptr::null_mut();
        let mut voice = ptr::null_mut();
        let mut engine = Engine {
            tr: ptr::addr_of_mut!(*owner),
            globals: Globals {
                secondary: &mut secondary,
                voice: &mut voice,
            },
            effects: effects(),
            failed: false,
        };
        let mut source = *b"\0_x \0\0\0\0\0\0";
        let mut out = [0x97; PHONEME_BYTES];
        assert_eq!(engine.lookup(&mut source, 1, true, &mut out), 0);
        assert_eq!(&out[..2], &[0, 0x97]);
        secondary = ptr::addr_of_mut!(*second);
        assert_eq!(engine.lookup(&mut source, 1, true, &mut out), 0x6672);
        assert_eq!(&out[..2], b"r\0");
        assert!(!secondary.is_null());
        assert!(!engine.failed);
    }
    #[test]
    fn child_rules_keep_prefix_mutate_owned_source_and_restore_outer_metadata() {
        let mut owner = Translator::new(b"en").unwrap();
        let saved = *b"saved\0";
        owner.rule_set_source(saved.as_ptr(), saved.len());
        let mut secondary = ptr::null_mut();
        let mut voice = ptr::null_mut();
        let mut engine = Engine {
            tr: ptr::addr_of_mut!(*owner),
            globals: Globals {
                secondary: &mut secondary,
                voice: &mut voice,
            },
            effects: effects(),
            failed: false,
        };
        let mut source = *b"\0 x \0\0\0\0\0\0";
        let mut out = [0; PHONEME_BYTES];
        out[0] = b'a';
        engine.rules(&mut source, 2, 20, 0, &mut out);
        assert_eq!(&out[..3], b"ab\0");
        assert_eq!(source[2], b'y');
        assert_eq!(owner.rule_source(), (saved.as_ptr(), saved.len()));
    }
    #[test]
    fn restoration_reads_current_voice_slot_and_records_missing_voice_failure() {
        let mut owner = Translator::new(b"en").unwrap();
        let mut first = Voice {
            phoneme_table: 5,
            ..Voice::default()
        };
        let mut next = Voice {
            phoneme_table: 19,
            ..Voice::default()
        };
        let mut secondary = ptr::null_mut();
        let mut voice = ptr::addr_of_mut!(first);
        let mut engine = Engine {
            tr: ptr::addr_of_mut!(*owner),
            globals: Globals {
                secondary: &mut secondary,
                voice: &mut voice,
            },
            effects: effects(),
            failed: false,
        };
        engine.select(true);
        assert_eq!(RESTORED.load(Ordering::SeqCst), 5);
        voice = ptr::addr_of_mut!(next);
        engine.select(true);
        assert_eq!(RESTORED.load(Ordering::SeqCst), 19);
        assert!(!voice.is_null());
        voice = ptr::null_mut();
        engine.select(true);
        assert!(engine.failed);
        assert_eq!(RESTORED.load(Ordering::SeqCst), 19);
        assert!(voice.is_null());
        let mut out = [0x97; 8];
        // SAFETY: live metadata/owner and actual output extent; zero capacity
        // rejects before any engine effect or output publication.
        let result = unsafe {
            espeak_rs_lookup_letter_host(
                engine.tr,
                &engine.globals,
                &engine.effects,
                b'x' as u32,
                32,
                0,
                0,
                out.as_mut_ptr(),
                0,
            )
        };
        assert_eq!(result, -1);
        assert_eq!(out, [0x97; 8]);
    }
}
/// The letter lookup's primitives against the engine (from numbers.c).
#[cfg(espeak_native_host)]
pub(super) mod engine {
    use super::{super::engine_c as c, Effects, Globals};
    use crate::{translator::Translator, voice::Voice};
    use std::ffi::{c_int, c_void};

    unsafe extern "C" fn lookup(tr: *mut c_void, word: *const u8, output: *mut u8) -> i32 {
        // SAFETY: terminated word and 200-byte output.
        unsafe { c::LookupBounded(tr, word.cast(), output.cast(), 200) }
    }
    unsafe extern "C" fn space(code: u32) -> i32 {
        super::super::ucd_isspace(code)
    }
    unsafe extern "C" fn rules(
        tr: *mut c_void,
        word: *mut u8,
        capacity: usize,
        flags: u32,
        output: *mut u8,
    ) {
        // SAFETY: the host's word buffer and `capacity`-byte output.
        unsafe {
            c::TranslateRules(
                tr,
                word.cast(),
                output.cast(),
                capacity as c_int,
                std::ptr::null_mut(),
                flags as c_int,
                std::ptr::null_mut(),
            )
        };
    }
    unsafe extern "C" fn setup() {
        // SAFETY: serialized engine.
        unsafe { c::SetTranslator3(c"en".as_ptr()) };
    }
    unsafe extern "C" fn restore(table: i32) {
        // SAFETY: serialized engine.
        unsafe { super::super::synthdata_compat::SelectPhonemeTable(table) };
    }
    unsafe extern "C" fn stress(tr: *mut c_void, output: *mut u8, flags: *mut u32, control: i32) {
        // SAFETY: the host's phoneme buffer and two flags.
        unsafe { c::SetWordStress(tr, output.cast(), flags, -1, control) };
    }
    static EFFECTS: Effects = Effects {
        lookup: Some(lookup),
        space: Some(space),
        rules: Some(rules),
        setup: Some(setup),
        restore: Some(restore),
        stress: Some(stress),
    };
    /// `LookupLetterBounded` (`accent` 0) and `LookupAccentedLetterBounded`
    /// (`accent` 1).
    ///
    /// # Safety
    /// As the C functions; serialized engine.
    #[no_mangle]
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe extern "C" fn espeak_rs_lookup_letter_engine(
        tr: *mut Translator,
        letter: u32,
        next: i32,
        control: i32,
        accent: i32,
        output: *mut u8,
        capacity: usize,
    ) -> i32 {
        // The engine's secondary translator and voice slots.
        let globals = Globals {
            secondary: std::ptr::addr_of!(c::translator3).cast::<*mut Translator>(),
            voice: std::ptr::addr_of!(c::voice).cast::<*mut Voice>(),
        };
        // SAFETY: forwarded caller contract; the tables outlive the call.
        unsafe {
            super::espeak_rs_lookup_letter_host(
                tr, &globals, &EFFECTS, letter, next, control, accent, output, capacity,
            )
        }
    }
}
