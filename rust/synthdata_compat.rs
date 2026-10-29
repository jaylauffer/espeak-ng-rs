//! Complete native resident-phoneme host; serialized raw views of Rust owners.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case, non_upper_case_globals)]
use super::{
    clause_postprocess_compat::{Entry, EMPTY},
    phoneme_assets_compat::{self, Descriptor, Outcome, Projection, TableView},
};
use crate::{
    phoneme::Phoneme,
    phoneme_context::{self, Position, Storage},
    phoneme_program::{PhonemeData, Program},
    spectrum::{FrameRef, Parameters, Settings},
    translator::Translator,
    voice::Voice,
};
use std::{
    ffi::{c_char, c_void},
    io::Write,
    ptr, slice,
    sync::atomic::{AtomicBool, Ordering},
};

#[no_mangle]
pub(super) static version_phdata: i32 = crate::phoneme_data::PHONDATA_VERSION as i32;
#[no_mangle]
pub(super) static mut n_tunes: i32 = 0;
#[no_mangle]
pub(super) static mut tunes: *mut u8 = ptr::null_mut();
#[no_mangle]
static mut n_phoneme_tab: i32 = 0;
#[no_mangle]
static mut phoneme_tab: super::CArray<[*mut Phoneme; 256]> = super::CArray([ptr::null_mut(); 256]);
#[no_mangle]
static mut phoneme_tab_list: super::CArray<[TableView; 150]> = super::CArray(
    [TableView {
        name: [0; 32],
        records: ptr::null_mut(),
        count: 0,
        includes: 0,
    }; 150],
);
#[no_mangle]
static mut phoneme_tab_number: i32 = 0;
#[no_mangle]
static mut wavefile_data: *mut u8 = ptr::null_mut();
#[no_mangle]
static mut seq_len_adjust: i32 = 0;
static mut PROGRAMS: *const u8 = ptr::null();
static mut DATA: *const u8 = ptr::null();
static mut PROGRAM_LENGTH: usize = 0;
static mut DATA_LENGTH: usize = 0;
type Frame = FrameRef<*mut c_void>;
static mut FRAMES: [Frame; crate::spectrum::MAX_FRAMES] = [Frame {
    length: 0,
    flags: 0,
    frame: ptr::null_mut(),
}; crate::spectrum::MAX_FRAMES];
static SPECTRUM_BUSY: AtomicBool = AtomicBool::new(false);
extern "C" {
    static espeak_rs_synthdata_path_width: usize;
    static espeak_rs_synthdata_path_separator: i32;
    static espeak_rs_postprocess_features: [i32; 3];
    static espeak_rs_postprocess_mbrola: *const u8;
    static path_home: c_char;
    static mut voice: *const Voice;
    fn create_file_error_context(context: *mut c_void, status: i32, path: *const c_char) -> i32;
    fn create_version_mismatch_error_context(
        context: *mut c_void,
        path: *const c_char,
        actual: i32,
        expected: i32,
    ) -> i32;
    fn FormantTransitionWithCapacity(
        frames: *mut Frame,
        count: *mut i32,
        transition0: i32,
        transition1: i32,
        other: *const Phoneme,
        which: i32,
        capacity: usize,
    ) -> i32;
}
macro_rules! read {
    ($p:expr, $f:ident) => {{
        // SAFETY: initialized field of a serialized live native record.
        unsafe { ptr::addr_of!((*$p).$f).read() }
    }};
}
macro_rules! write {
    ($p:expr, $f:ident, $v:expr) => {{
        // SAFETY: serialized live native record; publish only the changed field.
        unsafe { ptr::addr_of_mut!((*$p).$f).write($v) }
    }};
}
unsafe extern "C" fn project(_: *mut c_void, view: *const Projection) {
    // SAFETY: initialized exclusive projection callback; resident aliases are
    // revoked before reload/release and recreated only from initialized storage.
    // Copy metadata, never keep a reference to callback scratch.
    unsafe {
        PROGRAMS = (*view).buffers[1];
        DATA = (*view).buffers[2];
        PROGRAM_LENGTH = (*view).lengths[1].max(0) as usize;
        DATA_LENGTH = (*view).lengths[2].max(0) as usize;
        tunes = (*view).buffers[3];
        wavefile_data = (*view).wave;
        n_tunes = (*view).tune_count;
        n_phoneme_tab = (*view).active;
        phoneme_tab_number = (*view).number;
        ptr::copy_nonoverlapping(
            ptr::addr_of!((*view).tables).cast::<TableView>(),
            ptr::addr_of_mut!(phoneme_tab_list).cast(),
            150,
        );
        ptr::copy_nonoverlapping(
            ptr::addr_of!((*view).selected).cast::<*mut Phoneme>(),
            ptr::addr_of_mut!(phoneme_tab).cast(),
            256,
        );
    }
}
unsafe fn control(
    operation: i32,
    argument: i32,
    name: *const c_char,
    rate: *mut i32,
    context: *mut c_void,
) -> i32 {
    // SAFETY: initialized stable config and serialized table-number snapshot.
    let descriptor = unsafe {
        Descriptor {
            context,
            home: ptr::addr_of!(path_home),
            name,
            width: espeak_rs_synthdata_path_width,
            separator: espeak_rs_synthdata_path_separator,
            number: phoneme_tab_number,
            argument,
            operation,
            project: Some(project),
            file_error: Some(create_file_error_context),
            version_error: Some(create_version_mismatch_error_context),
        }
    };
    let mut result = Outcome { rate: 0, number: 0 };
    // SAFETY: copied descriptor, initialized names/path on applicable operations,
    // live borrowed error allocator; existing bounded proactor owns contents I/O.
    let status =
        unsafe { phoneme_assets_compat::espeak_rs_phoneme_assets(&descriptor, &mut result) };
    if status != 0 {
        return if operation <= 1 { status } else { -1 };
    }
    // SAFETY: native publication only after all callbacks and resource reads finish.
    unsafe {
        if operation >= 3 && result.number >= 0 {
            phoneme_tab_number = result.number;
        }
        if !rate.is_null() {
            rate.write(result.rate);
        }
    }
    if operation >= 3 {
        result.number
    } else {
        0
    }
}
#[no_mangle]
unsafe extern "C" fn LoadPhData(rate: *mut i32, context: *mut c_void) -> i32 {
    // SAFETY: whole-engine serialized resource admission and borrowed error/output views.
    unsafe { control(0, 0, ptr::null(), rate, context) }
}
#[no_mangle]
unsafe extern "C" fn FreePhData() {
    // SAFETY: caller drains all raw consumers before revocation and release.
    let _ = unsafe { control(1, 0, ptr::null(), ptr::null_mut(), ptr::null_mut()) };
}
#[no_mangle]
unsafe extern "C" fn SelectPhonemeTable(number: i32) {
    // SAFETY: serialized initialized table owner.
    let _ = unsafe { control(2, number, ptr::null(), ptr::null_mut(), ptr::null_mut()) };
}
#[no_mangle]
unsafe extern "C" fn LookupPhonemeTable(name: *const c_char) -> i32 {
    // SAFETY: serialized initialized terminated table name.
    unsafe { control(3, 0, name, ptr::null_mut(), ptr::null_mut()) }
}
#[no_mangle]
unsafe extern "C" fn SelectPhonemeTableName(name: *const c_char) -> i32 {
    // SAFETY: serialized initialized terminated table name.
    unsafe { control(4, 0, name, ptr::null_mut(), ptr::null_mut()) }
}
#[no_mangle]
unsafe extern "C" fn PhonemeCode(word: u32) -> i32 {
    // SAFETY: initialized selected table slots and retained resident records.
    unsafe {
        super::espeak_rs_phoneme_code(
            ptr::addr_of!(phoneme_tab).cast(),
            n_phoneme_tab as usize,
            word,
        ) as i32
    }
}
#[no_mangle]
unsafe extern "C" fn LookupPhonemeString(name: *const c_char) -> i32 {
    if name.is_null() {
        return 0;
    }
    let mut bytes = [0; 4];
    for (i, b) in bytes.iter_mut().enumerate() {
        // SAFETY: only the original four-byte/NUL mnemonic prefix is required.
        *b = unsafe { name.add(i).cast::<u8>().read() };
        if *b == 0 {
            break;
        }
    }
    // SAFETY: initialized retained selected table.
    unsafe { PhonemeCode(crate::phoneme::mnemonic(&bytes)) }
}
#[no_mangle]
unsafe extern "C" fn RustPhonemeDataLength() -> usize {
    // SAFETY: serialized initialized retained extent.
    unsafe { DATA_LENGTH }
}
#[no_mangle]
unsafe extern "C" fn RustPhonemePrograms(length: *mut usize) -> *const u8 {
    if length.is_null() {
        return ptr::null();
    }
    // SAFETY: serialized retained extent and writable disjoint scalar output.
    unsafe {
        length.write(PROGRAM_LENGTH);
        PROGRAMS
    }
}
#[no_mangle]
unsafe extern "C" fn RustInvalidInstruction(ph: *const Phoneme, instruction: i32) {
    if ph.is_null() {
        return;
    }
    let name = read!(ph, mnemonic).to_le_bytes();
    let length = name.iter().position(|b| *b == 0).unwrap_or(4);
    let mut stderr = std::io::stderr().lock();
    let _ = std::write!(
        stderr,
        "Invalid instruction {:04x} for phoneme '",
        instruction as u32
    );
    let _ = stderr.write_all(&name[..length]);
    let _ = stderr.write_all(b"'\n");
}
struct LiveStorage {
    start: *mut Entry,
    length: usize,
    word: *mut Entry,
    phoneme: Phoneme,
}
impl LiveStorage {
    fn pointer(&self, position: Position) -> Option<*mut Entry> {
        match position {
            Position::List(i) if i < self.length => {
                // SAFETY: admitted complete initialized clause extent.
                Some(unsafe { self.start.add(i) })
            }
            Position::PreviousVowel
                if !self.word.is_null() && !read!(self.word, phoneme).is_null() =>
            {
                Some(self.word)
            }
            _ => None,
        }
    }
}
impl Storage for LiveStorage {
    fn entry(&self, position: Position) -> Option<phoneme_context::Entry> {
        let p = self.pointer(position)?;
        let ph = read!(p, phoneme);
        Some(phoneme_context::Entry {
            // SAFETY: immutable initialized resident 16-byte phoneme record.
            phoneme: unsafe { ph.as_ref().copied() },
            code: read!(p, code),
            stress: read!(p, stress),
            word_stress: read!(p, word_stress),
            source: read!(p, source),
            flags: read!(p, flags),
        })
    }
    fn phoneme(&self, code: u8) -> Option<Phoneme> {
        // SAFETY: serialized initialized table pointer and retained immutable record.
        unsafe { phoneme_tab.0[usize::from(code)].as_ref().copied() }
    }
    fn refresh(&mut self, position: Position) {
        // Original refresh of previous vowel permits a null current ph when a
        // word view exists; read() still refuses an absent previous vowel.
        let p = match position {
            Position::PreviousVowel if !self.word.is_null() => self.word,
            _ => match self.pointer(position) {
                Some(p) => p,
                None => return,
            },
        };
        let code = read!(p, code);
        // SAFETY: initialized selected table, exclusive serialized pointer field.
        let ph = unsafe { phoneme_tab.0[usize::from(code)] };
        write!(p, phoneme, ph);
    }
    fn invalid_instruction(&mut self, instruction: u16) {
        // SAFETY: copied initialized phoneme metadata, diagnostic retains no loan.
        unsafe {
            RustInvalidInstruction(&self.phoneme, i32::from(instruction));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn InterpretPhonemeWithLength(
    tr: *const Translator,
    control: i32,
    current: *mut Entry,
    start: *mut Entry,
    out: *mut PhonemeData,
    word: *mut Entry,
    length: usize,
) {
    if current.is_null() || start.is_null() || out.is_null() {
        return;
    }
    let stride = std::mem::size_of::<Entry>();
    let Some(offset) = (current as usize).checked_sub(start as usize) else {
        return;
    };
    if length == 0 || length > 1001 || offset % stride != 0 || offset / stride >= length {
        return;
    }
    let ph = read!(current, phoneme);
    if !word.is_null() && read!(current, source) != 0 {
        write!(word, phoneme, ptr::null_mut());
    }
    // SAFETY: exclusive initialized output, disjoint from clause/resident inputs.
    unsafe {
        out.write_bytes(0, 1);
    }
    if ph.is_null() {
        return;
    }
    // SAFETY: initialized immutable resident phoneme copied before effects.
    let phoneme = unsafe { ph.read() };
    write!(out, parameters, {
        let mut p = [0; 16];
        p[10] = i32::from(phoneme.standard_length);
        p[9] = i32::from(phoneme.length_modifier);
        p
    });
    if phoneme.program == 0 {
        return;
    }
    // SAFETY: initialized settings, immutable resident program extent; no foreign
    // reference survives diagnostics, table refresh or native publication.
    let (programs, extent, klatt, mbrola, reduction) = unsafe {
        (
            PROGRAMS,
            PROGRAM_LENGTH,
            espeak_rs_postprocess_features[0] != 0 && !voice.is_null() && (*voice).klatt[0] != 0,
            espeak_rs_postprocess_features[1] != 0
                && !espeak_rs_postprocess_mbrola.is_null()
                && espeak_rs_postprocess_mbrola.read() != 0,
            if tr.is_null() {
                0
            } else {
                (*tr).postprocess_list().2
            },
        )
    };
    if programs.is_null() || extent > isize::MAX as usize {
        return;
    }
    let settings = phoneme_context::Settings {
        length,
        current: offset / stride,
        control: control as u32,
        has_translator: u32::from(!tr.is_null()),
        reduction,
        klatt: u32::from(klatt),
        mbrola: u32::from(mbrola),
    };
    // SAFETY: admitted retained immutable resident program bytes; callbacks only
    // refresh row pointers or emit diagnostics, never reload resources.
    let result =
        Program::new(unsafe { slice::from_raw_parts(programs, extent) }).and_then(|program| {
            let mut context = phoneme_context::Context::new(
                program,
                settings,
                LiveStorage {
                    start,
                    length,
                    word,
                    phoneme,
                },
            )?;
            program.interpret(
                &phoneme,
                settings.control,
                settings.has_translator != 0,
                &mut context,
            )
        });
    let Ok(result) = result else {
        return;
    };
    let is_vowel = !word.is_null() && read!(current, kind) == 2;
    // SAFETY: no loans cross publication; byte copy preserves original untouched
    // fields/padding even for C-provided rows with unused uninitialized tails.
    unsafe {
        super::write_phoneme_data(out, result);
        if is_vowel {
            ptr::copy(current, word, 1);
        }
    }
    let standard_length = read!(out, parameters)[10] as u8;
    write!(current, std_length, standard_length);
    let addresses = read!(out, sound_addresses);
    let parameters = read!(out, sound_parameters);
    let sound = usize::from(addresses[0] == 0);
    write!(current, phoneme_address, addresses[sound] as u32);
    write!(current, sound_parameter, parameters[sound]);
}
#[no_mangle]
unsafe extern "C" fn InterpretPhoneme2WithData(code: i32, ph: *mut Phoneme, out: *mut PhonemeData) {
    if out.is_null() {
        return;
    }
    if ph.is_null() {
        // SAFETY: exclusive aligned native output; the null-phoneme path clears it.
        unsafe {
            out.write_bytes(0, 1);
        }
        return;
    }
    let mut list = [EMPTY; 4];
    // SAFETY: serialized current table pause record, retained through pure VM.
    let pause = unsafe { phoneme_tab.0[9] };
    for p in &mut list {
        p.code = 9;
        p.phoneme = pause;
    }
    list[1].code = code as u8;
    list[1].phoneme = ph;
    list[2].source = 1;
    // SAFETY: initialized local clause/disjoint output, immutable resident records.
    unsafe {
        InterpretPhonemeWithLength(
            ptr::null(),
            0,
            list.as_mut_ptr().add(1),
            list.as_mut_ptr(),
            out,
            ptr::null_mut(),
            4,
        );
    }
}
#[no_mangle]
unsafe extern "C" fn InterpretPhoneme2(code: i32, out: *mut PhonemeData) {
    if !(0..256).contains(&code) {
        return;
    }
    // SAFETY: bounded current table slot, initialized disjoint output.
    unsafe {
        InterpretPhoneme2WithData(code, phoneme_tab.0[code as usize], out);
    }
}
#[no_mangle]
unsafe extern "C" fn TonePhoneme(p: *const Entry) -> *mut Phoneme {
    if p.is_null() {
        return ptr::null_mut();
    }
    let tone = read!(p, tone_data);
    if !tone.is_null() {
        tone
    } else {
        let code = read!(p, tone);
        // SAFETY: serialized initialized table; table-local code is one byte.
        unsafe { phoneme_tab.0[usize::from(code)] }
    }
}
struct SpectrumGuard;
impl Drop for SpectrumGuard {
    fn drop(&mut self) {
        SPECTRUM_BUSY.store(false, Ordering::Release);
    }
}
unsafe extern "C" fn transition(
    _: *mut c_void,
    frames: *mut Frame,
    count: *mut i32,
    parameters: *const Parameters,
    which: i32,
    adjust: *mut i32,
    capacity: usize,
) -> i32 {
    // SAFETY: native synchronous transition owns frame refs/pooled copies only;
    // no global spectrum reference is retained across the call.
    unsafe {
        seq_len_adjust = adjust.read();
        let added = FormantTransitionWithCapacity(
            frames,
            count,
            (*parameters).transition0,
            (*parameters).transition1,
            ptr::null(),
            which,
            capacity,
        );
        adjust.write(seq_len_adjust);
        added
    }
}
#[no_mangle]
unsafe extern "C" fn LookupSpect(
    ph: *const Phoneme,
    which: i32,
    parameters: *const Parameters,
    count: *mut i32,
    entry: *const Entry,
) -> *mut Frame {
    if ph.is_null() || parameters.is_null() || count.is_null() || entry.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: exclusive scalar output; unused host fields need no initialization.
    unsafe {
        count.write(0);
    }
    if SPECTRUM_BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return ptr::null_mut();
    }
    let _guard = SpectrumGuard;
    let is_vowel = u32::from(read!(ph, kind) == 2);
    let lengthened = u32::from(read!(entry, flags) & 0x08 != 0);
    // SAFETY: initialized copied settings and immutable resident resource extent.
    let (settings, bytes, extent) = unsafe {
        seq_len_adjust = 0;
        (
            Settings {
                which,
                is_vowel,
                lengthened,
                lengthen_length: phoneme_tab.0[12]
                    .as_ref()
                    .map_or(0, |p| i32::from(p.standard_length)),
            },
            DATA,
            DATA_LENGTH,
        )
    };
    let mut result = super::SpectrumSelection {
        start: 0,
        count: 0,
        length_adjust: 0,
    };
    let frames = ptr::addr_of_mut!(FRAMES).cast::<Frame>();
    // SAFETY: exclusive retained frame storage; resident bytes immutable through
    // formant effects. Legacy result aliases remain valid until next lookup.
    let status = unsafe {
        super::espeak_rs_spectrum_lookup(
            bytes,
            extent,
            parameters,
            &settings,
            ptr::null_mut(),
            Some(transition),
            frames,
            &mut result,
        )
    };
    if status != 0 {
        return ptr::null_mut();
    }
    // SAFETY: validated bounded selection and serialized global/scalar publication.
    unsafe {
        count.write(result.count as i32);
        seq_len_adjust = result.length_adjust;
        frames.add(result.start)
    }
}
#[no_mangle]
unsafe extern "C" fn GetEnvelope(index: i32) -> *const u8 {
    // SAFETY: serialized initialized immutable resident bytes/extent.
    let envelope = unsafe { super::espeak_rs_envelope(DATA, DATA_LENGTH, index) };
    if !envelope.is_null() {
        return envelope;
    }
    eprintln!("espeak: No envelope");
    super::envelope_data.0[0].as_ptr()
}
