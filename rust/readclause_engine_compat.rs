//! The clause reader's state and callbacks against the engine (from
//! readclause.c): `ReadClause`, `InitText2`, `SetVoiceStack` and the name
//! store, with the character, punctuation and SSML effects the Rust clause,
//! name and punctuation hosts call.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_upper_case_globals)]
use super::{
    clause_engine_compat as clause, clause_names_compat as names,
    clause_punctuation_compat as punctuation, engine_c as c,
    phoneme_render_compat::engine::espeak_rs_decode_phonemes_bounded_engine,
    ssml_compat::host::espeak_rs_ssml_tag_engine, ForeignVoice, RawDecoder, WChar,
};
use crate::{
    clause_engine::{Command, State},
    crt::PATH_SEPARATOR,
    name_storage::Names,
    ssml_voice::Frame,
    translator::Translator,
    voice::Voice,
};
use std::{
    ffi::{c_char, c_int, c_void},
    ptr::{self, addr_of, addr_of_mut},
};

unsafe extern "C" {
    static mut count_characters: c_int;
    static mut p_decoder: *mut c_void;
    static mut namedata: *mut c_char;
    static mut translator2: *mut c_void;
    static mut skipping_text: bool;
    static mut option_ssml: c_int;
    static mut option_phoneme_input: c_int;
    static mut option_linelength: c_int;
    static mut option_capitals: c_int;
    static mut option_punctuation: c_int;
    static mut option_punctlist: [WChar; 60];
    static mut skip_characters: c_int;
    static mut end_character_position: c_int;
    static mut clause_start_char: c_int;
    fn SetTranslator2(name: *const c_char) -> c_int;
    fn LookupSoundicon(code: c_int) -> c_int;
    fn espeak_GetCurrentVoice() -> *const ForeignVoice;
}

/// `CLAUSE_EOF` (translate.h).
const CLAUSE_EOF: c_int = 40 | 0x0008_0000 | 0x0001_0000;
/// The name store's limit, 128 MiB.
const NAME_BYTES: usize = 128 * 1024 * 1024;

// The reader's state, file statics in readclause.c.
static mut UNGOT: c_int = 0;
static mut UNGOT_SECOND: c_int = 0;
static mut XMLBASE: *const c_char = c"".as_ptr();
static mut IGNORE_TEXT: bool = false;
static mut AUDIO_TEXT: bool = false;
static mut CLEAR_SKIPPING_TEXT: bool = false;
static mut SAYAS_MODE: c_int = 0;
static mut SAYAS_START: c_int = 0;
static mut SSML_COUNT: c_int = 0;
// SAFETY: integers and byte arrays, valid as all zeros.
static mut SSML_STACK: [Frame; 20] = unsafe { std::mem::zeroed() };
// SAFETY: null pointers and zero scalars.
static mut BASE_VOICE: ForeignVoice = unsafe { std::mem::zeroed() };
static mut BASE_VARIANT: [u8; 40] = [0; 40];
static mut CURRENT_VOICE: [u8; 40] = [0; 40];
static mut PARAMETER_COUNT: c_int = 0;
static mut SPEECH_PARAMETERS: [i32; 15] = [0; 15];
static mut REPLAY_INDEX: i32 = -1;
static mut REPLAY: [u8; 24] = [0; 24];
static mut NAMES: *mut Names = ptr::null_mut();

/// `strncpy0(destination, source, N)`: at most N-1 bytes, NUL-padded.
///
/// # Safety
/// `source` is terminated or has N-1 readable bytes.
unsafe fn copy_padded<const N: usize>(destination: &mut [u8; N], source: *const u8) {
    let mut ended = false;
    for (index, slot) in destination.iter_mut().enumerate() {
        if !ended && index < N - 1 {
            // SAFETY: read only up to the terminator or N-1 bytes.
            let byte = unsafe { *source.add(index) };
            ended = byte == 0;
            *slot = byte;
        } else {
            ended = true;
            *slot = 0;
        }
    }
}
/// `strcpy(destination, source)` for a terminated buffer of N bytes.
///
/// # Safety
/// `destination` has N writable bytes.
unsafe fn copy_terminated<const N: usize>(destination: *mut u8, source: &[u8; N]) {
    let length = source.iter().position(|&b| b == 0).map_or(N, |end| end + 1);
    // SAFETY: caller contract.
    unsafe { ptr::copy_nonoverlapping(source.as_ptr(), destination, length) };
}

unsafe extern "C" fn eof() -> c_int {
    // SAFETY: serialized reader state and decoder.
    unsafe { super::espeak_rs_clause_eof(UNGOT, p_decoder.cast::<RawDecoder>()) }
}
/// `Eof`: whether the text is finished, with nothing read back.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_reader_eof() -> c_int {
    // SAFETY: caller contract.
    unsafe { eof() }
}
unsafe extern "C" fn get_char() -> c_int {
    // SAFETY: serialized reader state and decoder.
    unsafe {
        super::espeak_rs_clause_getc(
            addr_of_mut!(UNGOT),
            addr_of_mut!(count_characters),
            p_decoder.cast::<RawDecoder>(),
        )
    }
}
unsafe extern "C" fn unget_char(code: c_int) {
    // SAFETY: serialized reader state.
    unsafe { UNGOT = code };
}
unsafe extern "C" fn unget_second(code: c_int) {
    // SAFETY: serialized reader state.
    unsafe { UNGOT_SECOND = code };
}

/// `DecodeWithPhonemeMode`: phonemes as `[[...]]` text, in the secondary
/// translator's language when `secondary` is set.
unsafe fn decode_with_phoneme_mode(
    output: *mut u8,
    phonemes: *mut u8,
    tr: *mut Translator,
    secondary: *mut Translator,
    flags: *mut u32,
    capacity: usize,
) {
    let mut decoded = [0u8; 55];
    let stressed = if secondary.is_null() { tr } else { secondary };
    // SAFETY: the engine's translators and the caller's buffers.
    unsafe {
        c::SetWordStress(stressed.cast(), phonemes.cast(), flags, -1, 0);
        if espeak_rs_decode_phonemes_bounded_engine(phonemes, decoded.as_mut_ptr(), decoded.len())
            < 0
            || super::espeak_rs_clause_phoneme_wrapper(
                decoded.as_ptr().cast(),
                if secondary.is_null() {
                    ptr::null()
                } else {
                    c"en".as_ptr()
                },
                (*tr).translator_name as u32,
                output,
                capacity,
            ) < 0
        {
            output.write(0);
        }
    }
}

/// The character-name host's queries (`CharacterQuery`).
unsafe extern "C" fn character_query(owner: *mut c_void, command: *mut names::Command) -> i32 {
    let tr = owner.cast::<Translator>();
    // SAFETY: the host's exclusive command for this call.
    let command = unsafe { &mut *command };
    let mut phonemes = [0u8; 200];
    phonemes[..60].copy_from_slice(&command.data.phonemes);
    let start = command.data.start as usize;
    if start >= command.data.word.len() {
        return 2;
    }
    let mut word = command.data.word[start..].as_mut_ptr();
    // SAFETY: serialized engine translators, voice and tables.
    unsafe {
        let selected = if command.secondary != 0 {
            translator2.cast::<Translator>()
        } else {
            tr
        };
        match command.kind {
            1 => {
                if selected.is_null() {
                    return 2;
                }
                command.found = i32::from(
                    c::LookupDictListBounded(
                        selected.cast(),
                        ptr::addr_of_mut!(word).cast(),
                        phonemes.as_mut_ptr().cast(),
                        command.data.flags.as_mut_ptr(),
                        0,
                        ptr::null_mut(),
                        0,
                        phonemes.len(),
                    ) != 0,
                );
            }
            2 => {
                c::TranslateRules(
                    tr.cast(),
                    word.cast(),
                    phonemes.as_mut_ptr().cast(),
                    command.data.phonemes.len() as c_int,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                );
            }
            3 => {
                SetTranslator2(c"en".as_ptr());
                return if translator2.is_null() { 2 } else { 0 };
            }
            4 => {
                if selected.is_null() {
                    return 2;
                }
                decode_with_phoneme_mode(
                    command.text.as_mut_ptr(),
                    phonemes.as_mut_ptr(),
                    tr,
                    if command.secondary != 0 {
                        selected
                    } else {
                        ptr::null_mut()
                    },
                    command.data.flags.as_mut_ptr(),
                    command.text.len(),
                );
            }
            5 => {
                let voice = c::voice.cast::<Voice>();
                if voice.is_null() {
                    return 2;
                }
                super::synthdata_compat::SelectPhonemeTable((*voice).phoneme_table);
                return 0;
            }
            _ => return 2,
        }
    }
    let length = phonemes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(phonemes.len());
    if length >= command.data.phonemes.len() {
        return 2;
    }
    command.data.phonemes[..=length].copy_from_slice(&phonemes[..=length]);
    0
}
fn character_context(tr: *mut Translator) -> names::Context {
    names::Context {
        owner: tr.cast(),
        query: Some(character_query),
        // SAFETY: a live translator's field address, read during the call.
        language: unsafe { addr_of!((*tr).translator_name) },
    }
}
/// `LookupSpecial`: whether `name` has text, written to `output`.
unsafe fn lookup_special(
    tr: *mut Translator,
    name: &std::ffi::CStr,
    output: *mut u8,
    capacity: usize,
) -> bool {
    let context = character_context(tr);
    // SAFETY: the live translator and the caller's output.
    unsafe { names::espeak_rs_clause_special(&context, name.as_ptr(), output, capacity) >= 0 }
}
/// `LookupCharName` into a 74-byte output (empty when there is none).
unsafe fn lookup_char_name(output: *mut u8, tr: *mut Translator, code: c_int, only: bool) {
    let context = character_context(tr);
    // SAFETY: the live translator and the caller's 74 bytes.
    unsafe {
        if names::espeak_rs_clause_character_name(&context, code, u32::from(only), output, 74) < 0 {
            output.write(0);
        }
    }
}
unsafe extern "C" fn punctuation_name(
    owner: *mut c_void,
    code: i32,
    period: u32,
    output: *mut [u8; 74],
) -> i32 {
    let tr = owner.cast::<Translator>();
    let mut text = [0u8; 74];
    // SAFETY: the live translator and local text.
    let found = unsafe {
        if period != 0 {
            lookup_special(tr, c"_.p", text.as_mut_ptr(), text.len())
        } else {
            lookup_char_name(text.as_mut_ptr(), tr, code, false);
            true
        }
    };
    if !found {
        return 1;
    }
    let Some(length) = text.iter().position(|&b| b == 0) else {
        return 2;
    };
    // SAFETY: the host's 74-byte output.
    unsafe { (&mut *output)[..=length].copy_from_slice(&text[..=length]) };
    0
}
/// `AnnouncePunctuation`.
unsafe fn announce(
    tr: *mut Translator,
    code: c_int,
    next: *mut c_int,
    output: *mut u8,
    index: *mut c_int,
    end_clause: c_int,
    capacity: c_int,
) -> c_int {
    if capacity < 0 {
        return -1;
    }
    // SAFETY: the live translator and the engine's embedded values.
    let context = unsafe {
        punctuation::Context {
            owner: tr.cast(),
            icon: Some(LookupSoundicon),
            name: Some(punctuation_name),
            eof: Some(eof),
            read: Some(get_char),
            unread: Some(unget_char),
            unread_second: Some(unget_second),
            // LOPT_ANNOUNCE_PUNCT and EMBED_S.
            flags: addr_of!((*tr).langopts.param[13]),
            speed: addr_of!(super::wave_memory_compat::embedded_value.0[2]),
        }
    };
    // SAFETY: the caller's output and cursors.
    unsafe {
        punctuation::espeak_rs_clause_announce(
            &context,
            code,
            next,
            output,
            capacity as usize,
            index,
            u32::from(end_clause != 0),
        )
    }
}

/// `IgnoreOrReplaceChar`.
unsafe fn ignore_or_replace(tr: *mut Translator, code: *mut i32) -> bool {
    // SAFETY: the live translator's terminated pair table.
    unsafe {
        let table = (*tr).chars_ignore;
        if table.is_null() {
            return false;
        }
        let mut length = 0;
        while *table.add(length) != 0 {
            length += 2;
        }
        super::espeak_rs_clause_replace(table, length + 1, code) == 1
    }
}

/// What one `ReadClause` call passes its effects.
struct Owner {
    tr: *mut Translator,
    top: *mut c_int,
    tone: *mut c_int,
    voice: *mut u8,
    sonic: u32,
}
/// `ClauseSnapshot`: the engine state the clause reader reads.
unsafe fn snapshot(owner: &Owner, s: &mut State) {
    // SAFETY: serialized engine state and the live translator.
    unsafe {
        s.cursor.pending = UNGOT;
        s.cursor.count = count_characters;
        s.pending_second = UNGOT_SECOND;
        s.ignore = i32::from(IGNORE_TEXT);
        s.audio = i32::from(AUDIO_TEXT);
        s.clear_skipping = i32::from(CLEAR_SKIPPING_TEXT);
        s.skipping = i32::from(skipping_text);
        s.ssml = option_ssml;
        s.phoneme_input = option_phoneme_input;
        s.line_length = option_linelength;
        s.capitals = option_capitals;
        s.punctuation = option_punctuation;
        let list = &*addr_of!(option_punctlist);
        for (slot, &code) in s.punctuation_list.iter_mut().zip(list) {
            // WChar is u16 on Windows.
            #[allow(clippy::useless_conversion)]
            let code = u32::from(code);
            *slot = code;
            if code == 0 {
                break;
            }
        }
        s.sayas_mode = SAYAS_MODE;
        s.sayas_start = SAYAS_START;
        s.parameters = SPEECH_PARAMETERS;
        s.skip_characters = skip_characters;
        s.end_position = end_character_position;
        s.clause_start = clause_start_char;
        let tr = &*owner.tr;
        s.repeat_count = tr.phonemes_repeat_count;
        s.upper_count = tr.clause_upper_count;
        s.lower_count = tr.clause_lower_count;
        s.language = tr.translator_name;
        s.numbers = tr.langopts.numbers;
        s.lowercase_sentence = i32::from(tr.langopts.lowercase_sentence);
        copy_padded(&mut s.current_voice, addr_of!(CURRENT_VOICE).cast());
        let base = (*addr_of!(BASE_VOICE)).identifier;
        s.has_base_identifier = i32::from(!base.is_null());
        if !base.is_null() {
            copy_padded(&mut s.base_identifier, base.cast());
        }
    }
}
/// `ClausePublish`: the clause reader's state back into the engine.
unsafe fn publish(owner: &Owner, s: &State) {
    // SAFETY: serialized engine state, the live translator and the caller's
    // index, tone and 40-byte voice-change outputs.
    unsafe {
        UNGOT = s.cursor.pending;
        count_characters = s.cursor.count;
        UNGOT_SECOND = s.pending_second;
        IGNORE_TEXT = s.ignore != 0;
        AUDIO_TEXT = s.audio != 0;
        CLEAR_SKIPPING_TEXT = s.clear_skipping != 0;
        skipping_text = s.skipping != 0;
        option_ssml = s.ssml;
        option_phoneme_input = s.phoneme_input;
        option_linelength = s.line_length;
        option_capitals = s.capitals;
        option_punctuation = s.punctuation;
        let list = &mut *addr_of_mut!(option_punctlist);
        for (slot, &code) in list.iter_mut().zip(&s.punctuation_list) {
            *slot = code as WChar;
            if code == 0 {
                break;
            }
        }
        SAYAS_MODE = s.sayas_mode;
        SAYAS_START = s.sayas_start;
        SPEECH_PARAMETERS = s.parameters;
        skip_characters = s.skip_characters;
        end_character_position = s.end_position;
        clause_start_char = s.clause_start;
        let tr = &mut *owner.tr;
        tr.phonemes_repeat_count = s.repeat_count;
        tr.clause_upper_count = s.upper_count;
        tr.clause_lower_count = s.lower_count;
        copy_terminated(addr_of_mut!(CURRENT_VOICE).cast(), &s.current_voice);
        *owner.top = s.index_top;
        *owner.tone = s.tone;
        copy_terminated(owner.voice, &s.voice_change);
    }
}
unsafe extern "C" fn source_eof(_: *mut c_void) -> i32 {
    // SAFETY: the engine's serialized decoder.
    unsafe { super::text_decoder_eof(p_decoder.cast::<RawDecoder>()) }
}
unsafe extern "C" fn source_read(_: *mut c_void) -> u32 {
    // SAFETY: the engine's serialized decoder.
    unsafe { super::text_decoder_getc(p_decoder.cast::<RawDecoder>()) }
}
unsafe extern "C" fn source_peek(_: *mut c_void) -> u32 {
    // SAFETY: the engine's serialized decoder.
    unsafe { super::text_decoder_peekc(p_decoder.cast::<RawDecoder>()) }
}
/// `ClauseClassify`: the classes C's wide classifiers (ucd-tools through
/// src/include/compat/wctype.h), IsAlpha, IsBracket and isspace gave.
unsafe extern "C" fn classify(code: i32, kind: u32) -> i32 {
    let wide = code as u32;
    let class = match kind {
        0 => super::ucd_isspace(wide),
        1 => super::ucd_isalnum(wide),
        2 => super::ucd_isalpha(wide),
        3 => super::ucd_isupper(wide),
        4 => super::ucd_islower(wide),
        5 => super::ucd_isdigit(wide),
        6 => super::ucd_ispunct(wide),
        // SAFETY: pure classifiers.
        7 => unsafe { super::espeak_rs_common_class(wide, 0) },
        // SAFETY: as above.
        8 => unsafe { super::espeak_rs_common_predicate(wide, 5, None) },
        // SAFETY: the C classifier of an unsigned char value.
        9 => unsafe { libc::isspace(c_int::from(code as u8)) },
        _ => 0,
    };
    i32::from(class != 0)
}
unsafe extern "C" fn replace(owner: *mut c_void, code: *mut i32) -> i32 {
    // SAFETY: the live owner this call's entry made.
    i32::from(unsafe { ignore_or_replace((*owner.cast::<Owner>()).tr, code) })
}
/// `ClauseEffect`: an SSML tag, punctuation, character name or capital
/// marker the clause reader asks the engine for.
unsafe extern "C" fn effect(
    value: *mut c_void,
    state: *mut State,
    command: *mut Command,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    // SAFETY: the live owner this call's entry made and the reader's
    // exclusive state, command and output.
    unsafe {
        let owner = &*value.cast::<Owner>();
        let s = &mut *state;
        let command = &mut *command;
        publish(owner, s);
        if command.index < 0 || command.index as usize > capacity || capacity > i32::MAX as usize {
            return 2;
        }
        match command.kind {
            1 => {
                let mut xml = [0 as WChar; 501];
                for (slot, &code) in xml.iter_mut().zip(&command.xml) {
                    *slot = code as WChar;
                    if *slot == 0 {
                        break;
                    }
                }
                command.clause = espeak_rs_ssml_tag_engine(
                    xml.as_mut_ptr(),
                    output,
                    &mut command.index,
                    capacity as c_int,
                    XMLBASE,
                    addr_of_mut!(AUDIO_TEXT).cast(),
                    addr_of_mut!(CURRENT_VOICE).cast(),
                    addr_of!(BASE_VOICE),
                    addr_of!(BASE_VARIANT).cast(),
                    addr_of_mut!(IGNORE_TEXT).cast(),
                    addr_of_mut!(CLEAR_SKIPPING_TEXT).cast(),
                    addr_of_mut!(SAYAS_MODE),
                    addr_of_mut!(SAYAS_START),
                    addr_of_mut!(SSML_STACK).cast(),
                    addr_of_mut!(SSML_COUNT),
                    addr_of_mut!(PARAMETER_COUNT),
                    addr_of_mut!(SPEECH_PARAMETERS),
                    owner.sonic,
                );
            }
            2 => {
                command.clause = announce(
                    owner.tr,
                    command.code,
                    &mut command.next,
                    output,
                    &mut command.index,
                    command.end,
                    capacity as c_int,
                );
            }
            3 => lookup_char_name(
                command.text.as_mut_ptr(),
                owner.tr,
                command.code,
                command.end != 0,
            ),
            4 => {
                command.found = i32::from(lookup_special(
                    owner.tr,
                    c"_cap",
                    command.text.as_mut_ptr(),
                    30,
                ));
            }
            _ => return 2,
        }
        snapshot(owner, s);
        // User/backend callbacks may observe/change these published scalar outputs.
        s.index_top = *owner.top;
        s.tone = *owner.tone;
        copy_padded(&mut s.voice_change, owner.voice);
    }
    0
}

/// `ReadClause`. `sonic` is whether the build has libsonic.
///
/// # Safety
/// As the C function; serialized engine.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn espeak_rs_read_clause_engine(
    tr: *mut Translator,
    buf: *mut u8,
    charix: *mut i16,
    charix_top: *mut c_int,
    n_buf: c_int,
    tone_type: *mut c_int,
    voice_change: *mut u8,
    sonic: u32,
) -> c_int {
    if tr.is_null()
        || buf.is_null()
        || charix.is_null()
        || charix_top.is_null()
        || tone_type.is_null()
        || voice_change.is_null()
        || n_buf < 0
    {
        return CLAUSE_EOF;
    }
    let owner = Owner {
        tr,
        top: charix_top,
        tone: tone_type,
        voice: voice_change,
        sonic,
    };
    // SAFETY: integers and byte arrays, valid as all zeros (C's `= {0}`).
    let mut state: State = unsafe { std::mem::zeroed() };
    // SAFETY: serialized engine state and the caller's outputs.
    let clause = unsafe {
        snapshot(&owner, &mut state);
        state.index_top = *charix_top;
        state.replay_index = REPLAY_INDEX;
        state.replay = REPLAY;
        state.signed_bytes = i32::from(c_char::MIN != 0);
        state.wide16 = i32::from(std::mem::size_of::<WChar>() == 2);
        *tone_type = 0;
        *voice_change = 0;
        let context = clause::Context {
            owner: addr_of!(owner).cast_mut().cast(),
            eof: Some(source_eof),
            read: Some(source_read),
            peek: Some(source_peek),
            classify: Some(classify),
            replace: Some(replace),
            effect: Some(effect),
        };
        let clause = clause::espeak_rs_read_clause(
            &context,
            &mut state,
            buf,
            n_buf as usize,
            charix,
            n_buf as usize,
        );
        publish(&owner, &state);
        REPLAY_INDEX = state.replay_index;
        REPLAY = state.replay;
        clause
    };
    if clause < 0 {
        if n_buf > 0 {
            // SAFETY: the caller's non-empty buffer.
            unsafe { buf.write(0) };
        }
        return CLAUSE_EOF;
    }
    clause
}

/// `InitText2`: the reader's state for a new text.
///
/// # Safety
/// Serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_init_text() {
    // SAFETY: serialized engine state.
    unsafe {
        UNGOT = 0;
        UNGOT_SECOND = 0;
        SSML_COUNT = 1;
        PARAMETER_COUNT = 1;
        (*addr_of_mut!(SSML_STACK))[0].kind = 0;
        SPEECH_PARAMETERS = (*addr_of!(super::parameter_control_compat::param_stack)).0[0].values;
        // espeakPUNCTUATION and espeakCAPITALS.
        option_punctuation = SPEECH_PARAMETERS[5];
        option_capitals = SPEECH_PARAMETERS[6];
        CURRENT_VOICE[0] = 0;
        IGNORE_TEXT = false;
        AUDIO_TEXT = false;
        CLEAR_SKIPPING_TEXT = false;
        count_characters = -1;
        SAYAS_MODE = 0;
        XMLBASE = ptr::null();
    }
}

/// `SetVoiceStack`: the voice SSML falls back to.
///
/// # Safety
/// `voice` is null or a live voice description; `variant` is terminated.
#[no_mangle]
unsafe extern "C" fn espeak_rs_set_voice_stack(voice: *const ForeignVoice, variant: *const c_char) {
    // SAFETY: serialized engine state and the caller's terminated strings.
    unsafe {
        let base = &mut (*addr_of_mut!(SSML_STACK))[0];
        if voice.is_null() {
            *base = std::mem::zeroed();
            return;
        }
        let voice = &*voice;
        if !voice.languages.is_null() {
            let language = c::text(voice.languages.cast());
            let length = language.len().min(base.language.len() - 1);
            base.language[..length].copy_from_slice(&language[..length]);
            base.language[length] = 0;
        }
        if !voice.name.is_null() {
            copy_padded(&mut base.name, voice.name.cast());
        }
        base.variant = i32::from(voice.variant);
        base.age = i32::from(voice.age);
        base.gender = i32::from(voice.gender);
        let mut variant = variant.cast::<u8>();
        // Strip the variant directory name, !v plus PATHSEP.
        if *variant == b'!' && *variant.add(1) == b'v' && *variant.add(2) == PATH_SEPARATOR {
            variant = variant.add(3);
        }
        copy_padded(&mut *addr_of_mut!(BASE_VARIANT), variant);
        BASE_VOICE = *espeak_GetCurrentVoice();
    }
}

/// `AddNameData`: stores a name or URI, returning its offset in `namedata`.
///
/// # Safety
/// `name` is a terminated byte string, or wide string when `wide` is set.
#[no_mangle]
pub(super) unsafe extern "C" fn espeak_rs_add_name_data(name: *const c_char, wide: c_int) -> c_int {
    if name.is_null() {
        return -1;
    }
    let width = if wide != 0 {
        std::mem::size_of::<WChar>()
    } else {
        1
    };
    let mut characters = 0;
    // SAFETY: the caller's terminated string of `width`-byte units.
    unsafe {
        if wide != 0 {
            while *name.cast::<WChar>().add(characters) != 0 {
                characters += 1;
            }
        } else {
            characters = libc::strlen(name);
        }
    }
    if characters >= NAME_BYTES / width {
        return -1;
    }
    // SAFETY: serialized engine state.
    unsafe {
        if NAMES.is_null() {
            NAMES = super::espeak_rs_names_create(NAME_BYTES);
            if NAMES.is_null() {
                return -1;
            }
        }
        let mut view = ptr::null();
        let index = super::espeak_rs_names_append(
            NAMES,
            name.cast(),
            (characters + 1) * width,
            width,
            &mut view,
        );
        if index >= 0 {
            namedata = view.cast_mut().cast();
        }
        index
    }
}
/// `FreeNamedata`.
///
/// # Safety
/// No name view is in use; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_free_name_data() {
    // SAFETY: caller contract.
    unsafe {
        super::espeak_rs_names_destroy(NAMES);
        NAMES = ptr::null_mut();
        namedata = ptr::null_mut();
    }
}
/// `InitNamedata`.
///
/// # Safety
/// No name view is in use; serialized engine.
#[no_mangle]
unsafe extern "C" fn espeak_rs_init_name_data() {
    // SAFETY: caller contract.
    unsafe {
        super::espeak_rs_names_reset(NAMES);
        namedata = ptr::null_mut();
    }
}
