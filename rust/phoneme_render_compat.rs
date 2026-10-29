//! Serialized C phoneme views, interpreter effects and reusable clause storage.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme::Phoneme,
    phoneme_render::{self as render, Error, Host, Node},
};
use std::{ffi::c_char, ptr, sync::Mutex};

#[repr(C)]
struct List {
    flags: u16,
    code: u8,
    stress: u8,
    source: u16,
    word_stress: u8,
    tone: u8,
    ph: *const Phoneme,
    tone_data: *const Phoneme,
    length: u32,
    env: u8,
    kind: u8,
    prepause: u8,
    amp: u8,
    new_word: u8,
    pitch1: u8,
    pitch2: u8,
    std_length: u8,
    phontab_addr: u32,
    sound_param: i32,
}
#[repr(C)]
struct Table {
    name: [u8; 32],
    records: *const Phoneme,
    count: i32,
    includes: i32,
}
type Ipa = unsafe extern "C" fn(*const List, u32, *mut [u8; 18]) -> i32;
type Alpha = unsafe extern "C" fn(u32) -> i32;
#[derive(Clone, Copy)]
#[repr(C)]
struct Environment {
    list: *const List,
    count: *const i32,
    table: *const *const Phoneme,
    names: *const Table,
    ipa: Option<Ipa>,
    alpha: Option<Alpha>,
}
struct Foreign {
    env: Environment,
    direct: *const List,
    ipa: Ipa,
    alpha: Alpha,
}
impl Foreign {
    fn list(&self, slot: Option<usize>) -> *const List {
        match slot {
            Some(index) if index < render::MAX_LIST => {
                // SAFETY: admitted physical1001-slot list owner remains alive;
                // only the selected initialized node fields are read later.
                unsafe { self.env.list.add(index) }
            }
            Some(_) => ptr::null(),
            None => self.direct,
        }
    }
    fn record(record: *const Phoneme) -> Option<Phoneme> {
        if record.is_null() {
            None
        } else {
            // SAFETY: engine retains nonnull immutable initialized PHONEME_TAB
            // records through all interpreter effects; copied without a loan.
            Some(unsafe { record.read() })
        }
    }
}
impl Host for Foreign {
    fn phoneme(&self, slot: Option<usize>) -> Option<Phoneme> {
        let node = self.list(slot);
        if node.is_null() {
            return None;
        }
        // SAFETY: only the initialized selected record pointer is needed.
        Self::record(unsafe { ptr::addr_of!((*node).ph).read() })
    }
    fn stress_node(&self, slot: Option<usize>) -> Option<Node> {
        let node = self.list(slot);
        if node.is_null() {
            return None;
        }
        // SAFETY: standalone stress requires only initialized synthflags and,
        // for syllables, stresslevel; other node fields can be uninitialized.
        let flags = unsafe { ptr::addr_of!((*node).flags).read() };
        let stress = if flags & 4 != 0 {
            // SAFETY: syllable node's initialized stresslevel, no foreign loan.
            unsafe { ptr::addr_of!((*node).stress).read() }
        } else {
            0
        };
        Some(Node {
            flags,
            stress,
            ..Node::default()
        })
    }
    fn table_code(&self, slot: Option<usize>) -> Option<u8> {
        let node = self.list(slot);
        if node.is_null() {
            return None;
        }
        // SAFETY: switch writing requires only the initialized tone/table byte.
        Some(unsafe { ptr::addr_of!((*node).tone).read() })
    }
    fn count(&self) -> Result<usize, Error> {
        // SAFETY: admitted initialized serialized count slot, freshly read.
        let count = unsafe { self.env.count.read() };
        usize::try_from(count)
            .ok()
            .filter(|n| *n <= render::MAX_LIST)
            .ok_or(Error::Context)
    }
    fn node(&self, slot: Option<usize>) -> Option<Node> {
        let node = self.list(slot);
        if node.is_null() {
            return None;
        }
        // SAFETY: caller retains selected initialized metadata fields and
        // pointed immutable record. No full node/padding/tail read or owner
        // reference crosses a callback; interpreter changes are seen freshly.
        Some(unsafe {
            Node {
                phoneme: Self::record(ptr::addr_of!((*node).ph).read()),
                flags: ptr::addr_of!((*node).flags).read(),
                stress: ptr::addr_of!((*node).stress).read(),
                kind: ptr::addr_of!((*node).kind).read(),
                new_word: ptr::addr_of!((*node).new_word).read(),
                tone: ptr::addr_of!((*node).tone).read(),
            }
        })
    }
    fn special(&self, code: u8) -> Option<Phoneme> {
        // SAFETY: admitted256 initialized table slots, read at point of use.
        Self::record(unsafe { self.env.table.add(usize::from(code)).read() })
    }
    fn table_name(&self, table: u8) -> Option<[u8; 32]> {
        if table >= 150 {
            return None;
        }
        // SAFETY: admitted150 initialized table-name records. Read only the
        // bounded name, not possible padding/uninitialized unrelated fields.
        Some(unsafe { ptr::addr_of!((*self.env.names.add(usize::from(table))).name).read() })
    }
    fn tone(&self, slot: Option<usize>) -> Option<Phoneme> {
        let node = self.list(slot);
        if node.is_null() {
            return None;
        }
        // SAFETY: selected initialized node metadata; no callback/foreign loan.
        let (resolved, code) = unsafe {
            (
                ptr::addr_of!((*node).tone_data).read(),
                ptr::addr_of!((*node).tone).read(),
            )
        };
        Self::record(resolved).or_else(|| self.special(code))
    }
    fn ipa(&mut self, code: u8, slot: Option<usize>, out: &mut [u8; 18]) -> Result<(), Error> {
        let list = self.list(slot);
        if !list.is_null() && self.phoneme(slot).is_none() {
            return Err(Error::Context);
        }
        // SAFETY: synchronous bounded interpreter effect, serialized owner
        // retained; NULL selects direct-code interpretation. Owned initialized
        //18-byte output is disjoint. No foreign state/output loan is held.
        if unsafe { (self.ipa)(list, u32::from(code), out) } == 0 {
            Ok(())
        } else {
            Err(Error::Effect)
        }
    }
    fn alpha(&self, code: u32) -> bool {
        // SAFETY: admitted pure locale wide classifier; all legacy u32 values
        // accepted, cannot mutate/reenter/invalidate engine resources.
        unsafe { (self.alpha)(code) != 0 }
    }
}
unsafe fn host(env: *const Environment, direct: *const List) -> Option<Foreign> {
    if env.is_null() {
        return None;
    }
    // SAFETY: caller supplies one aligned initialized immutable descriptor;
    // pointed owners remain serialized/alive through interpreter callbacks.
    let env = unsafe { env.read() };
    if env.list.is_null() || env.count.is_null() || env.table.is_null() || env.names.is_null() {
        return None;
    }
    Some(Foreign {
        env,
        direct,
        ipa: env.ipa?,
        alpha: env.alpha?,
    })
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_write_phoneme(
    env: *const Environment,
    output: *mut u8,
    capacity: usize,
    ph: *const Phoneme,
    list: *const List,
    ipa: i32,
    stress: u32,
    flags: *mut i32,
) -> *mut c_char {
    if output.is_null()
        || ph.is_null()
        || capacity == 0
        || capacity > isize::MAX as usize
        || stress > 1
    {
        return ptr::null_mut();
    }
    // SAFETY: initialized descriptor and direct list owner, retained for call.
    let Some(mut host) = (unsafe { host(env, list) }) else {
        return ptr::null_mut();
    };
    let Some(ph) = Foreign::record(ph) else {
        return ptr::null_mut();
    };
    let Ok(text) = render::mnemonic(&mut host, ph, None, ipa != 0, stress != 0) else {
        return ptr::null_mut();
    };
    if text.terminated().len() > capacity {
        return ptr::null_mut();
    }
    // SAFETY: caller supplies exclusive/disjoint actual output footprint;
    // emission follows all effects/admission. No foreign tail is read/borrowed.
    unsafe {
        ptr::copy_nonoverlapping(text.terminated().as_ptr(), output, text.terminated().len())
    };
    if !flags.is_null() {
        if let Some(value) = text.flags() {
            // SAFETY: caller's aligned exclusive integer disjoint from output
            // and all owners; untouched when no flags byte is produced.
            unsafe { flags.write(value) };
        }
    }
    // SAFETY: length+NUL admitted inside caller's complete writable footprint.
    unsafe { output.add(text.bytes().len()).cast() }
}
static OUTPUT: Mutex<[u8; render::CLAUSE_BYTES]> = Mutex::new([0; render::CLAUSE_BYTES]);
#[no_mangle]
unsafe extern "C" fn espeak_rs_translated_phonemes(
    env: *const Environment,
    mode: i32,
) -> *const c_char {
    // SAFETY: initialized descriptor; no standalone list is used in this path.
    let Some(mut host) = (unsafe { host(env, ptr::null()) }) else {
        return c"".as_ptr();
    };
    // Serialized engine ABI: output remains valid until the next render. A
    // nested render is refused without deadlock/spin or changing outer output.
    let Ok(mut output) = OUTPUT.try_lock() else {
        return c"".as_ptr();
    };
    if render::clause(&mut host, mode, &mut output[..]).is_err() {
        return c"".as_ptr();
    }
    output.as_ptr().cast()
}
#[no_mangle]
extern "C" fn espeak_rs_phoneme_render_layout(field: u32) -> usize {
    match field {
        0 => std::mem::size_of::<List>(),
        1 => std::mem::offset_of!(List, ph),
        2 => std::mem::offset_of!(List, tone_data),
        3 => std::mem::offset_of!(List, new_word),
        4 => std::mem::size_of::<Table>(),
        5 => std::mem::offset_of!(Table, records),
        _ => 0,
    }
}

/// Phoneme text against the engine's tables: `EncodePhonemes[Bounded]`,
/// `DecodePhonemes`, `WritePhMnemonic[WithStress][Bounded]` and
/// `GetTranslatedPhonemeString`, with the callbacks C used to pass.
#[cfg(espeak_native_host)]
pub(super) mod engine {
    use super::{
        super::{clause_postprocess_compat as post, synthdata_compat as synth},
        Environment, List,
    };
    use crate::phoneme::Phoneme;
    use std::{
        ffi::{c_char, c_int},
        ptr,
    };

    /// `CHAR_MIN < 0`.
    const SIGNED: u32 = (c_char::MIN != 0) as u32;

    fn table() -> *const *const Phoneme {
        std::ptr::addr_of!(synth::phoneme_tab).cast()
    }
    /// The encoder's byte classes: `tolower` and `isspace` of the C locale.
    unsafe extern "C" fn encode_locale(byte: u32) -> u32 {
        let byte = c_int::from(byte as u8);
        // SAFETY: C classifiers of an unsigned char value.
        unsafe { libc::tolower(byte) as u32 | if libc::isspace(byte) != 0 { 256 } else { 0 } }
    }
    /// `isalpha((char)code)`, as the decoder's phoneme-switch names were read.
    unsafe extern "C" fn text_alpha(code: u32) -> c_int {
        // SAFETY: the C classifier of a char value, as C passed it.
        c_int::from(unsafe { libc::isalpha(c_int::from(code as c_char)) } != 0)
    }
    unsafe extern "C" fn ipa(list: *const List, code: u32, out: *mut [u8; 18]) -> i32 {
        let mut data = crate::phoneme_program::PhonemeData::default();
        // SAFETY: serialized engine tables; `list` is null or an entry of the
        // engine's phoneme list; `out` is the renderer's buffer.
        unsafe {
            if list.is_null() {
                synth::InterpretPhoneme2(code as i32, &mut data);
            } else {
                synth::InterpretPhonemeWithLength(
                    ptr::null(),
                    0,
                    list.cast_mut().cast(),
                    std::ptr::addr_of_mut!(post::phoneme_list).cast(),
                    &mut data,
                    ptr::null_mut(),
                    post::n_phoneme_list as usize,
                );
            }
            *out = data.ipa;
        }
        0
    }
    unsafe extern "C" fn alpha(code: u32) -> i32 {
        i32::from(super::super::ucd_isalpha(code) != 0)
    }
    fn environment() -> Environment {
        Environment {
            list: std::ptr::addr_of!(post::phoneme_list).cast(),
            count: std::ptr::addr_of!(post::n_phoneme_list),
            table: table(),
            names: std::ptr::addr_of!(synth::phoneme_tab_list).cast(),
            ipa: Some(ipa),
            alpha: Some(alpha),
        }
    }

    /// `EncodePhonemesBounded` (`EncodePhonemes` passes `PTRDIFF_MAX`).
    ///
    /// # Safety
    /// As the C functions; serialized engine.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_encode_phonemes_engine(
        input: *const c_char,
        output: *mut u8,
        capacity: usize,
        bad: *mut i32,
    ) -> *const c_char {
        // SAFETY: forwarded caller contract and the engine's table.
        unsafe {
            super::super::espeak_rs_encode_phonemes(
                input,
                output,
                capacity,
                bad,
                table(),
                synth::n_phoneme_tab,
                c"en".as_ptr(),
                Some(encode_locale),
            )
        }
    }
    /// `DecodePhonemes`: an empty string when the input cannot be decoded.
    ///
    /// # Safety
    /// As the C function; serialized engine.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_decode_phonemes_engine(input: *const c_char, output: *mut u8) {
        // SAFETY: forwarded caller contract and the engine's table.
        unsafe {
            if super::super::espeak_rs_decode_phonemes_legacy(
                input,
                table(),
                Some(text_alpha),
                SIGNED,
                output,
            ) < 0
            {
                output.write(0);
            }
        }
    }
    /// `DecodePhonemes` into `capacity` bytes: empty, and negative, when it
    /// does not fit.
    ///
    /// # Safety
    /// `input` is terminated; `output` has `capacity` writable bytes.
    #[no_mangle]
    pub(crate) unsafe extern "C" fn espeak_rs_decode_phonemes_bounded_engine(
        input: *const u8,
        output: *mut u8,
        capacity: usize,
    ) -> i32 {
        // SAFETY: caller contract and the engine's table.
        unsafe {
            let length = super::super::engine_c::text(input).len() + 1;
            let status = super::super::espeak_rs_decode_phonemes(
                input,
                length,
                table(),
                Some(text_alpha),
                SIGNED,
                output,
                capacity,
            );
            if status < 0 {
                output.write(0);
            }
            status
        }
    }
    /// `WritePhMnemonicBounded` (`stress` 0) and
    /// `WritePhMnemonicWithStressBounded` (`stress` 1).
    ///
    /// # Safety
    /// As the C functions; serialized engine.
    #[no_mangle]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "C" fn espeak_rs_write_phoneme_engine(
        output: *mut u8,
        capacity: usize,
        ph: *const Phoneme,
        list: *const List,
        ipa: i32,
        stress: u32,
        flags: *mut i32,
    ) -> *mut c_char {
        let env = environment();
        // SAFETY: forwarded caller contract; the descriptor outlives the call.
        unsafe {
            super::espeak_rs_write_phoneme(&env, output, capacity, ph, list, ipa, stress, flags)
        }
    }
    /// `GetTranslatedPhonemeString`.
    ///
    /// # Safety
    /// Serialized engine.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_translated_phonemes_engine(mode: i32) -> *const c_char {
        let env = environment();
        // SAFETY: the descriptor outlives the call.
        unsafe { super::espeak_rs_translated_phonemes(&env, mode) }
    }
}
