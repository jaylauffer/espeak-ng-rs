//! Serialized C bridge for the owned SSML controller.
// SPDX-License-Identifier: GPL-3.0-or-later
use super::{borrowed_voice, ForeignVoice, SsmlSpace, WChar};
use crate::{ssml_engine as engine, ssml_parameters as parameters, ssml_voice as voice};
use std::{
    ffi::{c_char, CStr},
    ptr,
};
type Append = unsafe extern "C" fn(*const c_char, i32) -> i32;
type Load = unsafe extern "C" fn(*const c_char) -> i32;
type Uri = unsafe extern "C" fn(i32, *const c_char, *const c_char) -> i32;
type Rate = unsafe extern "C" fn(i32, *mut RateView);
type Resolve = unsafe extern "C" fn(*const [u8; 40], *mut [u8; 40]) -> i32;
type Select = unsafe extern "C" fn(*const voice::Choice, *mut [u8; 40]) -> i32;
#[derive(Clone, Copy)]
#[repr(C)]
struct RateView {
    clause_pause: i32,
    pause: i32,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Context {
    parameters: *mut parameters::Frame,
    parameter_count: *mut i32,
    current: *mut [i32; 15],
    voices: *mut voice::Frame,
    voice_count: *mut i32,
    current_voice: *mut u8,
    previous_identifier: *mut [u8; 40],
    skip: *mut u8,
    punctuation: *mut i32,
    capitals: *mut i32,
    audio: *mut u8,
    ignore: *mut u8,
    clear_skipping: *mut u8,
    sayas_mode: *mut i32,
    sayas_start: *mut i32,
    base_voice: *const ForeignVoice,
    variant: *const c_char,
    xmlbase: *const c_char,
    wide_space: Option<SsmlSpace>,
    byte_space: Option<SsmlSpace>,
    lower: Option<SsmlSpace>,
    append: Option<Append>,
    load: Option<Load>,
    uri: Option<Uri>,
    rate: Option<Rate>,
    resolve: Option<Resolve>,
    select: Option<Select>,
    signed: u32,
    decimal: u32,
    tone: i32,
    sonic: u32,
}
unsafe fn string<const N: usize>(source: *const u8) -> Result<[u8; N], engine::Error> {
    if source.is_null() {
        return Err(engine::Error::Bounds);
    }
    let mut result = [0; N];
    for (index, slot) in result.iter_mut().enumerate() {
        // SAFETY: foreign owner retains an initialized terminated prefix within
        // N bytes. Read only through its NUL, never undefined unused tail.
        *slot = unsafe { *source.add(index) };
        if *slot == 0 {
            return Ok(result);
        }
    }
    Err(engine::Error::Bounds)
}
unsafe fn state(
    context: Context,
    mut result: engine::State,
) -> Result<engine::State, engine::Error> {
    // SAFETY: all context scalar pointers are live initialized/disjoint fields;
    // snapshot before any host callback, retaining no foreign exclusive borrow.
    let (parameter_count, voice_count, audio, ignore, clear) = unsafe {
        (
            *context.parameter_count,
            *context.voice_count,
            *context.audio,
            *context.ignore,
            *context.clear_skipping,
        )
    };
    if !(1..20).contains(&parameter_count)
        || !(1..=20).contains(&voice_count)
        || audio > 1
        || ignore > 1
        || clear > 1
    {
        return Err(engine::Error::Bounds);
    }
    result.parameter_count = parameter_count as usize;
    result.voice_count = voice_count as usize;
    for (index, frame) in result.parameters[..result.parameter_count]
        .iter_mut()
        .enumerate()
    {
        // SAFETY: all active parameter record integers are initialized. Unused
        // records are retained locally and never read from foreign storage.
        *frame = unsafe { context.parameters.add(index).read() };
    }
    for (index, frame) in result.voices[..result.voice_count].iter_mut().enumerate() {
        let source = unsafe {
            // SAFETY: context has20 live writable records; this index is active.
            context.voices.add(index)
        };
        // SAFETY: active scalar fields and terminated name/language prefixes
        // are initialized; unused byte tails are not borrowed or read.
        *frame = unsafe {
            voice::Frame {
                kind: ptr::addr_of!((*source).kind).read(),
                variant: ptr::addr_of!((*source).variant).read(),
                gender: ptr::addr_of!((*source).gender).read(),
                age: ptr::addr_of!((*source).age).read(),
                name: string(ptr::addr_of!((*source).name).cast())?,
                language: string(ptr::addr_of!((*source).language).cast())?,
            }
        };
    }
    // SAFETY: copied initialized fields/prefixes from the same live context.
    unsafe {
        result.current = *context.current;
        result.punctuation = *context.punctuation;
        result.capitals = *context.capitals;
        result.current_voice = string(context.current_voice)?;
        result.previous_identifier = string(context.previous_identifier.cast())?;
        result.skip = string(context.skip)?;
        result.audio = audio != 0;
        result.ignore = ignore != 0;
        result.clear_skipping = clear != 0;
        result.sayas_mode = *context.sayas_mode;
        result.sayas_start = *context.sayas_start;
    }
    Ok(result)
}
unsafe fn publish(context: Context, next: &engine::State, prior: &engine::State) {
    for (index, (next, prior)) in next.parameters.iter().zip(&prior.parameters).enumerate() {
        if next != prior {
            // SAFETY: exclusively serialized writable record; publish only
            // changed initialized records, preserving untouched inactive slots.
            unsafe {
                context.parameters.add(index).write(*next);
            }
        }
    }
    for (index, (next, prior)) in next.voices.iter().zip(&prior.voices).enumerate() {
        if next != prior {
            // SAFETY: changed owned record is fully initialized; inactive
            // foreign records are overwritten only when actually installed.
            unsafe {
                context.voices.add(index).write(*next);
            }
        }
    }
    // SAFETY: scalar outputs are initialized/exclusive/disjoint. Write only
    // changed string prefixes+NUL, leaving possibly undefined caller tails alone.
    unsafe {
        *context.parameter_count = next.parameter_count as i32;
        *context.voice_count = next.voice_count as i32;
        *context.current = next.current;
        *context.punctuation = next.punctuation;
        *context.capitals = next.capitals;
        *context.audio = u8::from(next.audio);
        *context.ignore = u8::from(next.ignore);
        *context.clear_skipping = u8::from(next.clear_skipping);
        *context.sayas_mode = next.sayas_mode;
        *context.sayas_start = next.sayas_start;
        for (destination, next, prior) in [
            (
                context.current_voice,
                next.current_voice.as_slice(),
                prior.current_voice.as_slice(),
            ),
            (
                context.previous_identifier.cast(),
                next.previous_identifier.as_slice(),
                prior.previous_identifier.as_slice(),
            ),
            (context.skip, next.skip.as_slice(), prior.skip.as_slice()),
        ] {
            if next != prior {
                let length = next
                    .iter()
                    .position(|b| *b == 0)
                    .expect("admitted owned string")
                    + 1;
                ptr::copy_nonoverlapping(next.as_ptr(), destination, length);
            }
        }
    }
}
struct Host {
    context: Context,
    prior: engine::State,
    invalid: bool,
}
impl engine::Host for Host {
    fn wide_space(&self, c: u32) -> bool {
        // SAFETY: admitted pure synchronous host classifier.
        unsafe { self.context.wide_space.expect("admitted classifier")(c) != 0 }
    }
    fn byte_space(&self, c: u32) -> bool {
        // SAFETY: admitted pure byte classifier receives0..255.
        unsafe { self.context.byte_space.expect("admitted classifier")(c) != 0 }
    }
    fn byte_lower(&self, c: u32) -> i32 {
        // SAFETY: admitted pure byte casing function receives0..255.
        unsafe { self.context.lower.expect("admitted classifier")(c) }
    }
    fn append_name(&mut self, name: &[u8]) -> i32 {
        if name.len() >= 160 {
            return -1;
        }
        let mut text = [0; 160];
        text[..name.len()].copy_from_slice(name);
        // SAFETY: copied terminated name stays alive across host append. No
        // foreign engine owner is borrowed; serialized host context remains live.
        unsafe { self.context.append.expect("admitted append")(text.as_ptr().cast(), 0) }
    }
    fn load_sound(&mut self, path: &[u8]) -> i32 {
        if path.len() >= 256 {
            return -1;
        }
        let mut text = [0; 256];
        text[..path.len()].copy_from_slice(path);
        // SAFETY: copied terminated path retained across host worker operation.
        unsafe { self.context.load.expect("admitted load")(text.as_ptr().cast()) }
    }
    fn has_uri_callback(&self) -> bool {
        self.context.uri.is_some()
    }
    fn uri(&mut self, name: &[u8], _base: Option<&[u8]>) -> i32 {
        let Some(callback) = self.context.uri else {
            return 1;
        };
        if name.len() >= 160 {
            return 1;
        }
        let mut text = [0; 160];
        text[..name.len()].copy_from_slice(name);
        // SAFETY: owned terminated name and live original base remain valid
        // across user callback; no exclusive foreign engine borrow is retained.
        unsafe { callback(1, text.as_ptr().cast(), self.context.xmlbase) }
    }
    fn rate(&mut self, rate: i32) -> engine::Rate {
        let mut factors = RateView {
            clause_pause: 0,
            pause: 0,
        };
        // SAFETY: exclusive initialized local effects and admitted host callback.
        unsafe {
            self.context.rate.expect("admitted rate")(rate, &mut factors);
        }
        engine::Rate {
            clause_pause: factors.clause_pause,
            pause: factors.pause,
        }
    }
    fn resolve_name(&mut self, name: &[u8; 40]) -> Result<Option<[u8; 40]>, voice::Error> {
        let mut result = [0; 40];
        // SAFETY: copied terminated name and initialized disjoint local output
        // retained for callback; no foreign catalogue owner borrow is held.
        match unsafe { self.context.resolve.expect("admitted resolver")(name, &mut result) } {
            0 => Ok(Some(result)),
            1 => Ok(None),
            _ => Err(voice::Error::Resolver),
        }
    }
    fn select_voice(&mut self, choice: &voice::Choice) -> Result<Option<[u8; 40]>, voice::Error> {
        let mut result = [0; 40];
        // SAFETY: owned immutable choice and exclusive copied identifier output
        // retained across selection without a foreign catalogue owner borrow.
        match unsafe { self.context.select.expect("admitted selection")(choice, &mut result) } {
            0 => Ok(Some(result)),
            1 => Ok(None),
            _ => Err(voice::Error::Resolver),
        }
    }
    fn publish(&mut self, next: &engine::State) {
        // SAFETY: caller serializes all live context storage through callbacks.
        unsafe {
            publish(self.context, next, &self.prior);
        }
        self.prior = *next;
    }
    fn refresh(&mut self, next: &mut engine::State) {
        // SAFETY: callbacks completed; copy initialized foreign effects before
        // further owner work, keeping installed inactive owned records locally.
        match unsafe { state(self.context, *next) } {
            Ok(state) => {
                *next = state;
                self.prior = state;
            }
            Err(_) => self.invalid = true,
        }
    }
    fn valid(&self) -> bool {
        !self.invalid
    }
}
struct Output {
    data: *mut u8,
    capacity: usize,
    length: usize,
    cursor: *mut i32,
}
impl engine::Output for Output {
    fn capacity(&self) -> usize {
        self.capacity
    }
    fn prefix(&self) -> &[u8] {
        // SAFETY: only initialized prefix is borrowed. Foreign callbacks cannot
        // invalidate/reenter this output; sparse writes expire prefix borrows.
        unsafe { std::slice::from_raw_parts(self.data, self.length) }
    }
    fn write(&mut self, index: usize, bytes: &[u8]) -> Result<(), engine::Error> {
        let end = index
            .checked_add(bytes.len())
            .filter(|end| *end <= self.capacity)
            .ok_or(engine::Error::Capacity)?;
        let _ = end;
        // SAFETY: admitted exclusive writable destination, disjoint source;
        // never create a Rust mutable slice over undefined unused output tail.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.data.add(index), bytes.len());
        }
        Ok(())
    }
    fn set_length(&mut self, length: usize) -> Result<(), engine::Error> {
        if length > self.capacity {
            return Err(engine::Error::Capacity);
        }
        self.length = length;
        // SAFETY: exclusive initialized scalar cursor, update before host calls.
        unsafe {
            *self.cursor = length as i32;
        }
        Ok(())
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_process(
    context: *const Context,
    xml: *mut WChar,
    length: usize,
    data: *mut u8,
    capacity: usize,
    cursor: *mut i32,
) -> i32 {
    if context.is_null()
        || xml.is_null()
        || data.is_null()
        || cursor.is_null()
        || length > crate::ssml_control::TAG_UNITS
        || capacity > i32::MAX as usize
    {
        return 0;
    }
    // SAFETY: retained initialized context copied before callbacks; caller keeps
    // all disjoint fields/input/output alive and serializes this engine call.
    let context = unsafe { *context };
    if context.signed > 1
        || context.sonic > 1
        || context.parameters.is_null()
        || context.parameter_count.is_null()
        || context.current.is_null()
        || context.voices.is_null()
        || context.voice_count.is_null()
        || context.current_voice.is_null()
        || context.previous_identifier.is_null()
        || context.skip.is_null()
        || context.punctuation.is_null()
        || context.capitals.is_null()
        || context.audio.is_null()
        || context.ignore.is_null()
        || context.clear_skipping.is_null()
        || context.sayas_mode.is_null()
        || context.sayas_start.is_null()
        || context.base_voice.is_null()
        || context.wide_space.is_none()
        || context.byte_space.is_none()
        || context.lower.is_none()
        || context.append.is_none()
        || context.load.is_none()
        || context.rate.is_none()
        || context.resolve.is_none()
        || context.select.is_none()
    {
        return 0;
    }
    // SAFETY: initialized scalar cursor retained exclusively through the call.
    let offset = unsafe { *cursor };
    let Ok(offset) = usize::try_from(offset) else {
        return 0;
    };
    if offset > capacity {
        return 0;
    }
    let empty_voice = voice::Frame {
        kind: 0,
        variant: 0,
        gender: 0,
        age: 0,
        name: [0; 40],
        language: [0; 20],
    };
    // SAFETY: context scalar/active fields and terminated prefixes are initialized.
    let Ok(initial) = (unsafe { state(context, engine::State::new([0; 15], empty_voice)) }) else {
        return 0;
    };
    let base = {
        // SAFETY: copy initialized immutable base record and packed strings;
        // no borrow of this foreign metadata crosses a callback.
        let base = unsafe { *context.base_voice };
        // SAFETY: copied base retains its live terminated names/packed list
        // during this snapshot; no foreign metadata borrow crosses a callback.
        let Some(borrowed) = (unsafe { borrowed_voice(&base) }) else {
            return 0;
        };
        if borrowed.languages.len() > 300 {
            return 0;
        }
        let mut result = engine::Base {
            languages: [0; 300],
            gender: base.gender,
            variant: [0; 40],
        };
        result.languages[..borrowed.languages.len()].copy_from_slice(borrowed.languages);
        if !context.variant.is_null() {
            // SAFETY: optional initialized terminated variant prefix within40.
            let Ok(variant) = (unsafe { string(context.variant.cast()) }) else {
                return 0;
            };
            result.variant = variant;
        }
        result
    };
    let settings = engine::Settings {
        signed_bytes: context.signed != 0,
        decimal: context.decimal,
        tone_language: context.tone,
        sonic: context.sonic != 0,
    };
    let mut controller = engine::Controller {
        state: initial,
        base,
    };
    let mut host = Host {
        context,
        prior: initial,
        invalid: false,
    };
    let mut output = Output {
        data,
        capacity,
        length: offset,
        cursor,
    };
    // SAFETY: exclusive initialized mutable XML span, disjoint from all context
    // fields/output. Pure classifiers/user callbacks cannot mutate/invalidate or
    // reenter this tag/output call. Host effects synchronize only copied engine
    // snapshots, never a foreign Rust owner borrow across callback execution.
    let units = unsafe { std::slice::from_raw_parts_mut(xml, length) };
    #[cfg(windows)]
    let mut tag = engine::Tag::U16(units);
    #[cfg(not(windows))]
    let mut tag = engine::Tag::U32(units);
    let base = if context.xmlbase.is_null() {
        None
    } else {
        // SAFETY: optional original terminated base remains immutable/alive
        // across every host call; it cannot alias a growable name-arena entry.
        Some(unsafe { CStr::from_ptr(context.xmlbase) }.to_bytes())
    };
    let result = controller.process(&mut tag, &mut output, &settings, base, &mut host);
    // SAFETY: final admitted owned effects publish after callbacks complete,
    // including earlier effects when a later capacity/arithmetic plan rejects.
    unsafe {
        publish(context, &controller.state, &host.prior);
    }
    if host.invalid {
        0
    } else {
        result.unwrap_or(0)
    }
}

/// The SSML tag host against the engine (from ssml.c): `ProcessSsmlTag`'s
/// classifiers, voice lookups, rate update and URI callback.
#[cfg(espeak_native_host)]
pub(super) mod host {
    use super::{
        super::{engine_c as c, parameter_control_compat, ForeignVoice, SsmlSpace, WChar},
        voice, Context, RateView, Uri,
    };
    use std::{
        ffi::{c_char, c_int, c_void},
        ptr,
    };

    unsafe extern "C" {
        static mut skip_marker: [c_char; 50];
        static mut option_punctuation: c_int;
        static mut option_capitals: c_int;
        fn SelectVoiceByName(
            voices: *mut *mut ForeignVoice,
            name: *const c_char,
        ) -> *mut ForeignVoice;
        fn SelectVoice(selector: *mut ForeignVoice, found: *mut c_int) -> *const c_char;
        fn espeak_SetParameter(parameter: c_int, value: c_int, relative: c_int) -> c_int;
        fn LoadSoundFile2(name: *const c_char) -> c_int;
        fn mbrtowc(
            output: *mut libc::wchar_t,
            input: *const c_char,
            length: usize,
            state: *mut c_void,
        ) -> usize;
    }
    // libc has no localeconv for Windows; there only the first member of the
    // UCRT's lconv, decimal_point, is read (as compiler_resource_native_compat).
    #[cfg(windows)]
    #[repr(C)]
    struct LocalePrefix {
        decimal_point: *const c_char,
    }
    #[cfg(windows)]
    unsafe extern "C" {
        fn localeconv() -> *const LocalePrefix;
    }
    #[cfg(not(windows))]
    use libc::localeconv;
    /// The `<audio>` URI callback (`espeak_SetUriCallback`).
    static mut URI_CALLBACK: Option<Uri> = None;
    /// The voice identifier the last voice change selected.
    static mut PREVIOUS_IDENTIFIER: [u8; 40] = [0; 40];

    /// `iswspace((wint_t)c)`: wint_t is 16 bits on Windows.
    unsafe extern "C" fn wide_space(code: u32) -> c_int {
        #[cfg(windows)]
        let code = u32::from(code as u16);
        c_int::from(super::super::ucd_isspace(code) != 0)
    }
    unsafe extern "C" fn byte_space(code: u32) -> c_int {
        // SAFETY: the C classifier of an unsigned char value.
        c_int::from(code <= 255 && unsafe { libc::isspace(code as c_int) } != 0)
    }
    unsafe extern "C" fn lower(code: u32) -> c_int {
        if code <= 255 {
            // SAFETY: the C converter of an unsigned char value.
            unsafe { libc::tolower(code as c_int) }
        } else {
            0
        }
    }
    /// The locale's decimal point as one character ('.' when it is not one).
    fn decimal_point() -> u32 {
        // SAFETY: the C runtime's current locale, read once; a fresh zeroed
        // conversion state larger than any C runtime's mbstate_t.
        unsafe {
            let decimal = (*localeconv()).decimal_point;
            let mut symbol = b'.' as libc::wchar_t;
            let mut state = [0u64; 16];
            let result = mbrtowc(
                &mut symbol,
                decimal,
                libc::strlen(decimal),
                state.as_mut_ptr().cast(),
            );
            if result == usize::MAX || result == usize::MAX - 1 || result == 0 {
                u32::from(b'.')
            } else {
                symbol as u32
            }
        }
    }
    /// Copies a terminated identifier into 40 bytes: 0, or 2 when too long.
    unsafe fn identifier(source: *const c_char, output: *mut [u8; 40]) -> i32 {
        // SAFETY: caller's terminated identifier and 40-byte output.
        unsafe {
            let bytes = c::text(source.cast());
            if bytes.len() >= 40 {
                return 2;
            }
            let output = &mut *output;
            output[..bytes.len()].copy_from_slice(bytes);
            output[bytes.len()] = 0;
        }
        0
    }
    unsafe extern "C" fn resolve(name: *const [u8; 40], output: *mut [u8; 40]) -> i32 {
        // SAFETY: the engine's voice catalog and the host's buffers.
        unsafe {
            let selected = SelectVoiceByName(ptr::null_mut(), name.cast());
            if selected.is_null() {
                return 1;
            }
            if (*selected).identifier.is_null() {
                return 2;
            }
            identifier((*selected).identifier, output)
        }
    }
    unsafe extern "C" fn select(choice: *const voice::Choice, output: *mut [u8; 40]) -> i32 {
        // SAFETY: the host's choice, live for the call, and its output.
        unsafe {
            let choice = &*choice;
            let mut selector: ForeignVoice = std::mem::zeroed();
            selector.name = choice.name.as_ptr().cast();
            selector.identifier = choice.identifier.as_ptr().cast();
            selector.languages = choice.language.as_ptr().cast();
            selector.gender = choice.gender as u8;
            selector.age = choice.age as u8;
            selector.variant = choice.variant as u8;
            let mut found = 0;
            let selected = SelectVoice(&mut selector, &mut found);
            if selected.is_null() {
                return 1;
            }
            identifier(selected, output)
        }
    }
    unsafe extern "C" fn rate(value: i32, result: *mut RateView) {
        // SAFETY: the engine's parameter setter and speed factors.
        unsafe {
            espeak_SetParameter(1, value, 0);
            let speed = &*ptr::addr_of!(super::super::clause_postprocess_compat::speed);
            *result = RateView {
                clause_pause: speed.clause_pause,
                pause: speed.pause,
            };
        }
    }

    /// `ParseSsmlReference`, with the C runtime's byte spaces.
    ///
    /// # Safety
    /// As the C function.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_ssml_reference_engine(
        reference: *const c_char,
        first: *mut i32,
        second: *mut i32,
    ) -> c_int {
        // SAFETY: forwarded caller contract.
        unsafe {
            super::super::espeak_rs_ssml_reference(reference, first, second, Some(byte_space))
        }
    }
    /// `espeak_SetUriCallback`.
    ///
    /// # Safety
    /// Serialized engine.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_set_uri_callback(callback: Option<Uri>) {
        // SAFETY: serialized engine slot.
        unsafe { URI_CALLBACK = callback };
    }

    /// `ProcessSsmlTag`: the clause terminator a tag produces. `sonic` is
    /// whether the build has libsonic.
    ///
    /// # Safety
    /// As the C function; serialized engine.
    #[no_mangle]
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe extern "C" fn espeak_rs_ssml_tag_engine(
        xml: *mut WChar,
        output: *mut u8,
        index: *mut i32,
        capacity: c_int,
        xmlbase: *const c_char,
        audio: *mut u8,
        current_voice: *mut u8,
        base_voice: *const ForeignVoice,
        variant: *const c_char,
        ignore: *mut u8,
        clear_skipping: *mut u8,
        sayas_mode: *mut i32,
        sayas_start: *mut i32,
        voices: *mut voice::Frame,
        voice_count: *mut i32,
        parameter_count: *mut i32,
        current: *mut [i32; 15],
        sonic: u32,
    ) -> c_int {
        if capacity < 0 || xml.is_null() {
            return 0;
        }
        // SAFETY: the engine's serialized state; the translator is null or live.
        let context = unsafe {
            let tr = c::translator.cast::<crate::translator::Translator>();
            Context {
                parameters: ptr::addr_of_mut!(parameter_control_compat::param_stack).cast(),
                parameter_count,
                current,
                voices,
                voice_count,
                current_voice,
                previous_identifier: ptr::addr_of_mut!(PREVIOUS_IDENTIFIER),
                skip: ptr::addr_of_mut!(skip_marker).cast(),
                punctuation: ptr::addr_of_mut!(option_punctuation),
                capitals: ptr::addr_of_mut!(option_capitals),
                audio,
                ignore,
                clear_skipping,
                sayas_mode,
                sayas_start,
                base_voice,
                variant,
                xmlbase,
                wide_space: Some(wide_space as SsmlSpace),
                byte_space: Some(byte_space as SsmlSpace),
                lower: Some(lower as SsmlSpace),
                append: Some(super::super::readclause_engine_compat::espeak_rs_add_name_data),
                load: Some(LoadSoundFile2),
                uri: URI_CALLBACK,
                rate: Some(rate),
                resolve: Some(resolve),
                select: Some(select),
                signed: u32::from(c_char::MIN != 0),
                decimal: decimal_point(),
                tone: if tr.is_null() {
                    0
                } else {
                    (*tr).langopts.tone_language
                },
                sonic,
            }
        };
        let mut length = 0;
        // SAFETY: the host's terminated tag.
        while unsafe { *xml.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: forwarded caller contract; the context outlives the call.
        unsafe {
            super::espeak_rs_ssml_process(
                &context,
                xml,
                length + 1,
                output,
                capacity as usize,
                index,
            )
        }
    }
}
