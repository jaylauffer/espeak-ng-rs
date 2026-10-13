//! Complete native clause list/pitch/length host and retained storage.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case, non_upper_case_globals)]
use super::clause_postprocess_storage::Lease;
use super::parameter_control_compat::{self, Embedded, Parameter};
use crate::{
    intonation, lengths,
    phoneme::Phoneme,
    phoneme_list::{self, Replacement, Source},
    phoneme_program::PhonemeData,
    speed,
    translator::Translator,
    voice::Voice,
};
use std::{ptr, slice};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry {
    pub(super) flags: u16,
    pub(super) code: u8,
    pub(super) stress: u8,
    pub(super) source: u16,
    pub(super) word_stress: u8,
    pub(super) tone: u8,
    pub(super) phoneme: *mut Phoneme,
    pub(super) tone_data: *mut Phoneme,
    pub(super) length: u32,
    pub(super) env: u8,
    pub(super) kind: u8,
    pub(super) prepause: u8,
    pub(super) amp: u8,
    pub(super) new_word: u8,
    pub(super) pitch1: u8,
    pub(super) pitch2: u8,
    pub(super) std_length: u8,
    pub(super) phoneme_address: u32,
    pub(super) sound_parameter: i32,
}
pub(super) const EMPTY: Entry = Entry {
    flags: 0,
    code: 0,
    stress: 0,
    source: 0,
    word_stress: 0,
    tone: 0,
    phoneme: ptr::null_mut(),
    tone_data: ptr::null_mut(),
    length: 0,
    env: 0,
    kind: 0,
    prepause: 0,
    amp: 0,
    new_word: 0,
    pitch1: 0,
    pitch2: 0,
    std_length: 0,
    phoneme_address: 0,
    sound_parameter: 0,
};
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<Entry>() == 48);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<Entry>() == 36);
#[no_mangle]
static mut phoneme_list: [Entry; 1001] = [EMPTY; 1001];
#[no_mangle]
static mut n_phoneme_list: i32 = 0;
#[no_mangle]
static mut ph_list2: [Source; 1000] = [Source {
    synthflags: 0,
    code: 0,
    stress: 0,
    source: 0,
    word_stress: 0,
    tone: 0,
}; 1000];
#[no_mangle]
static mut n_ph_list2: i32 = 0;
#[no_mangle]
static mut replace_phonemes: [Replacement; 60] = [Replacement {
    old: 0,
    new: 0,
    kind: 0,
}; 60];
#[no_mangle]
static mut n_replace_phonemes: i32 = 0;
#[no_mangle]
static mut speed: speed::Factors = speed::Factors {
    pause: 0,
    clause_pause: 0,
    minimum_pause: 0,
    wave: 0,
    length_modifier: 0,
    length_modifier2: 0,
    minimum_sample: 0,
    fast_settings: 0,
};
static mut LEN_SPEEDS: [i32; 3] = [130, 121, 118];
extern "C" {
    static espeak_rs_postprocess_features: [i32; 3];
    static mut phoneme_tab: [*mut Phoneme; 256];
    static mut n_phoneme_tab: i32;
    static mut tunes: *const u8;
    static mut n_tunes: i32;
    static mut option_wordgap: i32;
    static mut option_tone_flags: i32;
    static mut option_linelength: i32;
    static mut voice: *const Voice;
    static mut translator: *mut std::ffi::c_void;
    static espeak_rs_postprocess_mbrola: *const u8;
    static mut embedded_list: [u32; 250];
    static mut embedded_value: [i32; 15];
    static param_defaults: [i32; crate::ssml_parameters::PARAMETERS];
    static mut saved_parameters: [i32; crate::ssml_parameters::PARAMETERS];
    static mut param_stack: [crate::ssml_parameters::Frame; crate::ssml_parameters::STACK];
    fn SelectPhonemeTable(index: i32);
    fn RustInvalidInstruction(phoneme: *const Phoneme, instruction: i32);
    fn RustPhonemePrograms(length: *mut usize) -> *const u8;
    fn TonePhoneme(entry: *const Entry) -> *mut Phoneme;
    fn InterpretPhoneme2WithData(code: i32, phoneme: *mut Phoneme, data: *mut PhonemeData);
    fn GetEnvelope(index: i32) -> *const u8;
    fn SetEmbedded(command: i32, value: i32);
    fn GetAmplitude() -> i32;
    static mut espeak_rs_wave_memory: crate::wave_memory::WaveMemory;
    fn espeak_rs_queue_sonic_speed(memory: *mut crate::wave_memory::WaveMemory, value: i32);
}
struct ListHost;
impl phoneme_list::Host for ListHost {
    fn select(
        &mut self,
        index: i32,
        table: &mut phoneme_list::Table,
    ) -> Result<(), phoneme_list::Error> {
        // SAFETY: serialized resident table selection; no source/output reference crosses it.
        unsafe {
            SelectPhonemeTable(index);
        }
        for (i, record) in table.iter_mut().enumerate() {
            // SAFETY: initialized stable table slots and retained immutable records.
            *record = unsafe {
                ptr::addr_of!(phoneme_tab)
                    .cast::<*mut Phoneme>()
                    .add(i)
                    .read()
                    .as_ref()
                    .copied()
            };
        }
        Ok(())
    }
    fn invalid_instruction(&mut self, phoneme: &Phoneme, instruction: u16) {
        // SAFETY: bounded synchronous diagnostic borrowing only this copied record.
        unsafe {
            RustInvalidInstruction(phoneme, i32::from(instruction));
        }
    }
}
macro_rules! read {
    ($p:expr,$f:ident) => {{
        // SAFETY: serialized live native row; read only the initialized field used by C.
        unsafe { ptr::addr_of!((*$p).$f).read() }
    }};
}
macro_rules! write {
    ($p:expr,$f:ident,$v:expr) => {{
        // SAFETY: exclusive serialized native row; publish only the field changed by C.
        unsafe { ptr::addr_of_mut!((*$p).$f).write($v) }
    }};
}
/// Live initialized serialized translator; no source/resource reload during call.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_postprocess_list(
    owner: *const Translator,
    pause: i32,
    start: bool,
) -> i32 {
    if owner.is_null() {
        return libc::EINVAL;
    }
    let mut lease = match Lease::enter() {
        Ok(l) => l,
        Err(e) => return e,
    };
    let scratch = lease.scratch();
    // SAFETY: initialized owner/settings/global scalars copied before effects.
    let (
        (table, regression, reduction, stress_flags, vowel_pause, word_gap),
        count,
        gap,
        features,
        v,
        replacements_count,
    ) = unsafe {
        (
            (*owner).postprocess_list(),
            n_ph_list2,
            option_wordgap,
            espeak_rs_postprocess_features,
            voice,
            n_replace_phonemes,
        )
    };
    // SAFETY: native stable count; legacy clears it before invalid admission.
    unsafe {
        n_phoneme_list = 0;
    }
    let restore = || {
        // SAFETY: serialized resident table primitive, no loans retained.
        unsafe {
            SelectPhonemeTable(table);
        }
    };
    if !(1..=1000).contains(&count) {
        restore();
        return 0;
    }
    let mut count = count as usize;
    let source_length = (count + 2).min(1000);
    // SAFETY: admitted initialized first-stage rows; disjoint retained scratch.
    unsafe {
        ptr::copy_nonoverlapping(
            ptr::addr_of!(ph_list2).cast::<Source>(),
            scratch.source.as_mut_ptr(),
            source_length,
        );
    }
    let replacements_count = replacements_count.max(0) as usize;
    if replacements_count > 60 {
        restore();
        return libc::EINVAL;
    }
    let mut replacements = [Replacement::default(); 60];
    // SAFETY: bounded initialized replacement prefix copied before callbacks.
    unsafe {
        ptr::copy_nonoverlapping(
            ptr::addr_of!(replace_phonemes).cast::<Replacement>(),
            replacements.as_mut_ptr(),
            replacements_count,
        );
    }
    let mut program_length = 0;
    // SAFETY: serialized resident program owner returns immutable extent.
    let programs = unsafe { RustPhonemePrograms(&mut program_length) };
    if programs.is_null() || program_length > isize::MAX as usize {
        restore();
        return libc::EINVAL;
    }
    // SAFETY: admitted immutable program bytes, retained throughout table callbacks.
    let programs = unsafe { slice::from_raw_parts(programs, program_length) };
    // SAFETY: live native voice only when feature is enabled; initialized klatt slot.
    let klatt = features[0] != 0 && !v.is_null() && unsafe { (*v).klatt[0] != 0 };
    // SAFETY: initialized retained backend-name scalar.
    let mbrola = features[1] != 0
        && unsafe {
            !espeak_rs_postprocess_mbrola.is_null() && espeak_rs_postprocess_mbrola.read() != 0
        };
    let options = phoneme_list::Options {
        table,
        regression,
        reduction,
        stress_flags,
        vowel_pause,
        word_gap,
        option_wordgap: gap,
        klatt,
        mbrola,
        post_pause: pause,
        start_sentence: start,
        replacements: &replacements[..replacements_count],
        programs,
    };
    let produced = match phoneme_list::make_phoneme_list(
        &mut scratch.source[..source_length],
        &mut count,
        &mut scratch.output,
        &options,
        &mut ListHost,
    ) {
        Ok(n) => n,
        Err(_) => {
            restore();
            return libc::EINVAL;
        }
    };
    // SAFETY: exclusive serialized global source/count; callback loans have ended.
    unsafe {
        ptr::copy_nonoverlapping(
            scratch.source.as_ptr(),
            ptr::addr_of_mut!(ph_list2).cast::<Source>(),
            source_length,
        );
        n_ph_list2 = count as i32;
    }
    for (ix, o) in scratch.output[..produced].iter().enumerate() {
        // SAFETY: bounded initialized output slot, no reference to it crosses table selection.
        let p = unsafe { ptr::addr_of_mut!(phoneme_list).cast::<Entry>().add(ix) };
        // SAFETY: native table selector followed by validated output slot projection.
        let ph = unsafe {
            SelectPhonemeTable(o.table);
            phoneme_tab[o.slot as usize]
        };
        write!(p, phoneme, ph);
        write!(p, code, o.code);
        write!(p, kind, o.kind);
        write!(p, length, o.length);
        write!(p, source, o.source);
        write!(p, flags, o.synthflags);
        write!(p, new_word, o.new_word);
        write!(p, prepause, o.prepause);
        if ix + 2 >= produced {
            continue;
        }
        write!(p, env, o.env);
        write!(p, stress, o.stress);
        write!(p, word_stress, o.word_stress);
        write!(p, tone, o.tone);
        write!(p, amp, o.amp);
        write!(p, pitch1, o.pitch1);
        write!(p, pitch2, o.pitch2);
        let tone = if o.tone_table >= 0 {
            // SAFETY: retained resident table and validated tone slot.
            unsafe {
                SelectPhonemeTable(o.tone_table);
                phoneme_tab[o.tone as usize]
            }
        } else {
            ptr::null_mut()
        };
        write!(p, tone_data, tone);
    }
    // SAFETY: complete output prefix published before count.
    unsafe {
        n_phoneme_list = produced as i32;
    }
    restore();
    0
}
#[no_mangle]
unsafe extern "C" fn MakePhonemeList(owner: *const Translator, pause: i32, start: bool) {
    // SAFETY: legacy serialized initialized native owner.
    let _ = unsafe { espeak_rs_postprocess_list(owner, pause, start) };
}
/// Initialized list/table/tune owner views stay live and disjoint through publication.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_postprocess_pitch(owner: *const Translator, clause: i32) -> i32 {
    if owner.is_null() {
        return libc::EINVAL;
    }
    let mut lease = match Lease::enter() {
        Ok(l) => l,
        Err(e) => return e,
    };
    // SAFETY: initialized owner scalars copied before processing.
    let n = unsafe { n_phoneme_list };
    if n <= 0 || n > 1001 {
        return 0;
    }
    // SAFETY: admitted live initialized owner and resident metadata snapshots.
    let (settings, table_count, tune_count, tune_pointer) = unsafe {
        (
            (*owner).postprocess_pitch(option_tone_flags as u32),
            n_phoneme_tab,
            n_tunes,
            tunes,
        )
    };
    if !(0..=256).contains(&table_count) || (!tune_pointer.is_null() && tune_count < 0) {
        return libc::EINVAL;
    }
    let Some(tune_bytes) = (if tune_pointer.is_null() {
        0
    } else {
        tune_count as usize
    })
    .checked_mul(intonation::TUNE_SIZE)
    .filter(|n| *n <= isize::MAX as usize) else {
        return libc::EINVAL;
    };
    // SAFETY: initialized stable resident table slots, copied records before calculations.
    let Some(table) = (unsafe {
        super::borrowed_phonemes(ptr::addr_of!(phoneme_tab).cast(), table_count as usize)
    }) else {
        return libc::EINVAL;
    };
    let scratch = lease.scratch();
    for ix in 0..n as usize {
        // SAFETY: bounded native row pointer.
        let p = unsafe { ptr::addr_of!(phoneme_list).cast::<Entry>().add(ix) };
        let ph = read!(p, phoneme);
        let tone = read!(p, tone_data);
        scratch.pitch[ix] = intonation::Entry {
            synthflags: read!(p, flags),
            kind: read!(p, kind),
            code: if ph.is_null() { 0 } else { read!(ph, code) },
            std_length: if ph.is_null() {
                0
            } else {
                read!(ph, standard_length)
            },
            new_word: read!(p, new_word),
            stress: read!(p, stress),
            tone: read!(p, tone),
            env: read!(p, env),
            pitch1: read!(p, pitch1),
            pitch2: read!(p, pitch2),
            tone_shape: u8::from(!tone.is_null()),
            tone_start: if tone.is_null() {
                0
            } else {
                read!(tone, start_type)
            },
            tone_end: if tone.is_null() {
                0
            } else {
                read!(tone, end_type)
            },
        };
    }
    let tune_view = if tune_bytes == 0 {
        &[]
    } else {
        // SAFETY: admitted initialized immutable tune extent.
        unsafe { slice::from_raw_parts(tune_pointer, tune_bytes) }
    };
    if intonation::calc_pitches(
        &mut scratch.pitch[..n as usize],
        &table[..table_count as usize],
        intonation::Tunes::new(tune_view),
        &settings,
        clause,
    )
    .is_err()
    {
        return libc::EINVAL;
    }
    for (ix, e) in scratch.pitch[..n as usize].iter().enumerate() {
        // SAFETY: bounded serialized output row; publish only five changed fields.
        let p = unsafe { ptr::addr_of_mut!(phoneme_list).cast::<Entry>().add(ix) };
        write!(p, stress, e.stress);
        write!(p, tone, e.tone);
        write!(p, env, e.env);
        write!(p, pitch1, e.pitch1);
        write!(p, pitch2, e.pitch2);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn CalcPitches(owner: *const Translator, clause: i32) {
    // SAFETY: legacy serialized initialized engine views.
    let _ = unsafe { espeak_rs_postprocess_pitch(owner, clause) };
}
struct LengthHost {
    embedded: i32,
}
impl lengths::Host for LengthHost {
    fn embedded(&mut self) -> Result<[i32; 3], lengths::Error> {
        let d = Embedded {
            words: ptr::addr_of!(embedded_list).cast(),
            length: 250,
            cursor: &mut self.embedded,
            speeds: ptr::addr_of!(LEN_SPEEDS).cast(),
            set: Some(SetEmbedded),
            speed: Some(SetSpeed),
        };
        let mut out = [0; 3];
        // SAFETY: initialized raw global views, private cursor/output; callbacks retain no loans.
        if unsafe { parameter_control_compat::espeak_rs_length_embedded(&d, out.as_mut_ptr()) } != 0
        {
            return Err(lengths::Error::Host);
        }
        Ok(out)
    }
    fn tone_envelope(&mut self, index: usize) -> Result<u8, lengths::Error> {
        if index >= 1001 {
            return Err(lengths::Error::Host);
        }
        // SAFETY: bounded serialized native row; primitive resolves current resident tone.
        let p = unsafe { ptr::addr_of!(phoneme_list).cast::<Entry>().add(index) };
        // SAFETY: no scratch/global references cross synchronous primitive effects.
        let tone = unsafe { TonePhoneme(p) };
        if tone.is_null() {
            return Err(lengths::Error::Host);
        }
        let mut data = PhonemeData::default();
        // SAFETY: retained tone record and exclusive initialized local metadata.
        let code = i32::from(read!(p, tone));
        // SAFETY: retained tone record and exclusive initialized local metadata.
        unsafe {
            InterpretPhoneme2WithData(code, tone, &mut data);
        }
        // SAFETY: native envelope primitive returns a retained initialized 128-byte view.
        let envelope = unsafe { GetEnvelope(data.pitch_envelope) };
        if envelope.is_null() {
            return Err(lengths::Error::Host);
        }
        // SAFETY: read only the first initialized byte, no loan retained.
        Ok(unsafe { envelope.read() })
    }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_postprocess_lengths(owner: *const Translator) -> i32 {
    if owner.is_null() {
        return libc::EINVAL;
    }
    let mut lease = match Lease::enter() {
        Ok(l) => l,
        Err(e) => return e,
    };
    // SAFETY: initialized serialized list count, checked before reading unused settings.
    let n = unsafe { n_phoneme_list };
    if n <= 0 || n > 1001 {
        return 0;
    }
    // SAFETY: live initialized owner and retained modifier tables, copied before callbacks.
    let settings = unsafe { (*owner).postprocess_lengths(LEN_SPEEDS) };
    let Some(settings) = settings else {
        return libc::EINVAL;
    };
    let scratch = lease.scratch();
    let mut m = 0;
    while m < 1001 {
        // SAFETY: bounded native list row, read only fields used by the original host.
        let p = unsafe { ptr::addr_of!(phoneme_list).cast::<Entry>().add(m) };
        let ph = read!(p, phoneme);
        if m >= n as usize && ph.is_null() {
            break;
        }
        let tone = if read!(p, tone) != 0 {
            // SAFETY: synchronous serialized resident tone lookup; no row reference retained.
            unsafe { TonePhoneme(p) }
        } else {
            ptr::null_mut()
        };
        scratch.lengths[m] = lengths::Entry {
            length: read!(p, length),
            phflags: if ph.is_null() { 0 } else { read!(ph, flags) },
            mnemonic: if ph.is_null() { 0 } else { read!(ph, mnemonic) },
            synthflags: read!(p, flags),
            kind: read!(p, kind),
            stress: read!(p, stress),
            new_word: read!(p, new_word),
            prepause: read!(p, prepause),
            amp: read!(p, amp),
            pitch1: read!(p, pitch1),
            pitch2: read!(p, pitch2),
            env: read!(p, env),
            tone: read!(p, tone),
            code: if ph.is_null() { 0 } else { read!(ph, code) },
            length_mod: if ph.is_null() {
                0
            } else {
                read!(ph, length_modifier)
            },
            std_length: if ph.is_null() {
                0
            } else {
                read!(ph, standard_length)
            },
            tone_known: u8::from(!tone.is_null()),
            tone_length: if tone.is_null() {
                0
            } else {
                read!(tone, standard_length)
            },
        };
        m += 1;
    }
    let bad = match lengths::calc_lengths(
        &mut scratch.lengths[..m],
        n as usize,
        &settings,
        &mut scratch.more,
        &mut LengthHost { embedded: 0 },
    ) {
        Ok(b) => b,
        Err(_) => return libc::EINVAL,
    };
    for _ in 0..bad {
        eprintln!("espeak: Bad intonation data");
    }
    for (ix, e) in scratch.lengths[..m].iter().enumerate() {
        // SAFETY: bounded native output row; only changed fields publish after callbacks.
        let p = unsafe { ptr::addr_of_mut!(phoneme_list).cast::<Entry>().add(ix) };
        write!(p, length, e.length);
        write!(p, flags, e.synthflags);
        write!(p, prepause, e.prepause);
        write!(p, amp, e.amp);
        write!(p, pitch1, e.pitch1);
        write!(p, pitch2, e.pitch2);
        write!(p, env, e.env);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn CalcLengths(owner: *const Translator) {
    // SAFETY: legacy serialized initialized engine views.
    let _ = unsafe { espeak_rs_postprocess_lengths(owner) };
}
#[no_mangle]
unsafe extern "C" fn SetSpeed(control: i32) {
    // SAFETY: serialized initialized owner/global snapshots; no callbacks during configuration.
    let (v, mut state, primary, secondary, sonic) = unsafe {
        (
            voice,
            speed::State {
                factors: speed,
                lengths: LEN_SPEEDS,
            },
            embedded_value[2],
            embedded_value[8],
            espeak_rs_postprocess_features[2] != 0,
        )
    };
    if v.is_null() {
        eprintln!("Invalid speech rate arithmetic");
        return;
    }
    // SAFETY: live initialized voice borrowed only through this pure bounded calculation.
    let Ok(effects) = state.configure(unsafe { &*v }, primary, secondary, control as u32, sonic)
    else {
        eprintln!("Invalid speech rate arithmetic");
        return;
    };
    // SAFETY: serialized initialized owner fields publish before Sonic callbacks.
    unsafe {
        speed = state.factors;
        LEN_SPEEDS = state.lengths;
    }
    if sonic {
        for &value in &effects.values[..effects.count as usize] {
            // SAFETY: ordered bounded primitive effects; no voice/state references survive.
            unsafe {
                espeak_rs_queue_sonic_speed(ptr::addr_of_mut!(espeak_rs_wave_memory), value);
            }
        }
    }
}
#[no_mangle]
unsafe extern "C" fn SetParameter(parameter: i32, value: i32, relative: i32) -> i32 {
    // SAFETY: native stable initialized parameter/global views; descriptor retains no Rust references.
    let d = unsafe {
        Parameter {
            defaults: ptr::addr_of!(param_defaults).cast(),
            current: ptr::addr_of_mut!(param_stack[0].values).cast(),
            saved: ptr::addr_of_mut!(saved_parameters).cast(),
            embedded: ptr::addr_of_mut!(embedded_value).cast(),
            line: ptr::addr_of_mut!(option_linelength),
            gap: ptr::addr_of_mut!(option_wordgap),
            tone: ptr::addr_of_mut!(option_tone_flags),
            group: Translator::postprocess_group(translator.cast()),
            speed: Some(SetSpeed),
            amplitude: Some(GetAmplitude),
        }
    };
    // SAFETY: serialized admitted descriptor; callbacks retain no parameter/owner loans.
    unsafe { parameter_control_compat::espeak_rs_set_parameter(&d, parameter, value, relative) }
}
#[no_mangle]
unsafe extern "C" fn SetLengthMods(owner: *mut Translator, value: i32) {
    // SAFETY: legacy serialized initialized translator; native immutable tables.
    unsafe {
        Translator::postprocess_modifiers(owner, value);
    }
}
