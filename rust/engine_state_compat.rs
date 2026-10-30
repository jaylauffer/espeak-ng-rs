//! The engine's mutable state, defined in Rust under the names the C bridges
//! and the Rust hosts link against. Until 2026-10-30 translate.c, voices.c,
//! speech.c, soundicon.c, readclause.c and dictionary.c defined these; the C
//! oracle build (-DUSE_RUST_CORE=OFF) still does.
//!
//! Each definition keeps the C type's size and, for arrays, the 16-byte
//! alignment the C compilers gave them ([`CArray`]). A shared library exports
//! none of them: cmake/libespeak-ng.map and cmake/libespeak-ng.exports limit its
//! exports to the public API.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]

use super::{CArray, ForeignVoice, WChar};
use std::ffi::{c_char, c_int, c_long, c_uint, c_void};
use std::ptr::{null, null_mut};

/// translate.h: N_WORD_PHONEMES, N_TR_SOURCE, N_MARKER_LENGTH, N_PUNCTLIST.
const WORD_PHONEMES: usize = 200;
const TR_SOURCE: usize = 800 + 40;
const MARKER_LENGTH: usize = 50;
const PUNCTLIST: usize = 60;
/// synthesize.h: N_EMBEDDED_LIST.
const EMBEDDED_LIST: usize = 250;
/// voices.c: N_VOICES_LIST.
const VOICES_LIST: usize = 500;
/// sizeof(voice_t), asserted by rust_wavegen.c.
const VOICE_SIZE: usize = 1344;
/// soundicon.h: N_SOUNDICON_TAB.
const SOUNDICON_TAB: usize = 80;

/// soundicon.h: SOUND_ICON.
#[repr(C)]
pub(super) struct SoundIconSlot {
    name: c_int,
    length: c_int,
    data: *mut c_char,
    filename: *mut c_char,
}

const NO_SOUND_ICON: SoundIconSlot = SoundIconSlot {
    name: 0,
    length: 0,
    data: null_mut(),
    filename: null_mut(),
};

// translate.c
#[no_mangle]
static mut translator: *mut c_void = null_mut();
#[no_mangle]
static mut translator2: *mut c_void = null_mut();
#[no_mangle]
static mut translator2_language: CArray<[c_char; 20]> = CArray([0; 20]);
#[no_mangle]
static mut translator3: *mut c_void = null_mut();
#[no_mangle]
static mut translator3_language: CArray<[c_char; 20]> = CArray([0; 20]);
#[no_mangle]
static mut f_trans: *mut c_void = null_mut();
#[no_mangle]
static mut option_tone_flags: c_int = 0;
#[no_mangle]
static mut option_phonemes: c_int = 0;
#[no_mangle]
static mut option_phoneme_events: c_int = 0;
#[no_mangle]
static mut option_endpause: c_int = 0;
#[no_mangle]
static mut option_capitals: c_int = 0;
#[no_mangle]
static mut option_punctuation: c_int = 0;
#[no_mangle]
static mut option_sayas: c_int = 0;
#[no_mangle]
static mut option_sayas2: c_int = 0;
#[no_mangle]
static mut option_emphasis: c_int = 0;
#[no_mangle]
static mut option_ssml: c_int = 0;
#[no_mangle]
static mut option_phoneme_input: c_int = 0;
#[no_mangle]
static mut option_wordgap: c_int = 0;
#[no_mangle]
static mut count_sayas_digits: c_int = 0;
#[no_mangle]
static mut skip_sentences: c_int = 0;
#[no_mangle]
static mut skip_words: c_int = 0;
#[no_mangle]
static mut skip_characters: c_int = 0;
#[no_mangle]
static mut skip_marker: CArray<[c_char; MARKER_LENGTH]> = CArray([0; MARKER_LENGTH]);
#[no_mangle]
static mut skipping_text: bool = false;
#[no_mangle]
static mut end_character_position: c_int = 0;
#[no_mangle]
static mut count_sentences: c_int = 0;
#[no_mangle]
static mut count_words: c_int = 0;
#[no_mangle]
static mut clause_start_char: c_int = 0;
#[no_mangle]
static mut clause_start_word: c_int = 0;
#[no_mangle]
static mut new_sentence: bool = false;
#[no_mangle]
static mut word_emphasis: c_int = 0;
#[no_mangle]
static mut embedded_flag: c_int = 0;
#[no_mangle]
static mut max_clause_pause: c_int = 0;
#[no_mangle]
static mut any_stressed_words: bool = false;
#[no_mangle]
static mut pre_pause: c_int = 0;
#[no_mangle]
static mut current_alphabet: *mut c_void = null_mut();
#[no_mangle]
static mut word_phonemes: CArray<[c_char; WORD_PHONEMES]> = CArray([0; WORD_PHONEMES]);
#[no_mangle]
static mut option_punctlist: CArray<[WChar; PUNCTLIST]> = CArray([0; PUNCTLIST]);
#[no_mangle]
static mut option_linelength: c_int = 0;
#[no_mangle]
static mut embedded_ix: c_int = 0;
#[no_mangle]
static mut embedded_read: c_int = 0;
#[no_mangle]
static mut embedded_list: CArray<[c_uint; EMBEDDED_LIST]> = CArray([0; EMBEDDED_LIST]);
/// The source text of one clause (UTF-8), with room for embedded commands and a
/// voice change at its end.
#[no_mangle]
static mut espeak_clause_source: CArray<[c_char; TR_SOURCE]> = CArray([0; TR_SOURCE]);

// voices.c
#[no_mangle]
static mut tone_points: CArray<[c_int; 12]> =
    CArray([600, 170, 1200, 135, 2000, 110, 3000, 110, -1, 0, 0, 0]);
/// Formant rate limits adjusted for the actual sample rate.
#[no_mangle]
static mut formant_rate: CArray<[c_int; 9]> = CArray([0; 9]);
#[no_mangle]
static mut n_voices_list: c_int = 0;
#[no_mangle]
static mut voices_list: CArray<[*mut c_void; VOICES_LIST]> = CArray([null_mut(); VOICES_LIST]);
#[no_mangle]
static mut rust_voice_workspace: *mut c_void = null_mut();
#[no_mangle]
static mut rust_voice_catalog: *mut c_void = null_mut();
#[no_mangle]
static mut rust_current_voice: *mut c_void = null_mut();
#[no_mangle]
static mut current_voice_selected: ForeignVoice = ForeignVoice {
    name: null(),
    languages: null(),
    identifier: null(),
    gender: 0,
    age: 0,
    variant: 0,
    variants: 0,
    score: 0,
    spare: null_mut(),
};
/// The current voice_t; `voice` points at it.
#[no_mangle]
static mut voicedata: CArray<[u8; VOICE_SIZE]> = CArray([0; VOICE_SIZE]);
#[no_mangle]
static mut voice: *mut c_void = (&raw mut voicedata).cast();

// speech.c
#[no_mangle]
static mut count_samples: c_long = 0;
#[no_mangle]
static mut my_unique_identifier: c_uint = 0;
#[no_mangle]
static mut my_user_data: *mut c_void = null_mut();
#[no_mangle]
static mut synth_callback: *mut c_void = null_mut();
/// The espeak-ng-data directory.
#[no_mangle]
static mut path_home: CArray<[c_char; crate::crt::PATH_BUFFER]> =
    CArray([0; crate::crt::PATH_BUFFER]);

// soundicon.c
#[no_mangle]
static mut n_soundicon_tab: c_int = 0;
#[no_mangle]
static mut soundicon_tab: CArray<[SoundIconSlot; SOUNDICON_TAB]> =
    CArray([NO_SOUND_ICON; SOUNDICON_TAB]);
#[no_mangle]
static mut rust_soundicons: *mut c_void = null_mut();

// readclause.c
#[no_mangle]
static mut namedata: *mut c_char = null_mut();
#[no_mangle]
static mut p_decoder: *mut c_void = null_mut();
#[no_mangle]
static mut count_characters: c_int = 0;

// dictionary.c
#[no_mangle]
static mut dictionary_skipwords: c_int = 0;
#[no_mangle]
static mut dictionary_name: CArray<[c_char; 40]> = CArray([0; 40]);
