//! The public API's engine callbacks against the engine (from speech.c): the
//! data-path search, the lifecycle and driver tables' primitives, and the
//! API calls that only set or read engine state.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]
use super::{
    clause_postprocess_compat as post, engine_c as c, parameter_control_compat as parameters,
    phoneme_render_compat::engine::espeak_rs_translated_phonemes_engine, WChar,
};
use crate::{crt, translator::Translator};
use std::{
    ffi::{c_char, c_int, c_long, c_uint, c_void},
    ptr::{self, addr_of, addr_of_mut},
};

type SynthCallback = unsafe extern "C" fn(*mut i16, c_int, *mut c_void) -> c_int;
type Synthesize = unsafe extern "C" fn(c_uint, *const c_void, c_int) -> c_int;

unsafe extern "C" {
    static mut my_unique_identifier: c_uint;
    static mut my_user_data: *mut c_void;
    static mut synth_callback: Option<SynthCallback>;
    static mut option_phoneme_events: c_int;
    static mut option_punctlist: [WChar; 60];
    static mut p_decoder: *mut c_void;
    fn utf8_in(code: *mut c_int, text: *const c_char) -> c_int;
    fn VoiceReset(tone: c_int);
    fn espeak_GetCurrentVoice() -> *mut super::ForeignVoice;
    fn espeak_ng_SetVoiceByName(name: *const c_char) -> c_int;
    fn Generate(list: *mut c_void, count: *mut c_int, resume: bool) -> c_int;
    fn TranslateClauseWithTerminator(
        tr: *mut c_void,
        tone: *mut c_int,
        voice_change: *mut *mut c_char,
        terminator: *mut c_int,
    );
}
#[cfg(windows)]
unsafe extern "system" {
    fn RegOpenKeyExA(
        key: isize,
        name: *const c_char,
        options: u32,
        access: u32,
        result: *mut isize,
    ) -> i32;
    fn RegQueryValueExA(
        key: isize,
        name: *const c_char,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        size: *mut u32,
    ) -> i32;
}

/// `path_home = parts`, truncated as `snprintf(path_home, N_PATH_BUF, ...)`.
fn set_home(parts: &[&[u8]]) {
    let mut path = parts.concat();
    path.truncate(crt::PATH_BUFFER - 1);
    path.push(0);
    // SAFETY: the engine's N_PATH_BUF-byte data path, serialized.
    unsafe {
        ptr::copy_nonoverlapping(
            path.as_ptr(),
            addr_of_mut!(c::path_home).cast::<u8>(),
            path.len(),
        )
    };
}
/// Whether `path_home` names a directory.
fn home_is_directory() -> bool {
    // SAFETY: the terminated data path.
    unsafe { super::espeak_rs_file_length(addr_of!(c::path_home)) == -libc::EISDIR }
}
/// `check_data_path`: `path/espeak-ng-data`, or `path` itself when
/// `directory` allows it, becomes the data path if it is a directory.
unsafe fn check_data_path(path: *const c_char, directory: bool) -> bool {
    if path.is_null() {
        return false;
    }
    // SAFETY: caller's terminated path, copied before path_home is written.
    let path = unsafe { c::text(path.cast()) }.to_vec();
    set_home(&[&path, b"/espeak-ng-data"]);
    if home_is_directory() {
        return true;
    }
    if !directory {
        return false;
    }
    set_home(&[&path]);
    home_is_directory()
}

/// `espeak_ng_InitializePath`: the data directory from `path`, the
/// environment (and on Windows the registry), else `fallback`.
///
/// # Safety
/// `path` is null or terminated; `fallback` is terminated; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_initialize_path(path: *const c_char, fallback: *const c_char) {
    // SAFETY: caller contract; getenv's strings are read before any change.
    unsafe {
        if check_data_path(path, true) {
            return;
        }
        if check_data_path(libc::getenv(c"ESPEAK_DATA_PATH".as_ptr()), true) {
            return;
        }
        #[cfg(windows)]
        {
            const LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
            const KEY_READ: u32 = 0x2_0019;
            let mut key: isize = 0;
            let mut value = [0u8; crt::PATH_BUFFER - 13];
            RegOpenKeyExA(
                LOCAL_MACHINE,
                c"Software\\eSpeak NG".as_ptr(),
                0,
                KEY_READ,
                &mut key,
            );
            if key == 0 {
                RegOpenKeyExA(
                    LOCAL_MACHINE,
                    c"Software\\WOW6432Node\\eSpeak NG".as_ptr(),
                    0,
                    KEY_READ,
                    &mut key,
                );
            }
            let mut size = value.len() as u32 - 1;
            let mut kind = 1; // REG_SZ
            RegQueryValueExA(
                key,
                c"Path".as_ptr(),
                ptr::null_mut(),
                &mut kind,
                value.as_mut_ptr(),
                &mut size,
            );
            if check_data_path(value.as_ptr().cast(), true) {
                return;
            }
        }
        #[cfg(not(windows))]
        if check_data_path(libc::getenv(c"HOME".as_ptr()), false) {
            return;
        }
        set_home(&[c::text(fallback.cast())]);
    }
}

// The lifecycle and driver tables' primitives.
#[no_mangle]
extern "C" fn espeak_rs_engine_noop() {}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_current_voice_clear() {
    // SAFETY: the engine's current-voice record, serialized.
    unsafe { ptr::write_bytes(espeak_GetCurrentVoice(), 0, 1) };
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_stack_reset() {
    // SAFETY: serialized engine.
    unsafe {
        super::readclause_engine_compat::espeak_rs_set_voice_stack(ptr::null(), c"".as_ptr())
    };
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_voice_reset() {
    // SAFETY: serialized engine.
    unsafe { VoiceReset(0) };
}
/// `time(NULL)`.
#[no_mangle]
extern "C" fn espeak_rs_engine_clock() -> c_long {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as c_long)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_driver_encoding(tr: *const Translator) -> c_uint {
    // SAFETY: the driver's live translator.
    unsafe { (*tr).encoding as c_uint }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_driver_default_voice() -> c_int {
    // SAFETY: serialized engine.
    unsafe { espeak_ng_SetVoiceByName(c"en".as_ptr()) }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_driver_generate() -> c_int {
    // SAFETY: the engine's phoneme list, serialized.
    unsafe {
        Generate(
            addr_of_mut!(post::phoneme_list).cast(),
            addr_of_mut!(post::n_phoneme_list),
            true,
        )
    }
}
/// The audio request's diagnostic, as `fprintf(stderr, "audio %s error: %s\n")`.
///
/// # Safety
/// Both strings are terminated.
#[no_mangle]
unsafe extern "C" fn espeak_rs_request_diagnostic(
    operation: *const c_char,
    message: *const c_char,
) {
    // SAFETY: caller contract.
    let (operation, message) = unsafe { (c::text(operation.cast()), c::text(message.cast())) };
    c::error(&[b"audio ", operation, b" error: ", message, b"\n"]);
}

/// `espeak_ng_SetPhonemeEvents`.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_set_phoneme_events(enable: c_int, ipa: c_int) -> c_int {
    // espeakINITIALIZE_PHONEME_EVENTS and espeakINITIALIZE_PHONEME_IPA.
    let value = match (enable != 0, ipa != 0) {
        (false, _) => 0,
        (true, false) => 1,
        (true, true) => 3,
    };
    // SAFETY: serialized engine option.
    unsafe { option_phoneme_events = value };
    0
}
/// `espeak_ng_GetSampleRate`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_engine_sample_rate() -> c_int {
    // SAFETY: serialized engine scalar.
    unsafe { super::wavegen_host_compat::samplerate }
}
/// `sync_espeak_Char`: one character, spoken as `tts:char`.
///
/// # Safety
/// Serialized engine; `synthesize` is speech.c's `Synthesize`.
#[no_mangle]
unsafe extern "C" fn espeak_rs_speak_char(character: WChar, synthesize: Synthesize) -> c_int {
    let text = format!(
        "<say-as interpret-as=\"tts:char\">&#{};</say-as>\0",
        character as c_int
    );
    // SAFETY: serialized engine state; the text outlives the call.
    unsafe {
        my_unique_identifier = 0;
        my_user_data = ptr::null_mut();
        synthesize(0, text.as_ptr().cast(), 0x10) // espeakSSML
    }
}
/// `sync_espeak_Key`: a single character by name, else the key as text.
///
/// # Safety
/// `key` is terminated; serialized engine; `synthesize` is speech.c's.
#[no_mangle]
unsafe extern "C" fn espeak_rs_speak_key(key: *const c_char, synthesize: Synthesize) -> c_int {
    let mut letter = 0;
    // SAFETY: caller contract.
    unsafe {
        let length = utf8_in(&mut letter, key);
        if *key.offset(length as isize) == 0 {
            return espeak_rs_speak_char(letter as WChar, synthesize);
        }
        my_unique_identifier = 0;
        my_user_data = ptr::null_mut();
        synthesize(0, key.cast(), 0)
    }
}
/// `sync_espeak_SetPunctuationList`.
///
/// # Safety
/// `list` is null or a terminated wide string; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_set_punctuation_list(list: *const WChar) {
    // SAFETY: serialized engine state and the caller's string.
    unsafe {
        my_unique_identifier = 0;
        my_user_data = ptr::null_mut();
        let punctuation = &mut *addr_of_mut!(option_punctlist);
        punctuation[0] = 0;
        if list.is_null() {
            return;
        }
        // wcsncpy: up to 60 characters, then zeros; the last is always zero.
        let mut ended = false;
        for (index, slot) in punctuation.iter_mut().enumerate() {
            if !ended {
                *slot = *list.add(index);
                ended = *slot == 0;
            } else {
                *slot = 0;
            }
        }
        punctuation[59] = 0;
    }
}
/// `espeak_SetSynthCallback`; `events` is the asynchronous event queue's
/// setter when the build has one.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_set_synth_callback(
    callback: Option<SynthCallback>,
    events: Option<unsafe extern "C" fn(Option<SynthCallback>)>,
) {
    // SAFETY: serialized engine slot and the queue's setter.
    unsafe {
        synth_callback = callback;
        if let Some(events) = events {
            events(callback);
        }
    }
}
/// `espeak_GetParameter`: the current value, or the default.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_get_parameter(parameter: c_int, current: c_int) -> c_int {
    let Ok(index) = usize::try_from(parameter) else {
        return 0;
    };
    // SAFETY: the engine's parameter tables, read in bounds.
    unsafe {
        if current != 0 {
            (*addr_of!(parameters::param_stack)).0[0]
                .values
                .get(index)
                .copied()
                .unwrap_or(0)
        } else {
            (*addr_of!(parameters::param_defaults))
                .0
                .get(index)
                .copied()
                .unwrap_or(0)
        }
    }
}
/// `espeak_SetPhonemeTrace`: the trace options and stream (stderr if none).
///
/// # Safety
/// `stream` is null or a live C stream; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_set_phoneme_trace(mode: c_int, stream: *mut libc::FILE) {
    // SAFETY: serialized engine options.
    unsafe {
        c::option_phonemes = mode;
        c::f_trans = if stream.is_null() {
            crt::standard_error()
        } else {
            stream
        };
    }
}
/// `espeak_TextToPhonemesWithTerminator`.
///
/// # Safety
/// As the C function; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_text_to_phonemes(
    text: *mut *const c_void,
    mode: c_int,
    phoneme_mode: c_int,
    terminator: *mut c_int,
) -> *const c_char {
    // SAFETY: caller contract; the engine's decoder and translator.
    unsafe {
        if p_decoder.is_null() {
            p_decoder = super::create_text_decoder().cast();
        }
        let decoder = p_decoder.cast();
        let tr = c::translator.cast::<Translator>();
        if super::text_decoder_decode_string_multibyte(decoder, *text, (*tr).encoding as u32, mode)
            != 0
        {
            return ptr::null();
        }
        TranslateClauseWithTerminator(tr.cast(), ptr::null_mut(), ptr::null_mut(), terminator);
        *text = super::text_decoder_get_buffer(decoder);
        espeak_rs_translated_phonemes_engine(phoneme_mode)
    }
}
/// `espeak_IsPlaying`; `busy` is the asynchronous queue's test when the
/// build has one.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_is_playing(busy: Option<unsafe extern "C" fn() -> c_int>) -> c_int {
    // SAFETY: the queue's own predicate.
    busy.map_or(0, |busy| unsafe { busy() })
}
/// `espeak_Info`: the version, and the data path through `path`.
///
/// # Safety
/// `path` is null or writable; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_info(
    path: *mut *const c_char,
    version: *const c_char,
) -> *const c_char {
    if !path.is_null() {
        // SAFETY: caller contract.
        unsafe { *path = addr_of!(c::path_home) };
    }
    version
}
