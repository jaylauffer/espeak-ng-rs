//! Complete native synthesis host: retained command/generator state, ordered
//! effects and clause admission. The engine remains process-wide and serialized.
//! No Rust loan to a foreign row, queue or voice spans a host/user callback.
// Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case, non_upper_case_globals)]
use super::clause_postprocess_compat::{Entry, EMPTY};
use super::{commands_compat as command, generate_compat as generator};
use crate::{
    commands, formant, generate, output::FramePool, phoneme::Phoneme, phoneme_program::PhonemeData,
    speed, translator::Translator, voice::Voice, wave_memory::WaveMemory,
};
use std::{
    ffi::{c_char, c_void},
    mem::size_of,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

type PhonemeCallback = unsafe extern "C" fn(*const c_char) -> i32;
type MbrolaGenerate = unsafe extern "C" fn(*mut Entry, *mut i32, bool) -> i32;
#[repr(C)]
struct SoundIcon {
    name: i32,
    samples: i32,
    data: *const u8,
    filename: *const u8,
}
extern "C" {
    static mut samplerate: i32;
    static mut speed: speed::Factors;
    static mut voice: *mut Voice;
    static mut translator: *mut c_void;
    static mut wavefile_data: *const u8;
    static mut seq_len_adjust: i32;
    static mut formant_rate: [i32; 6];
    static mut soundicon_tab: [SoundIcon; 80];
    static mut n_soundicon_tab: i32;
    static mut embedded_list: [u32; 250];
    static mut clause_start_char: i32;
    static mut clause_start_word: i32;
    static mut count_sentences: i32;
    static mut count_characters: i32;
    static mut option_phoneme_events: i32;
    static mut option_phonemes: i32;
    static mut output_hooks: *const c_void;
    static mut p_decoder: *mut c_void;
    static mut skipping_text: bool;
    static mut f_trans: *mut libc::FILE;
    static mut phoneme_list: [Entry; 1001];
    static mut n_phoneme_list: i32;
    static mut espeak_rs_wave_memory: WaveMemory;
    static mut espeak_rs_frame_pool: FramePool;
    static espeak_rs_synthesis_mbrola: Option<MbrolaGenerate>;
    static espeak_rs_postprocess_mbrola: *const u8;
    fn espeak_rs_frame_pool_storage(
        pool: *mut c_void,
        kind: u32,
        frame: *mut c_void,
    ) -> *mut c_void;
    fn espeak_rs_queue_marker(
        memory: *mut WaveMemory,
        kind: i32,
        position: i32,
        length: i32,
        value: i32,
    ) -> i32;
    fn espeak_rs_queue_phoneme_marker(
        memory: *mut WaveMemory,
        kind: i32,
        position: i32,
        length: i32,
        name: *const c_char,
    ) -> i32;
    fn espeak_rs_queue_phoneme_alignment(memory: *mut WaveMemory, name: *mut c_char, kind: i32);
    fn espeak_rs_queue_sonic_speed(memory: *mut WaveMemory, speed: i32);
    fn espeak_rs_queue_voice(memory: *mut WaveMemory, voice: *mut u8);
    fn WcmdqFree() -> i32;
    fn WcmdqStop();
    fn RustPhonemeDataLength() -> usize;
    fn LookupSpect(
        phoneme: *mut Phoneme,
        which: i32,
        fmt: *mut generate::FmtParams,
        count: *mut i32,
        entry: *mut Entry,
    ) -> *mut commands::FrameRef;
    fn SetEmbedded(command: i32, value: i32);
    fn SetSpeed(control: i32);
    fn WritePhMnemonicWithStressBounded(
        output: *mut c_char,
        capacity: usize,
        phoneme: *mut Phoneme,
        entry: *mut Entry,
        ipa: i32,
        flags: *mut i32,
    ) -> *mut c_char;
    fn GetEnvelope(index: i32) -> *const u8;
    fn InterpretPhonemeWithLength(
        tr: *mut c_void,
        control: i32,
        entry: *mut Entry,
        list: *mut Entry,
        data: *mut PhonemeData,
        worddata: *mut Entry,
        count: usize,
    );
    fn TonePhoneme(entry: *const Entry) -> *mut Phoneme;
    fn InterpretPhoneme2WithData(code: i32, phoneme: *mut Phoneme, data: *mut PhonemeData);
    fn text_decoder_eof(decoder: *mut c_void) -> i32;
    fn SelectPhonemeTable(index: i32);
    fn TranslateClause(tr: *mut c_void, tone: *mut i32, voice_change: *mut *mut c_char);
    fn CalcPitches(tr: *mut c_void, tone: i32);
    fn CalcLengths(tr: *mut c_void);
    fn GetTranslatedPhonemeString(options: i32) -> *const c_char;
    fn LoadVoiceVariant(name: *const c_char, variant: i32) -> *mut Voice;
}

// Stable scratch is reused for every clause, including queue suspensions.
static mut COMMAND: commands::State = commands::State {
    last_frame: 0,
    last_pitch_cmd: 0,
    last_amp_cmd: 0,
    last_wcmdq: 0,
    pitch_length: 0,
    amp_length: 0,
    syllable_start: 0,
    syllable_end: 0,
    syllable_centre: 0,
    fmt_amplitude: 0,
    wave_flag: 0,
};
static mut GENERATE: generate::State = generate::State {
    ix: 0,
    embedded_ix: 0,
    word_count: 0,
    source: 0,
};
const EMPTY_GENERATE: generate::Entry = generate::Entry {
    phoneme: Phoneme {
        mnemonic: 0,
        flags: 0,
        program: 0,
        code: 0,
        kind: 0,
        start_type: 0,
        end_type: 0,
        standard_length: 0,
        length_modifier: 0,
    },
    length: 0,
    synthflags: 0,
    source: 0,
    kind: 0,
    new_word: 0,
    prepause: 0,
    amp: 0,
    env: 0,
    pitch1: 0,
    pitch2: 0,
    stress: 0,
    tone: 0,
    present: 0,
};
static mut SNAPSHOT: [generate::Entry; 1001] = [EMPTY_GENERATE; 1001];
static mut WORD: Entry = EMPTY;
static mut COUNT: usize = 0;
static mut MODULATION: i32 = 0;
static mut LOOKUP: *mut command::CommandLookup = ptr::null_mut();
static mut PHONEME_CALLBACK: Option<PhonemeCallback> = None;
static COMMAND_BUSY: AtomicBool = AtomicBool::new(false);
static GENERATE_BUSY: AtomicBool = AtomicBool::new(false);
static SPEAK_BUSY: AtomicBool = AtomicBool::new(false);
struct Lease(&'static AtomicBool);
impl Lease {
    fn acquire(flag: &'static AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self(flag))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Copy scalars before effects; the temporary translator loan ends here.
unsafe fn settings() -> command::CommandSettings {
    // SAFETY: serialized engine retains initialized settings and data owners;
    // only used fields are read, and no reference escapes this snapshot.
    unsafe {
        let factors = ptr::addr_of!(speed).read();
        let (long_vowel_threshold, sonorant_min, _) = if translator.is_null() {
            (0, 0, 0)
        } else {
            (&*translator.cast::<Translator>()).synthesis_options()
        };
        command::CommandSettings {
            settings: commands::Settings {
                samplerate,
                pause_factor: factors.pause,
                clause_pause_factor: factors.clause_pause,
                min_pause: factors.minimum_pause,
                wav_factor: factors.wave,
                lenmod_factor: factors.length_modifier,
                lenmod2_factor: factors.length_modifier2,
                min_sample_len: factors.minimum_sample,
                klatt: i32::from(!voice.is_null() && ptr::addr_of!((*voice).klatt[0]).read() != 0),
                long_vowel_threshold,
                sonorant_min,
                fall_envelope: super::envelope_data.0[0].as_ptr() as usize,
            },
            wave: wavefile_data,
            wave_length: if wavefile_data.is_null() {
                0
            } else {
                RustPhonemeDataLength()
            },
            queue: ptr::addr_of_mut!(espeak_rs_wave_memory),
        }
    }
}
struct Spect {
    phoneme: *mut Phoneme,
    entry: *mut Entry,
}

unsafe extern "C" fn command_effect(context: *mut c_void, effect: *mut command::Effect) -> i32 {
    // SAFETY: private ordered callback; effect/local outputs are exclusive.
    // COMMAND is borrowed by the writer and is deliberately never touched here.
    // Queue/frame loans end before lookup, speed or formant callbacks.
    unsafe {
        let e = &mut *effect;
        match e.op {
            4 => {
                let mut start = e.a;
                super::espeak_rs_smooth_spectrum(
                    ptr::addr_of_mut!(espeak_rs_wave_memory.queue).cast(),
                    170,
                    &mut start,
                    e.b,
                    e.c,
                    ptr::addr_of!(formant_rate),
                    ptr::addr_of_mut!(espeak_rs_frame_pool).cast(),
                    Some(espeak_rs_frame_pool_storage),
                );
                return start;
            }
            5 => {
                let spect = &*context.cast::<Spect>();
                e.lookup.write(command::CommandLookup::default());
                LOOKUP = e.lookup;
                MODULATION = 0;
                let mut count = 0;
                let frames = LookupSpect(spect.phoneme, e.a, e.fmt, &mut count, spect.entry);
                LOOKUP = ptr::null_mut();
                (*e.lookup).modulation = MODULATION;
                if !frames.is_null() {
                    (*e.lookup).found = 1;
                    (*e.lookup).count = if (0..=25).contains(&count) { count } else { 26 };
                    if (0..=25).contains(&count) {
                        ptr::copy_nonoverlapping(frames, e.frames, count as usize);
                    }
                }
            }
            6 => {
                let frame = e.value as *const formant::Frame;
                e.a = if frame.is_null() {
                    0
                } else {
                    i32::from(ptr::addr_of!((*frame).length).read())
                };
                e.b = if frame.is_null() {
                    0
                } else {
                    i32::from(ptr::addr_of!((*frame).flags).read())
                };
            }
            7 => {
                let high = e.words[0] as *const formant::Frame;
                let frame = super::espeak_rs_frame_copy(
                    e.value as *mut c_void,
                    1,
                    ptr::addr_of_mut!(espeak_rs_frame_pool).cast(),
                    Some(espeak_rs_frame_pool_storage),
                )
                .cast::<formant::Frame>();
                if !frame.is_null() && !high.is_null() {
                    for ix in 3..8 {
                        if ix < 7 {
                            ptr::addr_of_mut!((*frame).frequencies[ix])
                                .write(ptr::addr_of!((*high).frequencies[ix]).read());
                        }
                        ptr::addr_of_mut!((*frame).heights[ix])
                            .write(ptr::addr_of!((*high).heights[ix]).read());
                    }
                }
                e.value = frame as isize;
            }
            8 => seq_len_adjust = 0,
            9 => {
                SetEmbedded(e.a, e.value as i32);
                SetSpeed(2);
                e.settings.write(settings().settings);
                return 1;
            }
            10 => {
                let index = e.value as u32 as usize;
                let count = n_soundicon_tab;
                if !(0..=80).contains(&count) || index >= count as usize {
                    return 0;
                }
                let icon = ptr::addr_of!(soundicon_tab).cast::<SoundIcon>().add(index);
                e.a = ptr::addr_of!((*icon).samples).read();
                if e.a != 0 {
                    let data = ptr::addr_of!((*icon).data).read();
                    if data.is_null() {
                        return 0;
                    }
                    e.words[0] = data.add(44) as isize;
                }
                return 1;
            }
            _ => {}
        }
    }
    0
}

/// Private command admission rejects reentry immediately, without waiting.
unsafe fn command<R>(
    invalid: R,
    body: impl FnOnce(*mut commands::State, &command::CommandSettings) -> R,
) -> R {
    let Some(_lease) = Lease::acquire(&COMMAND_BUSY) else {
        return invalid;
    };
    // SAFETY: serialized admitted writer; its callbacks never access COMMAND.
    unsafe { body(ptr::addr_of_mut!(COMMAND), &settings()) }
}
unsafe fn end_amplitude() {
    // SAFETY: forwarded serialized command contract.
    unsafe {
        command((), |state, s| {
            command::espeak_rs_command_end_amplitude(
                state,
                s,
                ptr::null_mut(),
                Some(command_effect),
            )
        });
    }
}
unsafe fn end_pitch(voice_break: i32) {
    // SAFETY: forwarded serialized command contract.
    unsafe {
        command((), |state, s| {
            command::espeak_rs_command_end_pitch(
                state,
                s,
                ptr::null_mut(),
                Some(command_effect),
                voice_break,
            )
        });
    }
}
unsafe fn pause(length: i32, control: i32) {
    // SAFETY: forwarded serialized command contract.
    unsafe {
        command((), |state, s| {
            command::espeak_rs_command_pause(
                state,
                s,
                ptr::null_mut(),
                Some(command_effect),
                length,
                control,
            )
        });
    }
}
#[no_mangle]
unsafe extern "C" fn SynthesizeInit() {
    let Some(_lease) = Lease::acquire(&COMMAND_BUSY) else {
        return;
    };
    // SAFETY: serialized initialization, preserving the historical partial reset.
    unsafe {
        COMMAND.last_pitch_cmd = 0;
        COMMAND.last_amp_cmd = 0;
        COMMAND.last_frame = 0;
        COMMAND.syllable_centre = -1;
    }
}
#[no_mangle]
unsafe extern "C" fn WordToString(output: *mut c_char, word: u32) -> *const c_char {
    if output.is_null() {
        return ptr::null();
    }
    // SAFETY: ABI caller provides five writable bytes; no row/pointer loans.
    unsafe {
        ptr::copy_nonoverlapping(word.to_le_bytes().as_ptr(), output.cast(), 4);
        output.add(4).write(0);
    }
    output
}
#[no_mangle]
unsafe extern "C" fn PauseLength(length: i32, control: i32) -> i32 {
    // SAFETY: serialized scalar snapshot, no command state mutation.
    command::espeak_rs_pause_length(&unsafe { settings() }, length, control)
}
#[no_mangle]
unsafe extern "C" fn DoSample3(data: *mut PhonemeData, length: i32, amplitude: i32) -> i32 {
    // SAFETY: ABI caller retains the initialized phoneme data for this command.
    unsafe {
        command(0, |state, s| {
            command::espeak_rs_command_sample(
                state,
                s,
                ptr::null_mut(),
                Some(command_effect),
                data,
                length,
                amplitude,
            )
            .max(0)
        })
    }
}
#[no_mangle]
unsafe extern "C" fn DoSpect2(
    phoneme: *mut Phoneme,
    which: i32,
    fmt: *mut generate::FmtParams,
    entry: *mut Entry,
    modulation: i32,
) -> i32 {
    if phoneme.is_null() || entry.is_null() {
        return 0;
    }
    // SAFETY: caller retains initialized used fields and the previous row for
    // vowel starts; only copies survive lookup/transition callbacks.
    unsafe {
        let ph = commands::SpectPhoneme {
            kind: ptr::addr_of!((*phoneme).kind).read(),
            std_length: ptr::addr_of!((*phoneme).standard_length).read(),
            flags: ptr::addr_of!((*phoneme).flags).read(),
            synthflags: ptr::addr_of!((*entry).flags).read(),
            length: ptr::addr_of!((*entry).length).read(),
            prev_kind: if which == 1 {
                ptr::addr_of!((*entry.sub(1)).kind).read()
            } else {
                0
            },
        };
        let mut spect = Spect { phoneme, entry };
        command(0, |state, s| {
            command::espeak_rs_command_spect(
                state,
                s,
                ptr::addr_of_mut!(spect).cast(),
                Some(command_effect),
                &ph,
                which,
                fmt,
                modulation,
            )
            .max(0)
        })
    }
}
#[no_mangle]
unsafe extern "C" fn FormantTransitionWithCapacity(
    sequence: *mut super::SpectrumFrame,
    count: *mut i32,
    data1: u32,
    data2: u32,
    other: *mut Phoneme,
    which: i32,
    capacity: usize,
) -> i32 {
    // SAFETY: serialized retained sequence, count and frame storage. No command
    // state loan spans this transition; lookup pauses are deferred to the writer.
    unsafe {
        let settings = formant::Settings {
            which,
            klatt: u32::from(!voice.is_null() && ptr::addr_of!((*voice).klatt[0]).read() != 0),
            formant_factor: if voice.is_null() {
                256
            } else {
                ptr::addr_of!((*voice).formant_factor).read()
            },
            other_glottal: u32::from(
                !other.is_null() && ptr::addr_of!((*other).mnemonic).read() == u32::from(b'?'),
            ),
            length_adjust: seq_len_adjust,
        };
        let mut effects = formant::Effects::default();
        if super::espeak_rs_formant_transition(
            sequence,
            capacity,
            count,
            data1,
            data2,
            &settings,
            ptr::addr_of_mut!(espeak_rs_frame_pool).cast(),
            Some(espeak_rs_frame_pool_storage),
            &mut effects,
        ) != 0
        {
            return 0;
        }
        seq_len_adjust = effects.length_adjust;
        if effects.has_modulation != 0 {
            MODULATION = effects.modulation;
        }
        if effects.pause != 0 {
            if LOOKUP.is_null() {
                pause(effects.pause as i32, 0);
            } else if (0..4).contains(&(*LOOKUP).n_pauses) {
                (*LOOKUP).pauses[(*LOOKUP).n_pauses as usize] = effects.pause as i32;
                (*LOOKUP).n_pauses += 1;
            } else {
                (*LOOKUP).n_pauses = 5;
            }
        }
        effects.return_length
    }
}
#[no_mangle]
unsafe extern "C" fn FormantTransition2(
    sequence: *mut super::SpectrumFrame,
    count: *mut i32,
    data1: u32,
    data2: u32,
    other: *mut Phoneme,
    which: i32,
) -> i32 {
    // SAFETY: forwarded historical 25-entry sequence contract.
    unsafe { FormantTransitionWithCapacity(sequence, count, data1, data2, other, which, 25) }
}
#[no_mangle]
unsafe extern "C" fn DoMarker(kind: i32, position: i32, length: i32, value: i32) {
    // SAFETY: serialized queue mutation, no foreign references.
    unsafe {
        espeak_rs_queue_marker(
            ptr::addr_of_mut!(espeak_rs_wave_memory),
            kind,
            position,
            length,
            value,
        );
    }
}
#[no_mangle]
unsafe extern "C" fn DoPhonemeMarker(kind: i32, position: i32, length: i32, name: *const c_char) {
    // SAFETY: caller provides the historical eight readable marker bytes.
    unsafe {
        espeak_rs_queue_phoneme_marker(
            ptr::addr_of_mut!(espeak_rs_wave_memory),
            kind,
            position,
            length,
            name,
        );
    }
}
#[no_mangle]
unsafe extern "C" fn DoSonicSpeed(value: i32) {
    // SAFETY: serialized queue mutation.
    unsafe {
        espeak_rs_queue_sonic_speed(ptr::addr_of_mut!(espeak_rs_wave_memory), value);
    }
}
#[no_mangle]
unsafe extern "C" fn DoVoiceChange(source: *const Voice) -> i32 {
    if source.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: voice-control operation copies the foreign representation as
    // bytes, including padding. Queue consumption uses C free, so this rare
    // control allocation deliberately uses the matching C allocator.
    unsafe {
        let copy = libc::malloc(size_of::<Voice>()).cast::<u8>();
        if copy.is_null() {
            return libc::ENOMEM;
        }
        ptr::copy_nonoverlapping(source.cast::<u8>(), copy, size_of::<Voice>());
        espeak_rs_queue_voice(ptr::addr_of_mut!(espeak_rs_wave_memory), copy);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn DoEmbedded(index: *mut i32, source: i32) {
    // SAFETY: caller retains exclusive cursor and serialized embedded list.
    unsafe {
        command((), |state, s| {
            command::espeak_rs_command_embedded(
                state,
                s,
                ptr::null_mut(),
                Some(command_effect),
                ptr::addr_of!(embedded_list).cast(),
                250,
                index,
                source,
                clause_start_char,
                count_characters,
            );
        });
    }
}

unsafe fn envelope(e: &generator::Effect) -> *const u8 {
    // SAFETY: pure generator validates envelope table indices before effects;
    // program offsets use the admitted resident phoneme-data adapter.
    unsafe {
        match e.envelope {
            1 => super::envelope_data.0[e.envelope_value as usize].as_ptr(),
            2 => GetEnvelope(e.envelope_value),
            _ => ptr::null(),
        }
    }
}
unsafe extern "C" fn generate_effect(context: *mut c_void, effect: *mut generator::Effect) -> i32 {
    // SAFETY: private pure-generator callback validates row indices and retains
    // snapshot/state separately. Access only used foreign fields through raw
    // pointers; no row/COMMAND loan survives nested interpretation/commands.
    unsafe {
        let e = &mut *effect;
        let list = context.cast::<Entry>();
        let p = list.wrapping_add(e.index as usize);
        match e.op {
            0 => return WcmdqFree(),
            1 => {
                COMMAND.pitch_length = 0;
                COMMAND.amp_length = 0;
                COMMAND.last_frame = 0;
                COMMAND.last_wcmdq = -1;
                COMMAND.syllable_start = ptr::addr_of!(espeak_rs_wave_memory.tail).read();
                COMMAND.syllable_end = COMMAND.syllable_start;
                COMMAND.syllable_centre = -1;
                COMMAND.last_pitch_cmd = -1;
                ptr::addr_of_mut!(WORD).write(EMPTY);
            }
            2 => return i32::from(COMMAND.last_pitch_cmd >= 0),
            3 => {
                if !output_hooks.is_null()
                    && output_hooks
                        .cast::<Option<unsafe extern "C" fn(*mut c_char, i32)>>()
                        .read()
                        .is_some()
                {
                    let mut bytes = [0u8; 64];
                    let mut flags = 0;
                    if WritePhMnemonicWithStressBounded(
                        bytes.as_mut_ptr().cast(),
                        bytes.len(),
                        ptr::addr_of!((*p).phoneme).read(),
                        p,
                        0,
                        &mut flags,
                    )
                    .is_null()
                    {
                        return -1;
                    }
                    let length = bytes.iter().position(|&b| b == 0).unwrap_or(63) + 1;
                    let copy = libc::malloc(length).cast::<u8>();
                    if copy.is_null() {
                        return -1;
                    }
                    ptr::copy_nonoverlapping(bytes.as_ptr(), copy, length);
                    espeak_rs_queue_phoneme_alignment(
                        ptr::addr_of_mut!(espeak_rs_wave_memory),
                        copy.cast(),
                        i32::from(ptr::addr_of!((*p).kind).read()),
                    );
                }
            }
            4 => {
                let mut index = e.embedded_ix.read();
                DoEmbedded(&mut index, e.a);
                e.embedded_ix.write(index);
            }
            5 => COMMAND.last_frame = 0,
            6 => DoMarker(e.index, e.a, e.b, e.c),
            7 => {
                let mut bytes = [0u8; 64];
                if WritePhMnemonicWithStressBounded(
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    ptr::addr_of!((*p).phoneme).read(),
                    p,
                    e.a,
                    ptr::null_mut(),
                )
                .is_null()
                {
                    return -1;
                }
                DoPhonemeMarker(7, e.b, 0, bytes.as_ptr().cast());
            }
            8 => end_amplitude(),
            9 => end_pitch(e.a),
            10 => pause(e.a, e.b),
            11 => {
                let env = envelope(e);
                command((), |state, s| {
                    command::espeak_rs_command_amplitude(
                        state,
                        s,
                        ptr::null_mut(),
                        Some(command_effect),
                        e.a,
                        env,
                    )
                });
            }
            12 => {
                let env = envelope(e);
                command((), |state, s| {
                    command::espeak_rs_command_pitch(
                        state,
                        s,
                        ptr::null_mut(),
                        Some(command_effect),
                        env,
                        e.a,
                        e.b,
                    )
                });
            }
            13 => command((), |state, s| {
                command::espeak_rs_command_start_syllable(
                    state,
                    s,
                    ptr::null_mut(),
                    Some(command_effect),
                )
            }),
            14 => InterpretPhonemeWithLength(
                ptr::null_mut(),
                e.a,
                p,
                list,
                e.data,
                if e.b != 0 {
                    ptr::addr_of_mut!(WORD)
                } else {
                    ptr::null_mut()
                },
                COUNT,
            ),
            15 => {
                let tone = TonePhoneme(p);
                if tone.is_null() {
                    return -1;
                }
                InterpretPhoneme2WithData(i32::from(ptr::addr_of!((*p).tone).read()), tone, e.data);
            }
            16 => {
                DoSpect2(ptr::addr_of!((*p).phoneme).read(), e.a, e.fmt, p, e.b);
            }
            17 => {
                DoSample3(e.data, e.a, e.b);
            }
            18 => ptr::addr_of_mut!((*p).flags).write(e.a as u16),
            19 => ptr::addr_of_mut!((*p).std_length).write(e.a as u8),
            _ => {}
        }
    }
    0
}
#[no_mangle]
unsafe extern "C" fn Generate(list: *mut Entry, n: *mut i32, resume: bool) -> i32 {
    if list.is_null() || n.is_null() {
        return 0;
    }
    let Some(_lease) = Lease::acquire(&GENERATE_BUSY) else {
        return 1;
    };
    // SAFETY: serialized engine retains at most 1001 foreign rows and two
    // admitted neighbours. Copy initialized fields individually; neither unused
    // row tails nor ordinary phoneme-list padding are interpreted as Rust data.
    // The stable snapshot/state loans touch disjoint storage from every effect.
    unsafe {
        if let Some(mbrola) = espeak_rs_synthesis_mbrola {
            if !espeak_rs_postprocess_mbrola.is_null() && espeak_rs_postprocess_mbrola.read() != 0 {
                return mbrola(list, n, resume);
            }
        }
        let mut count = n.read().max(0) as usize;
        if count > 1000 || translator.is_null() {
            n.write(0);
            return 0;
        }
        let capacity = (count + 2).min(1001);
        for index in 0..capacity {
            let p = list.add(index);
            let phoneme = ptr::addr_of!((*p).phoneme).read();
            let value = generate::Entry {
                phoneme: if phoneme.is_null() {
                    EMPTY_GENERATE.phoneme
                } else {
                    phoneme.read()
                },
                present: u8::from(!phoneme.is_null()),
                length: ptr::addr_of!((*p).length).read(),
                synthflags: ptr::addr_of!((*p).flags).read(),
                source: ptr::addr_of!((*p).source).read(),
                kind: ptr::addr_of!((*p).kind).read(),
                new_word: ptr::addr_of!((*p).new_word).read(),
                prepause: ptr::addr_of!((*p).prepause).read(),
                amp: ptr::addr_of!((*p).amp).read(),
                env: ptr::addr_of!((*p).env).read(),
                pitch1: ptr::addr_of!((*p).pitch1).read(),
                pitch2: ptr::addr_of!((*p).pitch2).read(),
                stress: ptr::addr_of!((*p).stress).read(),
                tone: ptr::addr_of!((*p).tone).read(),
            };
            ptr::addr_of_mut!(SNAPSHOT)
                .cast::<generate::Entry>()
                .add(index)
                .write(value);
        }
        let (_, _, word_merge) = (&*translator.cast::<Translator>()).synthesis_options();
        let settings = generate::Settings {
            phoneme_events: option_phoneme_events,
            word_merge,
            clause_start_char,
            clause_start_word,
            count_sentences,
            count_characters,
        };
        COUNT = count;
        let status = generator::espeak_rs_generate(
            ptr::addr_of_mut!(SNAPSHOT).cast(),
            capacity,
            &mut count,
            u32::from(resume),
            ptr::addr_of_mut!(GENERATE),
            &settings,
            list.cast(),
            Some(generate_effect),
        );
        if status == 1 {
            return 1;
        }
        n.write(if status == 0 { count as i32 } else { 0 });
    }
    0
}
#[no_mangle]
unsafe extern "C" fn SpeakNextClause(control: i32) -> i32 {
    let Some(_lease) = Lease::acquire(&SPEAK_BUSY) else {
        return 0;
    };
    // SAFETY: serialized engine entry. Copy callback/scalars before invocation;
    // no borrowed voice/translator/phoneme output survives a callback. Historical
    // C stdio and queued allocation consumers remain foreign runtime primitives.
    unsafe {
        if control == 2 {
            n_phoneme_list = 0;
            WcmdqStop();
            return 0;
        }
        if p_decoder.is_null() || text_decoder_eof(p_decoder) != 0 {
            skipping_text = false;
            return 0;
        }
        if voice.is_null() || translator.is_null() {
            return 0;
        }
        SelectPhonemeTable(ptr::addr_of!((*voice).phoneme_table).read());
        let mut tone = 0;
        let mut change = ptr::null_mut();
        TranslateClause(translator, &mut tone, &mut change);
        CalcPitches(translator, tone);
        CalcLengths(translator);
        let callback = PHONEME_CALLBACK;
        if option_phonemes & 15 != 0 || callback.is_some() {
            let output = GetTranslatedPhonemeString(option_phonemes);
            if !output.is_null() {
                if option_phonemes & 15 != 0 && !f_trans.is_null() {
                    libc::fwrite(output.cast(), 1, libc::strlen(output), f_trans);
                    libc::fputc(i32::from(b'\n'), f_trans);
                }
                if let Some(callback) = callback {
                    callback(output);
                }
            }
        }
        if skipping_text {
            n_phoneme_list = 0;
            return 1;
        }
        Generate(
            ptr::addr_of_mut!(phoneme_list).cast(),
            ptr::addr_of_mut!(n_phoneme_list),
            false,
        );
        if !change.is_null() && !LoadVoiceVariant(change, 0).is_null() {
            DoVoiceChange(voice);
        }
    }
    1
}
#[no_mangle]
unsafe extern "C" fn espeak_SetPhonemeCallback(callback: Option<PhonemeCallback>) {
    // SAFETY: serialized registration; callback replacement during callback is
    // allowed because SpeakNextClause captures its function pointer by value.
    unsafe {
        PHONEME_CALLBACK = callback;
    }
}
