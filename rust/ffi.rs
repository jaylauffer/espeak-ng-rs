//! Legacy C ABI adapter. The decoder borrows C buffers without copying them.
//!
//! Callers must supply live, readable buffers of the declared size, keep them
//! alive until rebinding/destroying the decoder, and serialize decoder access.
//! Mnemonic tables must end with a NULL name. Strings must be NUL-terminated.
//! All exported integer discriminants are validated before using Rust enums.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::{Encoding, Mode, State};
use crate::unicode::{self, Category};
use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;

const UNKNOWN_ENCODING: c_int = 0x100010ff;
const INVALID_ARGUMENT: c_int = 22;

type SsmlSpace = unsafe extern "C" fn(u32) -> c_int;
#[path = "clause_engine_compat.rs"]
mod clause_engine_compat;
#[path = "common_primitives_compat.rs"]
mod common_primitives_compat;
#[path = "intonation_compat.rs"]
mod intonation_compat;
#[no_mangle]
unsafe extern "C" fn espeak_rs_common_predicate(
    code: u32,
    kind: u32,
    classifier: Option<SsmlSpace>,
) -> i32 {
    if matches!(kind, 0 | 7 | 8) && classifier.is_none() {
        return -1;
    }
    let classify = |code| {
        // SAFETY: admitted pure locale callback; no owner/storage borrow held.
        unsafe { classifier.expect("admitted locale classifier")(code) }
    };
    use crate::common_text as common;
    match kind {
        0 => i32::from(common::word_alpha(code, |code| classify(code) != 0)),
        1 => i32::from(common::emoji(code)),
        2 => i32::from(common::regional_indicator(code)),
        3 => i32::from(common::emoji_modifier(code)),
        4 => i32::from(common::emoji_tag(code)),
        5 => common::bracket(code as i32),
        6 => i32::from(common::digit09(code)),
        7 => i32::from(common::digit(code, |code| classify(code) != 0)),
        8 => common::space(code, classify),
        9 => i32::from(common::byte_space(code)),
        _ => -1,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_common_null(input: *const u8, length: usize) -> i32 {
    if input.is_null() || length == 0 || length > isize::MAX as usize {
        return 0;
    }
    for index in 0..length {
        // SAFETY: caller supplies initialized readable bytes through the first
        // nonzero byte or the requested extent. Stop there, without borrowing
        // or reading an unused/uninitialized tail. Unaligned byte reads okay.
        if unsafe { *input.add(index) } != 0 {
            return 0;
        }
    }
    1
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_common_word(input: *const u8) -> u32 {
    if input.is_null() {
        return 0;
    }
    let mut bytes = [0; 4];
    for (index, byte) in bytes.iter_mut().enumerate() {
        // SAFETY: caller supplies initialized storage through first NUL or four
        // bytes, whichever occurs earlier. Unused string tail is never read.
        *byte = unsafe { *input.add(index) };
        if *byte == 0 {
            break;
        }
    }
    crate::common_text::string_word(&bytes)
}
#[no_mangle]
extern "C" fn espeak_rs_common_lower(code: u32, dotless: u32) -> u32 {
    crate::common_text::lower(code, dotless != 0)
}
#[path = "clause_names_compat.rs"]
mod clause_names_compat;
unsafe fn decode_phoneme_text(
    input: *const u8,
    length: usize,
    records: *const *const crate::phoneme::Phoneme,
    alpha: Option<SsmlSpace>,
    signed: u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if input.is_null()
        || output.is_null()
        || records.is_null()
        || signed > 1
        || length > isize::MAX as usize
        || capacity > isize::MAX as usize
    {
        return -1;
    }
    let Some(alpha) = alpha else { return -1 };
    // SAFETY: initialized input extent and all256 table pointers/pointed records
    // stay immutable/alive through pure stable locale classification; outputs
    // are exclusive/disjoint. No engine/resource callback or mutation occurs.
    let input = unsafe { std::slice::from_raw_parts(input, length) };
    let lookup = |code: u8| {
        // SAFETY: all256 table slots initialized; nonnull immutable record is
        // an initialized16-byte PHONEME_TAB copied before locale classification.
        let record = unsafe { *records.add(usize::from(code)) };
        if record.is_null() {
            None
        } else {
            // SAFETY: nonnull admitted immutable initialized record, copied by value.
            Some(unsafe { *record })
        }
    };
    let classifier = |code| {
        // SAFETY: admitted pure stable locale callback; signed-byte UB domain
        // rejected before invocation. It cannot invalidate/reenter/mutate.
        unsafe { alpha(code) != 0 }
    };
    let Ok(plan) = crate::phoneme_text::decode(input, lookup, classifier, signed != 0, capacity)
    else {
        return -1;
    };
    if plan.length > i32::MAX as usize {
        return -1;
    }
    plan.emit(|position, bytes| {
        // SAFETY: complete output footprint admitted before first write. Only
        // needed prefix/initial legacy bytes are written; no unused-tail borrow.
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), output.add(position), bytes.len()) };
        Ok(())
    })
    .map_or(-1, |length| length as i32)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_decode_phonemes(
    input: *const u8,
    length: usize,
    records: *const *const crate::phoneme::Phoneme,
    alpha: Option<SsmlSpace>,
    signed: u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    // SAFETY: caller retains declared disjoint spans and immutable table through
    // pure classification; complete checked output admission precedes emission.
    unsafe { decode_phoneme_text(input, length, records, alpha, signed, output, capacity) }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_decode_phonemes_legacy(
    input: *const c_char,
    records: *const *const crate::phoneme::Phoneme,
    alpha: Option<SsmlSpace>,
    signed: u32,
    output: *mut u8,
) -> i32 {
    if input.is_null() {
        return -1;
    }
    // SAFETY: legacy initialized terminated input; caller must provide writable
    // output for max(3, decoded prefix+NUL), as its extent-free C ABI requires.
    // Native users and known-capacity callers use the bounded entry point.
    let input = unsafe { CStr::from_ptr(input) }.to_bytes_with_nul();
    // SAFETY: retained legacy input/table and actual admitted writable footprint
    // contract, with pure stable classifiers and exclusive disjoint destination.
    unsafe {
        decode_phoneme_text(
            input.as_ptr(),
            input.len(),
            records,
            alpha,
            signed,
            output,
            isize::MAX as usize,
        )
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_phoneme_wrapper(
    decoded: *const c_char,
    secondary: *const c_char,
    language: u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if decoded.is_null() || output.is_null() {
        return -1;
    }
    // SAFETY: read only initialized terminated prefixes, never unused decoded
    // tail. Strings are immutable/live/disjoint from admitted exclusive output.
    let decoded = unsafe { CStr::from_ptr(decoded) }.to_bytes_with_nul();
    let secondary = if secondary.is_null() {
        None
    } else {
        // SAFETY: optional initialized terminated immutable default voice name.
        Some((unsafe { CStr::from_ptr(secondary) }.to_bytes(), language))
    };
    let Ok(plan) = crate::phoneme_text::wrapper(decoded, secondary, capacity) else {
        return -1;
    };
    // SAFETY: complete prefix+NUL admitted before publication; source is local.
    unsafe { ptr::copy_nonoverlapping(plan.bytes.as_ptr(), output, plan.length + 1) };
    plan.length as i32
}
#[path = "clause_punctuation_compat.rs"]
mod clause_punctuation_compat;
#[no_mangle]
extern "C" fn espeak_rs_clause_type(code: u32) -> i32 {
    crate::clause_input::clause_type(code)
}
#[no_mangle]
extern "C" fn espeak_rs_clause_properties(properties: u64) -> i32 {
    crate::clause_input::clause_properties(properties)
}
#[no_mangle]
extern "C" fn espeak_rs_clause_roman(code: u32) -> i32 {
    i32::from(crate::clause_input::roman_upper(code))
}
#[no_mangle]
extern "C" fn espeak_rs_clause_phoneme_mode(
    enabled: i32,
    mode: i32,
    current: i32,
    next: i32,
) -> i32 {
    crate::clause_input::phoneme_mode(enabled, mode, current, next)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_word(output: *mut u8, word: u32) {
    if output.is_null() {
        return;
    }
    let (bytes, length) = crate::clause_input::language_word(word);
    // SAFETY: caller supplies five writable exclusive bytes, disjoint from
    // owned source. Write only the prefix+NUL, preserving legacy unused tails.
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), output, length + 1) };
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_replace(
    table: *const u16,
    length: usize,
    code: *mut i32,
) -> i32 {
    if table.is_null() || code.is_null() || length > isize::MAX as usize / 2 {
        return -1;
    }
    // SAFETY: immutable initialized table extent and exclusive initialized code
    // are disjoint and retained through this callback-free owner call.
    let result = unsafe {
        crate::clause_input::replacement(std::slice::from_raw_parts(table, length), *code)
    };
    let Ok(result) = result else { return -1 };
    // SAFETY: admitted initialized exclusive scalar; rejected tables unchanged.
    unsafe { *code = result.code };
    i32::from(result.ignore)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_utf8_in2(
    code: *mut i32,
    mut input: *const u8,
    backwards: i32,
) -> i32 {
    if code.is_null() || input.is_null() {
        return 0;
    }
    // This legacy ABI lacks an extent. Its owner must retain readable initialized
    // storage through the first non-continuation byte in the requested direction
    // and up to three following nonzero bytes (or an earlier NUL). The safe Rust
    // API validates an explicit slice instead. No source borrow spans a callback.
    // SAFETY: caller's retained directional/head extent described above.
    unsafe {
        while *input & 0xc0 == 0x80 {
            input = input.offset(if backwards != 0 { -1 } else { 1 });
        }
    }
    let result = crate::utf8::head(|index| {
        // SAFETY: legacy initialized head/tail extent; reads only needed bytes,
        // stopping at the first NUL. Source is immutable and disjoint from code.
        Some(unsafe { *input.add(index) })
    });
    let Ok(result) = result else { return 0 };
    // SAFETY: caller's exclusive initialized output, published after decoding.
    unsafe { *code = result.code as i32 };
    result.width as i32
}
#[path = "ssml_compat.rs"]
mod ssml_compat;

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_resource(
    kind: i32,
    input: *const WChar,
    length: usize,
    start: usize,
    wide_space: Option<SsmlSpace>,
    byte_space: Option<SsmlSpace>,
    output: *mut crate::ssml_resource::Request,
) -> i32 {
    if output.is_null() {
        return 1;
    }
    let (Some(wide_space), Some(byte_space)) = (wide_space, byte_space) else {
        return 1;
    };
    // SAFETY: immutable initialized tag alive across pure classifiers, which
    // cannot mutate/invalidate/reenter; initialized output is exclusive/disjoint.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 1;
    };
    let result = crate::ssml_resource::request(
        kind,
        input,
        start,
        |c| {
            // SAFETY: pure host wide classifier.
            unsafe { wide_space(c) != 0 }
        },
        |c| {
            // SAFETY: pure byte classifier receives0..255.
            unsafe { byte_space(c) != 0 }
        },
    );
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive disjoint request after admission/classification.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_marker(
    input: *const crate::ssml_resource::Request,
    skip: *const c_char,
    output: *mut u32,
) -> i32 {
    if input.is_null() || skip.is_null() || output.is_null() {
        return 1;
    }
    // SAFETY: initialized immutable request and terminated skip string alive;
    // output is exclusive/disjoint. No callbacks or retained borrows.
    let result = unsafe { crate::ssml_resource::marker(&*input, CStr::from_ptr(skip).to_bytes()) };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive disjoint scalar effect after complete admission.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_file(
    input: *const crate::ssml_resource::Request,
    base: *const c_char,
    output: *mut crate::ssml_resource::Path,
) -> i32 {
    if input.is_null() || output.is_null() {
        return 1;
    }
    // SAFETY: optional terminated base and initialized request remain immutable
    // and alive; exclusive initialized output is disjoint. No callbacks.
    let (input, base) = unsafe {
        (
            &*input,
            if base.is_null() {
                None
            } else {
                Some(CStr::from_ptr(base).to_bytes())
            },
        )
    };
    let Ok(result) = crate::ssml_resource::file(input, base) else {
        return 1;
    };
    // SAFETY: exclusive disjoint complete path after capacity admission.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_signal(
    kind: u32,
    index: i32,
    output: *mut crate::ssml_resource::Signal,
) -> i32 {
    if output.is_null() {
        return 1;
    }
    let Ok(result) = crate::ssml_resource::signal(kind, index) else {
        return 1;
    };
    // SAFETY: exclusive initialized signal output. No engine/I/O callbacks.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_audio(
    kind: i32,
    self_closing: u32,
    output: *mut crate::ssml_resource::Audio,
) -> i32 {
    if output.is_null() || self_closing > 1 {
        return 1;
    }
    let Ok(result) = crate::ssml_resource::audio(kind, self_closing != 0) else {
        return 1;
    };
    // SAFETY: exclusive initialized audio effect output, no engine callbacks.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
extern "C" fn espeak_rs_names_create(limit: usize) -> *mut crate::name_storage::Names {
    match crate::name_storage::Names::new(limit) {
        Ok(owner) => Box::into_raw(Box::new(owner)),
        Err(_) => ptr::null_mut(),
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_names_destroy(owner: *mut crate::name_storage::Names) {
    if !owner.is_null() {
        // SAFETY: unique live owner transferred once after all name views drain.
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_names_reset(owner: *mut crate::name_storage::Names) {
    if !owner.is_null() {
        // SAFETY: exclusive live owner; all earlier records/views drained.
        unsafe {
            (&mut *owner).reset();
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_names_append(
    owner: *mut crate::name_storage::Names,
    input: *const u8,
    length: usize,
    width: usize,
    view: *mut *const u8,
) -> i32 {
    if owner.is_null()
        || input.is_null()
        || view.is_null()
        || length == 0
        || length > crate::name_storage::MAX_BYTES
        || !matches!(width, 1 | 2 | 4)
    {
        return -1;
    }
    // SAFETY: unique live owner; initialized immutable source is disjoint from
    // owner/backing bytes and exclusive view output. No callbacks/concurrency.
    let (owner, bytes) = unsafe { (&mut *owner, std::slice::from_raw_parts(input, length)) };
    let Ok(record) = owner.append(bytes, width) else {
        return -1;
    };
    // SAFETY: exclusive disjoint view output published only after full admission;
    // raw view expires at reset/growing append/destruction. No exclusive borrow
    // is retained by the adapter while the compatibility owner consumes bytes.
    unsafe {
        *view = owner.bytes().as_ptr();
    }
    record.offset as i32
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_names_reserved(owner: *const crate::name_storage::Names) -> usize {
    if owner.is_null() {
        return 0;
    }
    // SAFETY: live shared owner, no concurrent mutation or destruction.
    unsafe { (&*owner).reserved_bytes() }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_pause(
    input: *const WChar,
    length: usize,
    start: usize,
    rate: i32,
    multiplier: i32,
    space: Option<SsmlSpace>,
    output: *mut crate::ssml_clause::Break,
) -> i32 {
    if output.is_null() {
        return 1;
    }
    let Some(space) = space else {
        return 1;
    };
    // SAFETY: immutable initialized tag stays alive across pure classifiers,
    // which cannot mutate/invalidate/reenter; output is exclusive/disjoint.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 1;
    };
    let result = crate::ssml_clause::pause(input, start, rate, multiplier, |c| {
        // SAFETY: synchronous pure locale classifier.
        unsafe { space(c) != 0 }
    });
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive initialized disjoint effect, published after admission.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_pause_finish(
    input: *const crate::ssml_clause::Break,
    clause_pause: i32,
    pause: i32,
    sonic: u32,
    output: *mut i32,
) -> i32 {
    if input.is_null() || output.is_null() || sonic > 1 {
        return 1;
    }
    // SAFETY: initialized immutable effect, exclusive disjoint output.
    let result = unsafe { (&*input).finish(clause_pause, pause, sonic != 0) };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: admitted exclusive disjoint scalar effect.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_voice_clause(
    kind: i32,
    frames: *const crate::ssml_voice::Frame,
    count: i32,
    output: *mut crate::ssml_clause::VoicePlan,
) -> i32 {
    if frames.is_null() || output.is_null() || !(1..=20).contains(&count) {
        return 1;
    }
    let mut kinds = [0i32; 20];
    for (index, slot) in kinds[..count as usize].iter_mut().enumerate() {
        // SAFETY: read only each initialized frame-kind field. Compatibility
        // storage may have uninitialized name/property tails; never borrow the
        // whole record or read those fields. No callbacks or concurrent writes.
        *slot = unsafe { ptr::addr_of!((*frames.add(index)).kind).read() };
    }
    let Ok(result) = crate::ssml_clause::voice(kind, &kinds[..count as usize]) else {
        return 1;
    };
    // SAFETY: exclusive initialized output disjoint from all frame kinds.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_voice_clause_finish(
    input: *const crate::ssml_clause::VoicePlan,
    flags: i32,
    output: *mut i32,
) -> i32 {
    if input.is_null() || output.is_null() {
        return 1;
    }
    // SAFETY: initialized immutable plan, exclusive disjoint scalar output.
    let result = unsafe { (&*input).finish(flags) };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: admitted exclusive disjoint scalar effect.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_text(
    kind: i32,
    input: *const WChar,
    length: usize,
    start: usize,
    output: *mut u8,
    capacity: usize,
    state: *mut crate::ssml_text::State,
    wide_space: Option<SsmlSpace>,
    byte_space: Option<SsmlSpace>,
) -> i32 {
    if state.is_null() || output.is_null() || capacity > i32::MAX as usize {
        return 1;
    }
    let (Some(wide_space), Some(byte_space)) = (wide_space, byte_space) else {
        return 1;
    };
    // SAFETY: exclusive initialized disjoint state; immutable tag and initialized
    // output prefix remain alive across pure locale classifiers. No reentry.
    let (Some(input), snapshot) = (unsafe { (ssml_wide(input, length), *state) }) else {
        return 1;
    };
    let Ok(offset) = usize::try_from(snapshot.offset) else {
        return 1;
    };
    if offset > capacity {
        return 1;
    }
    let plan = {
        // SAFETY: only the initialized prefix is borrowed; unused output storage
        // is writable but may be uninitialized. Input/tag/state are disjoint.
        let prefix = unsafe { std::slice::from_raw_parts(output, offset) };
        let request = crate::ssml_text::Request {
            kind,
            input,
            start,
            prefix,
            capacity,
            state: snapshot,
        };
        let result = crate::ssml_text::plan(
            request,
            |c| {
                // SAFETY: pure wide locale classifier cannot mutate or reenter.
                unsafe { wide_space(c) != 0 }
            },
            |c| {
                // SAFETY: pure byte locale classifier receives 0..255.
                unsafe { byte_space(c) != 0 }
            },
        );
        let Ok(plan) = result else {
            return 1;
        };
        plan
    };
    // Plan retains the immutable tag copy source, never the output prefix. The
    // prefix borrow is no longer used. Complete capacity admission precedes all
    // sparse raw writes and state publication; no exclusive full-tail borrow.
    plan.emit(|index, bytes| {
        // SAFETY: admitted disjoint destination ranges in caller capacity.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), output.add(index), bytes.len());
        }
    });
    // SAFETY: exclusive disjoint state after output publication.
    unsafe {
        *state = plan.state;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_tag(
    input: *const WChar,
    length: usize,
    signed: u32,
    space: Option<SsmlSpace>,
    lower: Option<SsmlSpace>,
    output: *mut crate::ssml_control::Tag,
) -> i32 {
    if output.is_null() || signed > 1 {
        return 1;
    }
    let (Some(space), Some(lower)) = (space, lower) else {
        return 1;
    };
    // SAFETY: initialized immutable tag extent retained across pure classifiers.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 1;
    };
    let result = crate::ssml_control::tag(
        input,
        signed != 0,
        |c| {
            // SAFETY: pure locale wide classifier; no input mutation/reentry.
            unsafe { space(c) != 0 }
        },
        |c| {
            // SAFETY: pure locale byte lower classifier receives only 0..255.
            unsafe { lower(c) }
        },
    );
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive initialized disjoint effect after classifiers finish.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_directive(
    kind: i32,
    input: *const WChar,
    length: usize,
    start: usize,
    base: *const [i32; crate::ssml_parameters::PARAMETERS],
    current: *const [i32; crate::ssml_parameters::PARAMETERS],
    tone: i32,
    decimal: u32,
    space: Option<SsmlSpace>,
    output: *mut crate::ssml_parameters::Frame,
) -> i32 {
    if output.is_null() || base.is_null() || current.is_null() {
        return 1;
    }
    let Some(space) = space else {
        return 1;
    };
    // SAFETY: retained initialized immutable tag/base/current snapshots, disjoint
    // from exclusive output. Pure classifier cannot mutate/invalidate or reenter.
    let (Some(input), base, current) = (unsafe { (ssml_wide(input, length), &*base, &*current) })
    else {
        return 1;
    };
    let context = crate::ssml_control::Context {
        base,
        current,
        tone_language: tone,
        decimal,
    };
    let result = crate::ssml_control::directive(kind, input, start, context, |c| {
        // SAFETY: synchronous pure host locale wide classifier.
        unsafe { space(c) != 0 }
    });
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: same exclusive initialized disjoint frame effect after admission.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_voice_frame(
    input: *const WChar,
    length: usize,
    start: usize,
    kind: i32,
    count: i32,
    wide_space: Option<SsmlSpace>,
    byte_space: Option<SsmlSpace>,
    output: *mut crate::ssml_voice::FrameChange,
) -> i32 {
    if output.is_null() || count < 1 {
        return 1;
    }
    let (Some(wide_space), Some(byte_space)) = (wide_space, byte_space) else {
        return 1;
    };
    // SAFETY: shared initialized immutable tag span retained across classifiers.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 1;
    };
    let result = crate::ssml_voice::frame_change(
        input,
        start,
        kind,
        count as usize,
        |c| {
            // SAFETY: synchronous pure locale classifier, no reentry/invalidation.
            unsafe { wide_space(c) != 0 }
        },
        |c| {
            // SAFETY: synchronous pure byte classifier, receives only 0..255.
            unsafe { byte_space(c) != 0 }
        },
    );
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive initialized disjoint effect after all planning ends.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_voice_changed(
    current: *mut u8,
    selected: *const c_char,
) -> i32 {
    if current.is_null() || selected.is_null() {
        return -1;
    }
    // SAFETY: caller retains initialized terminated current/selected prefixes;
    // exclusive current capacity40 is admitted, its unused tail may be uninitialized.
    let result = unsafe {
        crate::ssml_voice::voice_changed(
            CStr::from_ptr(current.cast()).to_bytes_with_nul(),
            CStr::from_ptr(selected).to_bytes(),
        )
    };
    match result {
        Ok(Some(bytes)) => {
            let length = bytes
                .iter()
                .position(|b| *b == 0)
                .expect("validated identifier");
            // SAFETY: only admitted prefix+NUL is written after input borrows end.
            // No byte in the unused current tail is read or written.
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), current, length + 1);
            }
            1
        }
        Ok(None) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_float(
    input: *const WChar,
    length: usize,
    decimal: u32,
    space: Option<SsmlSpace>,
    output: *mut f64,
    tail: *mut usize,
) -> i32 {
    if output.is_null() || tail.is_null() {
        return 2;
    }
    let Some(space) = space else {
        return 2;
    };
    // SAFETY: shared initialized immutable input retained across pure classifier.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 2;
    };
    let result = crate::ssml_prosody::number(input, 0, decimal, |c| {
        // SAFETY: synchronous pure host locale classifier, no input invalidation.
        unsafe { space(c) != 0 }
    });
    match result {
        Ok(Some((number, index))) => {
            // SAFETY: exclusive initialized disjoint number/tail outputs.
            unsafe {
                *output = number;
                *tail = index;
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_prosody(
    param: i32,
    input: *const WChar,
    length: usize,
    decimal: u32,
    space: Option<SsmlSpace>,
    output: *mut crate::ssml_prosody::Value,
) -> i32 {
    if output.is_null() {
        return 1;
    }
    let Some(space) = space else {
        return 1;
    };
    // SAFETY: shared initialized immutable wide span retained across classifier.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 1;
    };
    let result = crate::ssml_prosody::value(param, input, decimal, |c| {
        // SAFETY: synchronous pure host locale classifier, no reentry/mutations.
        unsafe { space(c) != 0 }
    });
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive initialized disjoint effect after complete validation.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_prosody_parameter(
    param: i32,
    input: *const WChar,
    length: usize,
    base: i32,
    current: i32,
    decimal: u32,
    space: Option<SsmlSpace>,
    output: *mut i32,
) -> i32 {
    if output.is_null() || !(1..=4).contains(&param) {
        return 1;
    }
    let Some(space) = space else {
        return 1;
    };
    // SAFETY: shared initialized immutable wide span retained across classifier.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return 1;
    };
    let result =
        crate::ssml_prosody::parameter(param as usize, input, base, current, decimal, |c| {
            // SAFETY: synchronous pure host locale classifier, no reentry/mutations.
            unsafe { space(c) != 0 }
        });
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive initialized disjoint scalar output after all validation.
    unsafe {
        *output = result;
    }
    0
}
type SsmlResolveName = unsafe extern "C" fn(*const [u8; 40], *mut [u8; 40]) -> i32;

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_voice_choice(
    frames: *const crate::ssml_voice::Frame,
    count: i32,
    base: *const ForeignVoice,
    previous: *const [u8; 40],
    resolve: Option<SsmlResolveName>,
    output: *mut crate::ssml_voice::Choice,
) -> i32 {
    if frames.is_null()
        || base.is_null()
        || previous.is_null()
        || output.is_null()
        || count < 1
        || count as usize > crate::ssml_voice::STACK
    {
        return 1;
    }
    let Some(resolve) = resolve else {
        return 1;
    };
    // SAFETY: caller retains initialized immutable frames, base voice's terminated
    // names/packed list and prior identifier; all remain alive/immutable across
    // ordered resolution. Lookup must not reenter/invalidate those snapshots.
    // No exclusive Rust engine/catalogue owner is borrowed across callbacks.
    let (frames, base, previous) = unsafe {
        (
            std::slice::from_raw_parts(frames, count as usize),
            borrowed_voice(&*base),
            &*previous,
        )
    };
    let Some(base) = base else {
        return 1;
    };
    let result = crate::ssml_voice::choice(frames, base.languages, previous, |name| {
        let mut identifier = [0; 40];
        // SAFETY: synchronous callback borrows immutable terminated name and an
        // exclusive initialized/disjoint local identifier, copying before return.
        match unsafe { resolve(name, &mut identifier) } {
            0 => Ok(Some(identifier)),
            1 => Ok(None),
            _ => Err(crate::ssml_voice::Error::Resolver),
        }
    });
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: exclusive disjoint output after all callbacks and validation end.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_base_variant(
    selected: *const c_char,
    gender: u32,
    base_gender: u32,
    variant: *const c_char,
    output: *mut [u8; 40],
) -> i32 {
    if selected.is_null()
        || variant.is_null()
        || output.is_null()
        || gender > 255
        || base_gender > 255
    {
        return -1;
    }
    // SAFETY: initialized immutable terminated inputs, disjoint from exclusive
    // fixed output. No callbacks/mutations occur during complete variant planning.
    let result = unsafe {
        crate::ssml_voice::base_variant(
            CStr::from_ptr(selected).to_bytes(),
            gender as u8,
            base_gender as u8,
            CStr::from_ptr(variant).to_bytes(),
        )
    };
    let Some(result) = result else {
        return 0;
    };
    // SAFETY: same initialized exclusive disjoint 40-byte output after planning.
    unsafe {
        *output = result;
    }
    1
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_parameters(
    frames: *const crate::ssml_parameters::Frame,
    count: i32,
    current: *const [i32; crate::ssml_parameters::PARAMETERS],
    punctuation: i32,
    capitals: i32,
    pop: u32,
    kind: i32,
    capacity: usize,
    output: *mut crate::ssml_parameters::Effects,
) -> i32 {
    if frames.is_null()
        || current.is_null()
        || output.is_null()
        || pop > 1
        || count < 0
        || count as usize > crate::ssml_parameters::STACK
    {
        return 1;
    }
    // SAFETY: shared initialized count-frame extent and current snapshot;
    // retained/disjoint output, with no callbacks, mutations or reentry in plan.
    let (frames, current) = unsafe {
        (
            std::slice::from_raw_parts(frames, count as usize),
            &*current,
        )
    };
    let result = if pop == 0 {
        crate::ssml_parameters::parameters(frames, current, punctuation, capitals)
    } else {
        crate::ssml_parameters::pop(frames, kind, current, punctuation, capitals)
    };
    let Ok(result) = result else {
        return 1;
    };
    if result.changed != 0 && capacity <= result.length as usize {
        return 1;
    }
    // SAFETY: exclusive disjoint effect output after complete capacity admission.
    unsafe {
        *output = result;
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_push(
    frames: *mut crate::ssml_parameters::Frame,
    count: *mut i32,
    kind: i32,
) -> i32 {
    if frames.is_null() || count.is_null() {
        return -1;
    }
    // SAFETY: exclusive initialized/disjoint count, validated before any writes.
    let mut active = match usize::try_from(unsafe { *count }) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    if active >= crate::ssml_parameters::STACK {
        return -1;
    }
    // SAFETY: caller retains exclusive initialized 20-frame stack, disjoint from
    // count and all other engine state. Native push has no callbacks/reentry.
    let result = unsafe {
        crate::ssml_parameters::push(
            std::slice::from_raw_parts_mut(frames, crate::ssml_parameters::STACK),
            &mut active,
            kind,
        )
    };
    let Ok(index) = result else {
        return -1;
    };
    // SAFETY: same exclusive initialized count after successful frame publication.
    unsafe {
        *count = active as i32;
    }
    index as i32
}

unsafe fn ssml_wide<'a>(input: *const WChar, length: usize) -> Option<crate::ssml::Wide<'a>> {
    if input.is_null() || length > isize::MAX as usize / std::mem::size_of::<WChar>() {
        return None;
    }
    // SAFETY: caller retains this initialized wide-unit span immutably for the
    // call; validated extent fits a Rust slice and follows host wchar_t width.
    let units = unsafe { std::slice::from_raw_parts(input, length) };
    #[cfg(windows)]
    let result = crate::ssml::Wide::U16(units);
    #[cfg(not(windows))]
    let result = crate::ssml::Wide::U32(units);
    Some(result)
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_compare(
    input: *const WChar,
    length: usize,
    name: *const c_char,
) -> c_int {
    if name.is_null() {
        return 1;
    }
    // SAFETY: shared initialized wide extent and terminated ASCII mnemonic.
    let (input, name) = unsafe { (ssml_wide(input, length), CStr::from_ptr(name).to_bytes()) };
    c_int::from(!name.is_ascii() || !crate::ssml::attribute_matches(input, name))
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_number(
    input: *const WChar,
    length: usize,
    default: i32,
    kind: i32,
) -> i32 {
    // SAFETY: optional shared initialized wide-unit extent supplied by C.
    let input = unsafe { ssml_wide(input, length) };
    crate::ssml::attribute_number(input, default, kind == 1).unwrap_or(default)
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_lookup(
    input: *const WChar,
    length: usize,
    mut table: *const Mnem,
) -> i32 {
    if table.is_null() {
        return -1;
    }
    // SAFETY: optional shared initialized wide-unit extent.
    let input = unsafe { ssml_wide(input, length) };
    for _ in 0..4096 {
        // SAFETY: caller retains a shared initialized mnemonic table ending at
        // its first NULL name; this loop never advances beyond that sentinel.
        let entry = unsafe { &*table };
        if entry.name.is_null() {
            return entry.value;
        }
        // SAFETY: every non-NULL mnemonic is an immutable terminated ASCII name.
        let name = unsafe { CStr::from_ptr(entry.name).to_bytes() };
        if name.is_ascii() && crate::ssml::attribute_matches(input, name) {
            return entry.value;
        }
        // SAFETY: non-sentinel entry is followed by another initialized entry.
        table = unsafe { table.add(1) };
    }
    -1
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_attribute(
    input: *const WChar,
    length: usize,
    start: usize,
    name: *const c_char,
    space: Option<SsmlSpace>,
    output: *mut usize,
) -> c_int {
    if name.is_null() || output.is_null() {
        return 2;
    }
    let Some(space) = space else {
        return 2;
    };
    // SAFETY: caller retains shared initialized text and terminated ASCII name;
    // classifier is synchronous/pure and cannot mutate or invalidate either.
    let (input, name) = unsafe { (ssml_wide(input, length), CStr::from_ptr(name).to_bytes()) };
    let Some(input) = input else {
        return 2;
    };
    if !name.is_ascii() {
        return 2;
    }
    let result = crate::ssml::attribute(input, start, name, |c| {
        // SAFETY: pure host locale classifier accepts any uint32 code unit.
        unsafe { space(c) != 0 }
    });
    match result {
        Ok(Some(index)) => {
            let index = match index {
                crate::ssml::Attribute::Value(index) => index,
                crate::ssml::Attribute::Empty => usize::MAX,
            };
            // SAFETY: exclusive initialized disjoint output, after parsing.
            unsafe {
                *output = index;
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_copy(
    input: *const WChar,
    length: usize,
    preceding: u32,
    space: Option<SsmlSpace>,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if output.is_null() || capacity > i32::MAX as usize {
        return -1;
    }
    let Some(space) = space else {
        return -1;
    };
    // SAFETY: initialized shared source extent, immutable across pure classifier.
    let Some(input) = (unsafe { ssml_wide(input, length) }) else {
        return -1;
    };
    let Ok(plan) = crate::ssml::copy_plan(input, preceding, capacity, |c| {
        // SAFETY: pure locale byte classifier receives only 0..255.
        unsafe { space(c) != 0 }
    }) else {
        return -1;
    };
    let mut offset = 0;
    plan.emit(|bytes| {
        // SAFETY: caller admits exclusive disjoint capacity writable bytes;
        // validated plan writes only encoded prefix plus NUL, never borrows tail.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), output.add(offset), bytes.len());
        }
        offset += bytes.len();
    });
    plan.length() as i32
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_reference(
    input: *const c_char,
    first: *mut i32,
    second: *mut i32,
    space: Option<SsmlSpace>,
) -> i32 {
    if input.is_null() || first.is_null() || second.is_null() {
        return -1;
    }
    let Some(space) = space else {
        return -1;
    };
    // SAFETY: shared terminated input; initialized exclusive/disjoint first and
    // second snapshots, no mutation during the pure locale classifier calls.
    let result = unsafe {
        crate::ssml::reference(CStr::from_ptr(input).to_bytes(), *first, *second, |c| {
            space(c) != 0
        })
    };
    let Ok(result) = result else {
        return -1;
    };
    // SAFETY: same disjoint exclusive initialized outputs after full validation.
    unsafe {
        *first = result.first;
        *second = result.second;
    }
    result.status
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_ssml_key(input: *mut u8, index: i32, outix: *mut i32) -> i32 {
    if input.is_null() || index < 0 || outix.is_null() {
        return 0;
    }
    // SAFETY: shared initialized terminated key string during planning.
    let name = unsafe { CStr::from_ptr(input.cast()).to_bytes() };
    let Some((bytes, length, code)) = crate::ssml::key_name(name) else {
        return 0;
    };
    let Some(next) = index.checked_add(length as i32) else {
        return 0;
    };
    if length > name.len() {
        return 0;
    }
    // SAFETY: caller retains exclusive key bytes and initialized/disjoint outix;
    // replacement fits the original prefix, no tail or terminator is modified.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), input, length);
        *outix = next;
    }
    code
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_pitch(
    voice: *const crate::voice::Voice,
    first: i32,
    second: i32,
    embedded: *const crate::synthesis_parameters::Embedded,
    output: *mut crate::synthesis_parameters::Pitch,
) -> c_int {
    if voice.is_null() || embedded.is_null() || output.is_null() {
        return 1;
    }
    // SAFETY: retained immutable initialized voice/embedded snapshots and
    // exclusive disjoint output. No callbacks/reentry or mutations in planning.
    let result = unsafe { crate::synthesis_parameters::pitch(&*voice, first, second, *embedded) };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: same exclusive initialized disjoint output after validation.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_pitch_formants(
    voice: *mut crate::voice::Voice,
    pitch: i32,
    tone: i32,
) -> c_int {
    if voice.is_null() {
        return 1;
    }
    // SAFETY: exclusive initialized acoustic snapshot, no aliases/callbacks
    // during transactional bounded frequency/height planning and mutation.
    if unsafe { crate::synthesis_parameters::pitch_formants(&mut *voice, pitch, tone) }.is_ok() {
        0
    } else {
        1
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_general_amplitude(
    amplitude: i32,
    emphasis: i32,
    output: *mut i32,
) -> c_int {
    if output.is_null() {
        return 1;
    }
    let Ok(emphasis) = usize::try_from(emphasis) else {
        return 1;
    };
    let Ok(result) = crate::synthesis_parameters::general_amplitude(amplitude, emphasis) else {
        return 1;
    };
    // SAFETY: exclusive writable disjoint scalar output; no callbacks occur.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_amplitude(
    length: i32,
    value: i32,
    general: i32,
    consonant: i32,
    output: *mut crate::synthesis_parameters::Amplitude,
) -> c_int {
    if output.is_null() {
        return 1;
    }
    let Ok(result) = crate::synthesis_parameters::amplitude(length, value, general, consonant)
    else {
        return 1;
    };
    // SAFETY: initialized exclusive disjoint output; commit after validation.
    unsafe {
        *output = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_pitch(
    envelope: *const [u8; 128],
    number: i32,
    pitch: *const crate::synthesis_parameters::Pitch,
    split: i32,
    final_only: u32,
    output: *mut u8,
    capacity: usize,
) -> c_int {
    if envelope.is_null() || pitch.is_null() || output.is_null() || final_only > 1 {
        return 1;
    }
    // SAFETY: retained initialized immutable 128-byte envelope/pitch and
    // exclusive disjoint writable output capacity. No callbacks/I/O in planning.
    let result = unsafe {
        crate::mbrola_output::pitch_text(&*envelope, number, *pitch, split, final_only != 0)
    };
    let Ok(result) = result else {
        return 1;
    };
    if result.terminated().len() > capacity {
        return 1;
    }
    // SAFETY: admitted writable output prefix; input borrows have ended.
    unsafe {
        ptr::copy_nonoverlapping(
            result.terminated().as_ptr(),
            output,
            result.terminated().len(),
        );
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_scale(
    bytes: *mut u8,
    length: usize,
    amplitude: i32,
) -> c_int {
    if bytes.is_null() || length > isize::MAX as usize {
        return 1;
    }
    // SAFETY: exclusively owned initialized PCM returned by the backend read;
    // that callback finished before this borrow. No callbacks or I/O occur.
    if unsafe {
        crate::mbrola_output::scale_pcm(std::slice::from_raw_parts_mut(bytes, length), amplitude)
    }
    .is_ok()
    {
        0
    } else {
        1
    }
}
#[no_mangle]
extern "C" fn espeak_rs_mbrola_create() -> *mut crate::mbrola::Table {
    Box::into_raw(Box::new(crate::mbrola::Table::default()))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_destroy(owner: *mut crate::mbrola::Table) {
    if !owner.is_null() {
        // SAFETY: serialized unique live owner transferred once after borrows drain.
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_load(
    owner: *mut crate::mbrola::Table,
    path: *const c_char,
    control: *mut u32,
    error: *mut i32,
) -> c_int {
    if owner.is_null() || path.is_null() || control.is_null() || error.is_null() {
        return 2;
    }
    // SAFETY: unique initialized owner, terminated disjoint path and exclusive
    // initialized/disjoint outputs. No retained mapping consumers during load.
    let result = unsafe {
        crate::voice_storage::compat_path(CStr::from_ptr(path).to_bytes())
            .and_then(|path| (&mut *owner).load(&path))
    };
    // SAFETY: same exclusive outputs. Failed loads preserve control/table.
    unsafe {
        *error = 0;
    }
    let Err(failure) = result else {
        // SAFETY: retained initialized owner and exclusive disjoint output.
        unsafe {
            *control = (*owner).control();
        }
        return 0;
    };
    // SAFETY: exclusive error output. Normalize portable I/O kinds to the C
    // errno vocabulary when a platform's raw error is not a Unix errno.
    unsafe {
        *error = match failure.kind() {
            std::io::ErrorKind::NotFound => 2,
            std::io::ErrorKind::PermissionDenied => 13,
            std::io::ErrorKind::IsADirectory => 21,
            std::io::ErrorKind::OutOfMemory => 12,
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => 22,
            _ => 5,
        };
    }
    #[cfg(unix)]
    // SAFETY: exclusive error output; Unix raw OS errors correspond to errno.
    unsafe {
        if let Some(errno) = failure.raw_os_error() {
            *error = errno;
        }
    }
    match failure.kind() {
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::InvalidInput => 2,
        std::io::ErrorKind::OutOfMemory => 3,
        std::io::ErrorKind::WouldBlock => 4,
        _ => 1,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_view(
    owner: *const crate::mbrola::Table,
    mappings: *mut *const crate::mbrola::Mapping,
    count: *mut usize,
    control: *mut u32,
) -> c_int {
    if owner.is_null() || mappings.is_null() || count.is_null() || control.is_null() {
        return 1;
    }
    // SAFETY: shared live owner and exclusive initialized disjoint outputs.
    // Published immutable records expire on successful reload/destruction.
    unsafe {
        *mappings = (*owner).mappings().as_ptr();
        *count = (*owner).mappings().len();
        *control = (*owner).control();
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_select(
    owner: *const crate::mbrola::Table,
    current: *const crate::phoneme::Phoneme,
    previous: *const crate::phoneme::Phoneme,
    next: *const crate::phoneme::Phoneme,
    pause: *const crate::phoneme::Phoneme,
    context: *const crate::mbrola::Context,
    selection: *mut crate::mbrola::Selection,
) -> c_int {
    if current.is_null() || context.is_null() || selection.is_null() {
        return 1;
    }
    // SAFETY: shared optional live owner, aligned retained initialized records
    // and context, exclusive disjoint output. Optional neighbor/pause pointers
    // may be NULL. No callbacks, mutable owner borrow or reentry occurs.
    let result = unsafe {
        crate::mbrola::select(
            owner.as_ref().map_or(&[], |owner| owner.mappings()),
            &*current,
            previous.as_ref(),
            next.as_ref(),
            pause.as_ref(),
            &*context,
        )
    };
    // SAFETY: same initialized exclusive disjoint output after planning.
    unsafe {
        *selection = result;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_change_stress(
    word: *mut u8,
    length: usize,
    table: *const *const crate::phoneme::Phoneme,
    flags: u32,
    level: i32,
) -> c_int {
    if word.is_null() || length == 0 || length > crate::word_stress::WORD_BYTES {
        return 1;
    }
    // SAFETY: initialized terminated input prefix and retained immutable sparse
    // table. Borrowed input/table are disjoint; no mutations/callbacks in plan.
    let result = unsafe {
        let Some(table) = borrowed_phonemes(table, 256) else {
            return 1;
        };
        crate::word_stress::change(
            std::slice::from_raw_parts(word, length),
            &table,
            flags,
            level,
        )
    };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: owner admits exclusive writable word storage for up to 200 bytes;
    // input borrows ended. Publish only prefix, without borrowing unused tail.
    unsafe {
        ptr::copy_nonoverlapping(result.phonemes.as_ptr(), word, result.length + 1);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_append_phonemes(
    word: *mut u8,
    length: usize,
    capacity: usize,
    addition: *const u8,
    addition_length: usize,
    table: *const *const crate::phoneme::Phoneme,
    table_count: usize,
    counts: *mut crate::phoneme_word::Counts,
) -> c_int {
    if word.is_null()
        || addition.is_null()
        || counts.is_null()
        || capacity > isize::MAX as usize
        || addition_length > isize::MAX as usize
    {
        return 1;
    }
    // SAFETY: retained immutable initialized addition/table and initialized
    // disjoint counts. Word capacity is writable; tail need not be initialized.
    // Addition is disjoint from word, retaining the C strcat caller contract.
    let result = unsafe {
        let Some(table) = borrowed_phonemes(table, 256) else {
            return 1;
        };
        crate::phoneme_word::plan_append(
            length,
            std::slice::from_raw_parts(addition, addition_length),
            capacity,
            &table,
            table_count,
            *counts,
        )
    };
    let Ok(plan) = result else {
        return 1;
    };
    if let Some(plan) = plan {
        // SAFETY: admitted bounded exclusive output and disjoint immutable tail;
        // exclusive count effects commit after all validation has succeeded.
        unsafe {
            ptr::copy_nonoverlapping(plan.tail.as_ptr(), word.add(plan.offset), plan.tail.len());
            *counts = plan.counts;
        }
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_special_attribute(
    word: *mut u8,
    length: usize,
    table: *const *const crate::phoneme::Phoneme,
    table_count: usize,
    options: i32,
    flags: u32,
    signed_bytes: u32,
) -> c_int {
    if word.is_null() || length > isize::MAX as usize || signed_bytes > 1 {
        return 1;
    }
    // SAFETY: exclusive initialized terminated word prefix, immutable disjoint
    // selected pointer slots/records; no reentry or callbacks occur.
    let result = unsafe {
        let Some(table) = borrowed_phonemes(table, 256) else {
            return 1;
        };
        crate::phoneme_word::special_attribute(
            std::slice::from_raw_parts_mut(word, length),
            options,
            flags,
            &table,
            table_count,
            signed_bytes != 0,
        )
    };
    if result.is_ok() {
        0
    } else {
        1
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_vowel_stress(
    word: *mut u8,
    length: usize,
    table: *const *const crate::phoneme::Phoneme,
    flags: u32,
    control: u32,
    vowel_stress: *mut i8,
    count: *mut i32,
    primary: *mut i32,
    maximum: *mut i32,
) -> c_int {
    if word.is_null()
        || length == 0
        || length > crate::word_stress::WORD_BYTES
        || vowel_stress.is_null()
        || count.is_null()
        || primary.is_null()
        || maximum.is_null()
    {
        return 1;
    }
    // SAFETY: initialized input prefix, 256 immutable selected pointer slots/
    // records and disjoint initialized primary. No callbacks or mutations until
    // the owned plan completes; all input borrows end before publication.
    let result = unsafe {
        let Some(table) = borrowed_phonemes(table, 256) else {
            return 1;
        };
        crate::word_stress::extract(
            std::slice::from_raw_parts(word, length),
            &table,
            flags,
            *primary,
            control,
        )
    };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: serialized exclusive word prefix and disjoint 100-byte stress/
    // integer outputs. Write only completed prefixes, retaining caller tails.
    unsafe {
        ptr::copy_nonoverlapping(result.phonemes.as_ptr(), word, result.length + 1);
        ptr::copy_nonoverlapping(result.stress.as_ptr(), vowel_stress, result.count + 1);
        *count = result.count as i32;
        *primary = result.primary as i32;
        *maximum = result.maximum;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_word_stress(
    word: *mut u8,
    length: usize,
    table: *const *const crate::phoneme::Phoneme,
    table_count: usize,
    settings: *const crate::word_stress::Settings,
    dictionary: *const u32,
    tonic: i32,
    control: u32,
    previous: *mut i32,
) -> c_int {
    if word.is_null()
        || length == 0
        || length > crate::word_stress::WORD_BYTES
        || settings.is_null()
        || previous.is_null()
    {
        return 1;
    }
    // SAFETY: retained immutable selected table, initialized settings/optional
    // dictionary and initialized terminated input prefix; outputs disjoint.
    // No mutable owner borrow/callback occurs during planning.
    let result = unsafe {
        let Some(table) = borrowed_phonemes(table, 256) else {
            return 1;
        };
        crate::word_stress::assign(
            std::slice::from_raw_parts(word, length),
            &table,
            table_count,
            &*settings,
            dictionary.as_ref().copied(),
            tonic,
            control,
        )
    };
    let Ok(result) = result else {
        return 1;
    };
    // SAFETY: caller admits up to 200 writable word bytes, without requiring
    // their unused tail to be initialized. Exclusive previous effect disjoint.
    unsafe {
        ptr::copy_nonoverlapping(result.phonemes.as_ptr(), word, result.length + 1);
        *previous = result.previous;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_utf8_out(code: u32, output: *mut u8) -> c_int {
    if output.is_null() {
        return 0;
    }
    let (bytes, length) = crate::suffix::encode(code);
    // SAFETY: caller retains exclusive storage for the encoded character's
    // 1..4 bytes. No byte outside that length is borrowed or written.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output, length);
    }
    length as c_int
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_remove_ending(
    word: *mut u8,
    length: usize,
    ending: u32,
    context: *const crate::suffix::Context,
    letters: *const RawLetters,
    copy: *mut u8,
    effects: *mut crate::suffix::Effects,
) -> c_int {
    if word.is_null()
        || context.is_null()
        || letters.is_null()
        || effects.is_null()
        || length > isize::MAX as usize
    {
        return 1;
    }
    // SAFETY: immutable initialized disjoint context/letter arrays, unique
    // initialized writable word span and exclusive disjoint outputs. Optional
    // copy has 160 writable bytes; no callbacks or reentry occur.
    let Some(letters) = (unsafe { (&*letters).borrow() }) else {
        return 1;
    };
    // SAFETY: word is an initialized exclusive span; context is readable and
    // disjoint from it and the immutable letter-set backing storage.
    let result = unsafe {
        crate::suffix::remove(
            std::slice::from_raw_parts_mut(word, length),
            ending,
            &*context,
            &letters,
        )
    };
    let Ok(outcome) = result else {
        return 1;
    };
    // SAFETY: exclusive effects and optional writable disjoint copy; write only
    // its completed original-word prefix, preserving the compatibility tail.
    unsafe {
        *effects = outcome.effects;
        if !copy.is_null() {
            ptr::copy_nonoverlapping(outcome.original.as_ptr(), copy, outcome.original_length + 1);
        }
    }
    0
}
#[repr(C)]
struct SoundIconView {
    name: i32,
    samples: i32,
    data: *const u8,
    filename: *const u8,
}
unsafe fn sound_icon_views(
    owner: &crate::sound_icons::Catalog,
    views: *mut SoundIconView,
    count: *mut c_int,
) {
    // SAFETY: adapter retains unique initialized disjoint 80-slot view storage
    // and count; immutable byte/name borrows drain before owner destruction.
    unsafe {
        for index in 0..crate::sound_icons::MAX_ICONS {
            let view = match owner.icon(index) {
                Some(icon) => SoundIconView {
                    name: icon.name,
                    samples: icon.samples,
                    data: if icon.bytes.is_empty() {
                        ptr::null()
                    } else {
                        icon.bytes.as_ptr()
                    },
                    filename: owner.filename_c(index).as_ptr(),
                },
                None => SoundIconView {
                    name: 0,
                    samples: 0,
                    data: ptr::null(),
                    filename: ptr::null(),
                },
            };
            views.add(index).write(view);
        }
        *count = owner.len() as c_int;
    }
}
#[no_mangle]
extern "C" fn espeak_rs_soundicons_create() -> *mut crate::sound_icons::Catalog {
    match crate::sound_icons::Catalog::new(crate::core_storage::MAX_BYTES) {
        Ok(owner) => Box::into_raw(Box::new(owner)),
        Err(_) => ptr::null_mut(),
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_soundicons_destroy(owner: *mut crate::sound_icons::Catalog) {
    if !owner.is_null() {
        // SAFETY: caller transfers unique live owner once after PCM/views drain.
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_soundicons_configure(
    owner: *mut crate::sound_icons::Catalog,
    path: *const c_char,
    points: *mut [i32; 12],
    width: usize,
    signed_character: u32,
    views: *mut SoundIconView,
    count: *mut c_int,
) -> c_int {
    if owner.is_null()
        || path.is_null()
        || points.is_null()
        || views.is_null()
        || count.is_null()
        || signed_character > 1
    {
        return 1;
    }
    // SAFETY: serialized unique live owner/points, shared terminated path,
    // initialized exclusive disjoint views/count. No callbacks or reentry.
    let result = unsafe {
        crate::voice_storage::compat_path(CStr::from_ptr(path).to_bytes())
            .and_then(std::fs::File::open)
            .and_then(|file| {
                crate::voice_reader::Reader::new(
                    file,
                    width,
                    crate::voice_reader::TextMode::platform(),
                )
            })
            .and_then(|mut reader| {
                crate::sound_icons::configure(
                    &mut reader,
                    &mut *points,
                    &mut *owner,
                    signed_character != 0,
                )
            })
    };
    // SAFETY: initialized exclusive disjoint outputs; publish completed prefix.
    unsafe {
        sound_icon_views(&*owner, views, count);
    }
    c_int::from(result.is_err())
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_soundicons_lookup(
    owner: *mut crate::sound_icons::Catalog,
    root: *const c_char,
    filename: *const c_char,
    character: i32,
    rate: i32,
    separator: u32,
    width: usize,
    views: *mut SoundIconView,
    count: *mut c_int,
) -> c_int {
    if owner.is_null()
        || root.is_null()
        || views.is_null()
        || count.is_null()
        || separator > 255
        || !(2..=4096).contains(&width)
    {
        return -1;
    }
    // Snapshot an optional filename before borrowing the owner exclusively:
    // compatibility callers may pass a name borrowed from its published views.
    let mut filename_copy = [0; 4096];
    let filename_length = if filename.is_null() {
        0
    } else {
        // SAFETY: optional input is a retained shared terminated string; owner
        // mutation has not begun, including when this string aliases a name.
        let bytes = unsafe { CStr::from_ptr(filename).to_bytes() };
        if bytes.len() > 4095 {
            return -1;
        }
        filename_copy[..bytes.len()].copy_from_slice(bytes);
        bytes.len()
    };
    // SAFETY: serialized unique live owner and terminated root disjoint from it;
    // initialized exclusive outputs disjoint from owner/paths.
    let owner = unsafe { &mut *owner };
    let result = (|| {
        let name = if filename.is_null() {
            let index = owner
                .find_name(character)
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
            if owner.icon(index).unwrap().samples != 0 {
                return Ok(index);
            }
            // Reusable bounded name copy ends its borrow before owner mutation.
            let mut name = [0; 4096];
            let bytes = owner.icon(index).unwrap().filename;
            name[..bytes.len()].copy_from_slice(bytes);
            let length = bytes.len();
            // SAFETY: terminated root is shared and retained through this call.
            let path = crate::sound_icons::path(
                unsafe { CStr::from_ptr(root).to_bytes() },
                &name[..length],
                separator as u8,
                width,
            )?;
            return owner.load_icon(index, &path, rate);
        } else {
            &filename_copy[..filename_length]
        };
        if let Some(index) = owner.find_file(name) {
            if owner.icon(index).unwrap().samples != 0 {
                return Ok(index);
            }
        }
        // SAFETY: terminated root remains readable and disjoint from the owner.
        let path = crate::sound_icons::path(
            unsafe { CStr::from_ptr(root).to_bytes() },
            name,
            separator as u8,
            width,
        )?;
        owner.load_file(name, &path, rate)
    })();
    // SAFETY: initialized exclusive disjoint views and count are retained.
    unsafe {
        sound_icon_views(owner, views, count);
    }
    result.map_or(-1, |index| index as c_int)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_speed_configure(
    voice: *const crate::voice::Voice,
    factors: *mut crate::speed::Factors,
    lengths: *mut [i32; 3],
    primary: i32,
    secondary: i32,
    control: u32,
    sonic: u32,
    effects: *mut crate::speed::SonicEffects,
) -> c_int {
    if voice.is_null() || factors.is_null() || lengths.is_null() || effects.is_null() || sonic > 1 {
        return 1;
    }
    // SAFETY: owner supplies a live shared voice and initialized exclusive
    // disjoint factor/length/effect outputs. No callbacks or reentry occur.
    let mut state = unsafe {
        crate::speed::State {
            factors: *factors,
            lengths: *lengths,
        }
    };
    // SAFETY: voice is readable and disjoint from mutable outputs.
    let Ok(plan) = state.configure(unsafe { &*voice }, primary, secondary, control, sonic != 0)
    else {
        return 1;
    };
    // SAFETY: initialized exclusive outputs with matching repr(C) layouts.
    unsafe {
        *factors = state.factors;
        *lengths = state.lengths;
        *effects = plan;
    }
    0
}
type DictionaryHandle = std::sync::Arc<crate::dictionary_storage::Snapshot>;
#[no_mangle]
extern "C" fn espeak_rs_dictionary_cache_create() -> *mut crate::dictionary_storage::Cache {
    match crate::dictionary_storage::Cache::new(crate::core_storage::MAX_BYTES) {
        Ok(cache) => Box::into_raw(Box::new(cache)),
        Err(_) => ptr::null_mut(),
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_dictionary_cache_destroy(
    cache: *mut crate::dictionary_storage::Cache,
) {
    if !cache.is_null() {
        // SAFETY: caller transfers the live unique cache once after loads drain.
        unsafe {
            drop(Box::from_raw(cache));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_dictionary_handle_destroy(handle: *mut DictionaryHandle) {
    if !handle.is_null() {
        // SAFETY: unique live handle is transferred once, after its views drain.
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_dictionary_cache_load(
    cache: *mut crate::dictionary_storage::Cache,
    path: *const c_char,
    handle: *mut *mut DictionaryHandle,
) -> c_int {
    if cache.is_null() || path.is_null() || handle.is_null() {
        return 2;
    }
    // SAFETY: retained unique serialized cache, terminated compatibility path,
    // initialized exclusive disjoint handle output. No callbacks or reentry.
    let result = unsafe {
        crate::voice_storage::compat_path(CStr::from_ptr(path).to_bytes())
            .map_err(crate::dictionary_storage::Error::Io)
            .and_then(|path| (&mut *cache).load(&path))
    };
    use crate::dictionary_storage::Error;
    match result {
        Ok(snapshot) => {
            // SAFETY: retained initialized exclusive output; caller owns the new
            // small handle and releases any previous handle after view rebinding.
            unsafe {
                *handle = Box::into_raw(Box::new(snapshot));
            }
            0
        }
        Err(Error::Empty) => 1,
        Err(Error::Invalid(_)) => 2,
        Err(Error::Io(error)) => match error.kind() {
            std::io::ErrorKind::OutOfMemory => 3,
            std::io::ErrorKind::InvalidData | std::io::ErrorKind::WouldBlock => 4,
            _ => 1,
        },
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_dictionary_handle_view(
    handle: *const DictionaryHandle,
    bytes: *mut *const u8,
    length: *mut usize,
    rules: *mut crate::rules::RuleIndex,
    buckets: *mut usize,
    offset: *mut usize,
) -> c_int {
    if handle.is_null()
        || bytes.is_null()
        || length.is_null()
        || rules.is_null()
        || buckets.is_null()
        || offset.is_null()
    {
        return 2;
    }
    // SAFETY: retained immutable live handle/snapshot and exclusive initialized
    // disjoint outputs, with 1024 bucket slots. Views live until handle release;
    // consumers must not modify immutable bytes or retain output-array borrows.
    unsafe {
        let data = &*handle;
        *bytes = data.bytes().as_ptr();
        *length = data.bytes().len();
        rules.write(data.rules().clone());
        ptr::copy_nonoverlapping(data.buckets().as_ptr(), buckets, crate::dictionary::BUCKETS);
        *offset = data.rules_offset();
    }
    0
}
#[no_mangle]
extern "C" fn espeak_rs_core_create() -> *mut crate::core_storage::Storage {
    Box::into_raw(Box::new(crate::core_storage::Storage::default()))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_core_destroy(storage: *mut crate::core_storage::Storage) {
    if !storage.is_null() {
        // SAFETY: caller transfers the unique live owner once after views drain.
        unsafe {
            drop(Box::from_raw(storage));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_core_load(
    storage: *mut crate::core_storage::Storage,
    slot: u32,
    path: *const c_char,
    bytes: *mut *mut u8,
    length: *mut i32,
    error: *mut i32,
) -> u32 {
    if storage.is_null() || path.is_null() || bytes.is_null() || length.is_null() || error.is_null()
    {
        return 5;
    }
    let Ok(slot) = crate::core_storage::Slot::try_from(slot) else {
        return 5;
    };
    // SAFETY: caller retains exclusive initialized owner and disjoint outputs,
    // terminated path and no live consumers of the replaced slot during load.
    let (result, view) = unsafe {
        let owner = &mut *storage;
        let result = crate::voice_storage::compat_path(CStr::from_ptr(path).to_bytes())
            .and_then(|path| owner.load(slot, &path));
        (result, owner.bytes(slot))
    };
    // SAFETY: live exclusive disjoint outputs. Storage words provide u64 alignment
    // and retained initialized bytes; C may borrow/mutate after this call returns.
    unsafe {
        *bytes = if view.is_empty() {
            ptr::null_mut()
        } else {
            view.as_ptr().cast_mut()
        };
        *length = view.len() as i32;
        *error = 0;
    }
    let Err(failure) = result else {
        return 0;
    };
    #[cfg(unix)]
    // SAFETY: same retained exclusive error output; Unix raw OS errors are errno.
    unsafe {
        *error = failure.raw_os_error().unwrap_or(0);
    }
    use std::io::ErrorKind;
    match failure.kind() {
        ErrorKind::NotFound => 1,
        ErrorKind::PermissionDenied => 2,
        ErrorKind::OutOfMemory => 3,
        ErrorKind::IsADirectory => 4,
        ErrorKind::InvalidInput | ErrorKind::InvalidData => 5,
        ErrorKind::UnexpectedEof => 6,
        _ => 7,
    }
}
#[no_mangle]
extern "C" fn espeak_rs_current_voice_create() -> *mut crate::voice_current::Current {
    Box::into_raw(Box::new(crate::voice_current::Current::default()))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_current_voice_destroy(current: *mut crate::voice_current::Current) {
    if !current.is_null() {
        // SAFETY: caller transfers the live unique owner once after all borrows drain.
        unsafe {
            drop(Box::from_raw(current));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_current_voice_prepare(
    current: *mut crate::voice_current::Current,
    requested: *const c_char,
    fallback: *const c_char,
    tone_only: u32,
    gender: u8,
    age: u8,
    language: *const [u8; 20],
    setup: *mut crate::voice_setup::Setup,
) -> c_int {
    if current.is_null() || fallback.is_null() || language.is_null() || setup.is_null() {
        return 2;
    }
    let mut request = [0; 41];
    let mut fallback_copy = [0; 40];
    // SAFETY: caller retains terminated input strings and initialized language.
    // Request may alias current identifier; snapshot before exclusive mutation.
    let (length, fallback_length, language) = unsafe {
        let input = if requested.is_null() {
            b"".as_slice()
        } else {
            CStr::from_ptr(requested).to_bytes()
        };
        let length = input.len().min(request.len());
        request[..length].copy_from_slice(&input[..length]);
        let input = CStr::from_ptr(fallback).to_bytes();
        if input.len() >= 40 {
            return 2;
        }
        fallback_copy[..input.len()].copy_from_slice(input);
        (length, input.len(), *language)
    };
    // SAFETY: serialized exclusive initialized owner/output, which are disjoint.
    // All possibly aliased input bytes have already been copied to local storage.
    let result = unsafe {
        (&mut *current).prepare(
            &request[..length],
            &fallback_copy[..fallback_length],
            tone_only != 0,
            gender,
            age,
            &language,
        )
    };
    match result {
        Ok(snapshot) => {
            // SAFETY: retained exclusive initialized output is disjoint from owner.
            unsafe {
                *setup = snapshot;
            }
            0
        }
        Err(_) => 2,
    }
}
type VoiceFile = crate::voice_reader::Reader<std::fs::File>;
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_file_open(
    path: *const c_char,
    width: usize,
) -> *mut VoiceFile {
    if path.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller retains a terminated compatibility path during setup.
    let bytes = unsafe { CStr::from_ptr(path).to_bytes() };
    let result = crate::voice_storage::compat_path(bytes)
        .and_then(std::fs::File::open)
        .and_then(|file| VoiceFile::new(file, width, crate::voice_reader::TextMode::platform()));
    match result {
        Ok(reader) => Box::into_raw(Box::new(reader)),
        Err(_) => ptr::null_mut(),
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_file_close(reader: *mut VoiceFile) {
    if !reader.is_null() {
        // SAFETY: caller transfers the live unique reader once, after reads drain.
        unsafe {
            drop(Box::from_raw(reader));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_file_next(
    reader: *mut VoiceFile,
    key: *mut *const c_char,
    value: *mut *const c_char,
) -> c_int {
    if reader.is_null() || key.is_null() || value.is_null() {
        return 2;
    }
    // SAFETY: serialized live unique reader; pointer outputs are exclusive and
    // disjoint from the owner. No callbacks or concurrent access to borrowed lines.
    match unsafe { (&mut *reader).next_directive() } {
        Ok(Some((attribute, data))) => {
            // SAFETY: returned spans have NUL sentinels in retained owner storage;
            // caller borrows until next read/close and does not mutate them.
            unsafe {
                *key = attribute.as_ptr().cast();
                *value = data.as_ptr().cast();
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}
#[repr(C)]
struct ForeignVoiceAction {
    action: u32,
    argument: u32,
    backend: crate::voice_backend::Mbrola,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_directive(
    voice: *mut crate::voice::Voice,
    setup: *mut crate::voice_setup::Setup,
    fast: *mut i32,
    features: u32,
    key: *const c_char,
    value: *const c_char,
    output: *mut ForeignVoiceAction,
) -> c_int {
    if voice.is_null()
        || setup.is_null()
        || fast.is_null()
        || key.is_null()
        || value.is_null()
        || output.is_null()
        || features & !3 != 0
    {
        return 2;
    }
    // SAFETY: caller retains initialized exclusive disjoint snapshots/output and
    // terminated inputs for this serialized call. Parsing performs no callbacks.
    let result = unsafe {
        crate::voice_directive::apply(
            &mut *voice,
            &mut *setup,
            &mut *fast,
            crate::voice_directive::Features {
                klatt: features & 1 != 0,
                mbrola: features & 2 != 0,
            },
            CStr::from_ptr(key).to_bytes(),
            CStr::from_ptr(value).to_bytes(),
        )
    };
    let Ok(action) = result else {
        return 2;
    };
    let effect = ForeignVoiceAction::new(action);
    // SAFETY: retained exclusive initialized action output is disjoint from inputs.
    unsafe {
        *output = effect;
    }
    0
}
impl ForeignVoiceAction {
    fn new(action: crate::voice_directive::Action) -> Self {
        let mut effect = ForeignVoiceAction {
            action: 0,
            argument: 0,
            backend: crate::voice_backend::Mbrola {
                voice: [0; 40],
                table: [0; 80],
                sample_rate: 0,
            },
        };
        let setup_effect = |action| match action {
            crate::voice_setup::Effect::None => 0,
            crate::voice_setup::Effect::SelectLanguage => 1,
            crate::voice_setup::Effect::SelectPhonemes => 2,
        };
        use crate::voice_directive::Action;
        match action {
            Action::LanguageOption(key) => {
                effect.action = 1;
                effect.argument = key;
            }
            Action::Acoustics { update_speed } => {
                effect.action = 2;
                effect.argument = u32::from(update_speed);
            }
            Action::Metadata(action) => {
                effect.action = 3;
                effect.argument = setup_effect(action);
            }
            Action::Replacement(action) => {
                effect.action = 4;
                effect.argument = setup_effect(action);
            }
            Action::Mbrola(request) => {
                effect.action = 5;
                effect.backend = request;
            }
            Action::UnsupportedMbrola => effect.action = 6,
            Action::UnsupportedKlatt => effect.action = 7,
            Action::Unknown => {}
        }
        effect
    }
}
type VoiceLoadCallback = unsafe extern "C" fn(
    *mut c_void,
    u32,
    *const crate::voice_setup::Setup,
    *const c_char,
    *const c_char,
    *const ForeignVoiceAction,
    *mut crate::voice::Voice,
    *mut i32,
) -> i32;
struct ForeignLoadHost {
    opaque: *mut c_void,
    callback: VoiceLoadCallback,
}
impl ForeignLoadHost {
    fn simple(
        &mut self,
        kind: u32,
        setup: *const crate::voice_setup::Setup,
        name: *const c_char,
        argument: u32,
    ) -> i32 {
        let mut action = ForeignVoiceAction::new(crate::voice_directive::Action::Unknown);
        action.argument = argument;
        // SAFETY: callback is retained for the serialized load, borrows live
        // terminated names/snapshots only during this call, and never reenters.
        unsafe {
            (self.callback)(
                self.opaque,
                kind,
                setup,
                name,
                ptr::null(),
                &action,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
    }
}
impl crate::voice_load::Host for ForeignLoadHost {
    fn directive(
        &mut self,
        voice: &mut crate::voice::Voice,
        fast: &mut i32,
        setup: &crate::voice_setup::Setup,
        key: &[u8],
        value: &[u8],
        action: crate::voice_directive::Action,
    ) -> bool {
        let action = ForeignVoiceAction::new(action);
        // SAFETY: native stream/setup strings have NUL sentinels. Snapshot
        // borrows are exclusive/disjoint and cannot be retained by the callback.
        unsafe {
            (self.callback)(
                self.opaque,
                1,
                setup,
                key.as_ptr().cast(),
                value.as_ptr().cast(),
                &action,
                voice,
                fast,
            ) != 0
        }
    }
    fn invalid(&mut self, key: &[u8]) {
        self.simple(2, ptr::null(), key.as_ptr().cast(), 0);
    }
    fn ensure_translator(&mut self, setup: &crate::voice_setup::Setup) {
        self.simple(3, setup, ptr::null(), 0);
    }
    fn select_table(&mut self, name: &[u8]) -> i32 {
        self.simple(4, ptr::null(), name.as_ptr().cast(), 0)
    }
    fn unknown_table(&mut self, name: &[u8]) {
        self.simple(5, ptr::null(), name.as_ptr().cast(), 0);
    }
    fn phoneme_index(&mut self, index: i32) {
        self.simple(6, ptr::null(), ptr::null(), index as u32);
    }
    fn dictionary(&mut self, name: &[u8], quiet: bool) -> bool {
        self.simple(7, ptr::null(), name.as_ptr().cast(), u32::from(quiet)) != 0
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_configure(
    reader: *mut VoiceFile,
    setup: *mut crate::voice_setup::Setup,
    voice: *mut crate::voice::Voice,
    fast: *mut i32,
    features: u32,
    control: u32,
    opaque: *mut c_void,
    callback: Option<VoiceLoadCallback>,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if setup.is_null() || voice.is_null() || fast.is_null() || features & !3 != 0 {
        return 2;
    }
    // SAFETY: caller retains initialized exclusive/disjoint setup/snapshots and
    // optional reader during serialized loading. Callbacks neither reenter nor
    // retain snapshot/line pointers; opaque state does not alias these buffers.
    let result = unsafe {
        crate::voice_load::configure(
            reader.as_mut(),
            &mut *setup,
            &mut *voice,
            &mut *fast,
            crate::voice_directive::Features {
                klatt: features & 1 != 0,
                mbrola: features & 2 != 0,
            },
            control,
            &mut ForeignLoadHost { opaque, callback },
        )
    };
    match result {
        Ok(_) => 0,
        Err(crate::voice_load::Error::InvalidSetup) => 2,
        Err(_) => 1,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_request(
    root: *const c_char,
    name: *const c_char,
    control: u32,
    separator: u8,
    path_capacity: usize,
    opaque: *mut c_void,
    length: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i64>,
    output: *mut crate::voice_request::Request,
) -> c_int {
    if root.is_null() || output.is_null() {
        return 2;
    }
    let Some(length) = length else {
        return 2;
    };
    // SAFETY: caller retains terminated inputs disjoint from exclusive output.
    let (root, name) = unsafe {
        (
            CStr::from_ptr(root).to_bytes(),
            if name.is_null() {
                None
            } else {
                Some(CStr::from_ptr(name).to_bytes())
            },
        )
    };
    let result = crate::voice_request::Request::prepare(
        root,
        name,
        control,
        separator,
        path_capacity,
        |path| {
            // SAFETY: synchronous owner callback borrows a terminated path held in
            // initialized request storage; it must not reenter request mutation.
            unsafe { length(opaque, path.as_ptr(), path.len()) }
        },
    );
    match result {
        Ok(Some(request)) => {
            // SAFETY: caller provides initialized aligned exclusive request output.
            unsafe {
                *output = request;
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_fallback(
    request: *const crate::voice_request::Request,
    opened: u32,
    found: u32,
    default: *const c_char,
    output: *mut [u8; 40],
) -> c_int {
    if request.is_null() || default.is_null() || output.is_null() {
        return 2;
    }
    // SAFETY: owner retains initialized request and terminated default disjoint
    // from exclusive output. The flags describe completed owner operations.
    let result = unsafe {
        (&*request).fallback(opened != 0, found != 0, CStr::from_ptr(default).to_bytes())
    };
    match result {
        Ok(Some(name)) => {
            // SAFETY: initialized exclusive name output remains retained.
            unsafe {
                *output = name;
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_identifier(
    output: *mut [u8; 40],
    requested: *const c_char,
    tone_only: u32,
) -> c_int {
    if output.is_null() || requested.is_null() {
        return 2;
    }
    // SAFETY: owner retains initialized identifier and terminated request. Copy
    // both before output mutation, allowing a request that aliases the old name.
    let current = unsafe { *output };
    // SAFETY: readable terminated request is retained until its bounded copy ends.
    let bytes = unsafe { CStr::from_ptr(requested) }.to_bytes();
    let count = bytes.len().min(41);
    let mut input = [0; 41];
    input[..count].copy_from_slice(&bytes[..count]);
    let Ok(identifier) =
        crate::voice_request::identifier(&current, &input[..count], tone_only != 0)
    else {
        return 2;
    };
    // SAFETY: owner provides exclusive initialized output; all inputs are copies.
    unsafe {
        *output = identifier;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_setup_attribute(
    state: *mut crate::voice_setup::Setup,
    key: *const c_char,
    value: *const c_char,
    effect: *mut u32,
) -> c_int {
    if state.is_null() || key.is_null() || value.is_null() || effect.is_null() {
        return 2;
    }
    // SAFETY: owner retains initialized aligned exclusive state/effect and two
    // disjoint terminated input strings for the serialized call.
    let result = unsafe {
        (&mut *state).apply(
            CStr::from_ptr(key).to_bytes(),
            CStr::from_ptr(value).to_bytes(),
        )
    };
    match result {
        Ok(Some(action)) => {
            // SAFETY: exclusive effect output is retained and disjoint from state.
            unsafe {
                *effect = match action {
                    crate::voice_setup::Effect::None => 0,
                    crate::voice_setup::Effect::SelectLanguage => 1,
                    crate::voice_setup::Effect::SelectPhonemes => 2,
                };
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}
unsafe fn borrowed_phonemes<'a>(
    table: *const *const crate::phoneme::Phoneme,
    length: usize,
) -> Option<[Option<&'a crate::phoneme::Phoneme>; 256]> {
    if table.is_null() || length > 256 {
        return None;
    }
    let mut records = [None; 256];
    for (i, entry) in records[..length].iter_mut().enumerate() {
        // SAFETY: private helper's caller retains length initialized pointer
        // slots and every nonnull aligned phoneme record for the borrowed call.
        *entry = unsafe { (*table.add(i)).as_ref() };
    }
    Some(records)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_phoneme_code(
    table: *const *const crate::phoneme::Phoneme,
    length: usize,
    word: u32,
) -> u32 {
    // SAFETY: owner supplies a retained sparse selected table and records.
    let Some(records) = (unsafe { borrowed_phonemes(table, length) }) else {
        return 0;
    };
    u32::from(crate::phoneme::code(records, word))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_phoneme_mnemonic(name: *const c_char) -> u32 {
    if name.is_null() {
        return 0;
    }
    // SAFETY: owner retains a terminated name. Packing examines at most four bytes.
    crate::phoneme::mnemonic(unsafe { CStr::from_ptr(name).to_bytes() })
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_replacement(
    value: *const c_char,
    table: *const *const crate::phoneme::Phoneme,
    length: usize,
    storage: *mut crate::voice_backend::Replacement,
    count: *mut c_int,
) -> c_int {
    if value.is_null() || storage.is_null() || count.is_null() {
        return 2;
    }
    // SAFETY: owner retains initialized count, 60 exclusive records, terminated
    // disjoint input and selected table for this serialized call.
    let result = unsafe {
        let Some(records) = borrowed_phonemes(table, length) else {
            return 2;
        };
        let Ok(mut used) = usize::try_from(*count) else {
            return 2;
        };
        let result = crate::voice_backend::replace(
            std::slice::from_raw_parts_mut(storage, 60),
            &mut used,
            CStr::from_ptr(value).to_bytes(),
            |word| crate::phoneme::code(records, word),
        );
        if result.is_ok() {
            *count = used as c_int;
        }
        result
    };
    if result.is_ok() {
        0
    } else {
        2
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_mbrola_request(
    value: *const c_char,
    output: *mut crate::voice_backend::Mbrola,
) -> c_int {
    if value.is_null() || output.is_null() {
        return 2;
    }
    // SAFETY: owner retains a terminated input disjoint from exclusive output.
    let result = crate::voice_backend::Mbrola::parse(unsafe { CStr::from_ptr(value).to_bytes() });
    let Ok(request) = result else { return 2 };
    // SAFETY: caller provides initialized aligned exclusive request output.
    unsafe {
        *output = request;
    }
    0
}
#[repr(C)]
#[derive(Clone, Copy)]
struct ForeignVoice {
    name: *const c_char,
    languages: *const c_char,
    identifier: *const c_char,
    gender: u8,
    age: u8,
    variant: u8,
    variants: u8,
    score: c_int,
    spare: *mut c_void,
}
struct ForeignCatalog {
    _catalog: crate::voice_storage::Catalog,
    records: Vec<ForeignVoice>,
    order: Vec<*mut ForeignVoice>,
    output: Vec<*mut ForeignVoice>,
    workspace: crate::voice_catalog::Workspace,
}
type CatalogDiagnostic = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize);
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_catalog_create(
    root: *const c_char,
    output: *mut *mut ForeignVoice,
    capacity: usize,
    count: *mut c_int,
    opaque: *mut c_void,
    diagnostic: Option<CatalogDiagnostic>,
) -> *mut c_void {
    if root.is_null() || output.is_null() || count.is_null() || capacity < 499 {
        return ptr::null_mut();
    }
    // SAFETY: owner retains a terminated root for initialization/offload work.
    let bytes = unsafe { CStr::from_ptr(root) }.to_bytes();
    let Ok(root) = crate::voice_storage::compat_path(bytes) else {
        return ptr::null_mut();
    };
    let result = crate::voice_storage::Catalog::load(&root, |kind, identifier| {
        if let Some(report) = diagnostic {
            let kind = match kind {
                crate::voice_storage::Diagnostic::GenderOnLanguage => 1,
                crate::voice_storage::Diagnostic::InvalidFile => 2,
                crate::voice_storage::Diagnostic::Full => 3,
            };
            // SAFETY: owner retains callback/opaque and consumes borrowed bytes
            // synchronously without reentering catalogue mutation.
            unsafe {
                report(opaque, kind, identifier.as_ptr(), identifier.len());
            }
        }
    });
    let Ok(catalog) = result else {
        return ptr::null_mut();
    };
    let records = catalog
        .records()
        .map(|record| {
            let view = record.view();
            ForeignVoice {
                name: if record.metadata.name[0] == 0 {
                    record.terminated_identifier().as_ptr().cast()
                } else {
                    record.metadata.name.as_ptr().cast()
                },
                languages: record.metadata.languages.as_ptr().cast(),
                identifier: record.terminated_identifier().as_ptr().cast(),
                gender: view.gender,
                age: view.age,
                variant: 0,
                variants: view.variants,
                score: 0,
                spare: ptr::null_mut(),
            }
        })
        .collect();
    let mut owner = Box::new(ForeignCatalog {
        _catalog: catalog,
        records,
        order: Vec::with_capacity(499),
        output: vec![ptr::null_mut(); 499],
        workspace: crate::voice_catalog::Workspace::new(499).expect("fixed valid capacity"),
    });
    for record in &mut owner.records {
        owner.order.push(record);
    }
    owner.order.push(ptr::null_mut());
    // SAFETY: initialized exclusive pointer order retains every owned record
    // and its terminated strings. Sorting moves pointers, leaving records stable.
    if unsafe { espeak_rs_voice_order(owner.order.as_mut_ptr(), owner.records.len()) } != 0 {
        return ptr::null_mut();
    }
    // SAFETY: owner provides at least 499 exclusive output pointer slots and an
    // initialized count, disjoint from retained root. Published records/strings
    // remain initialized at stable addresses until this returned owner is destroyed.
    unsafe {
        *count = owner.records.len() as c_int;
        ptr::copy_nonoverlapping(owner.order.as_ptr(), output, owner.order.len());
    }
    Box::into_raw(owner).cast()
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_catalog_workspace(
    owner: *mut c_void,
) -> *mut crate::voice_catalog::Workspace {
    if owner.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: sole serialized caller retains this owned catalogue. The workspace
    // is borrowed until catalogue destruction and must never be freed separately.
    unsafe { &mut (*owner.cast::<ForeignCatalog>()).workspace }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_catalog_list(
    owner: *mut c_void,
    spec: *const ForeignVoice,
    separator: u8,
    opaque: *mut c_void,
    directory: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize) -> u32>,
) -> *mut *mut ForeignVoice {
    if owner.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: optional initialized selector/terminated strings remain retained
    // and disjoint from result pointer storage and score fields. Copy before
    // borrowing the exclusive catalogue; no callback may reenter its mutation.
    let selector = if spec.is_null() {
        None
    } else {
        // SAFETY: retained initialized selector is copied before owner mutation.
        Some(unsafe { *spec })
    };
    // SAFETY: serialized owner retains the exact live returned catalogue.
    let owner = unsafe { &mut *owner.cast::<ForeignCatalog>() };
    owner.output.fill(ptr::null_mut());
    if let Some(spec) = selector {
        // SAFETY: caller retains optional terminated selector strings for this call.
        let properties = unsafe { foreign_properties(&spec) };
        let Ok(mut filter) =
            crate::voice_catalog::Filter::new(properties.language, true, false, separator)
        else {
            return ptr::null_mut();
        };
        if filter.parts == 1 {
            let is_directory = directory.is_some_and(|callback| {
                // SAFETY: caller retains callback/opaque; filter bytes are borrowed
                // only during this synchronous call, without catalogue reentry.
                unsafe { callback(opaque, filter.language.as_ptr(), filter.length) != 0 }
            });
            if is_directory {
                let Ok(updated) =
                    crate::voice_catalog::Filter::new(properties.language, true, true, separator)
                else {
                    return ptr::null_mut();
                };
                filter = updated;
            }
        }
        let roster = ForeignRoster {
            voices: owner.order.as_ptr(),
            length: owner.records.len(),
        };
        let selector = crate::voice_selection::Selector {
            name: properties.name,
            gender: properties.gender,
            age: properties.age,
        };
        let Ok(ranked) = owner.workspace.rank(&roster, selector, &filter, true) else {
            return ptr::null_mut();
        };
        for (position, entry) in ranked.iter().enumerate() {
            let record = owner.order[entry.index];
            if entry.update_score {
                // SAFETY: planning has ended; this owner retains exclusive score
                // fields disjoint from selector strings and output pointer storage.
                unsafe {
                    (*record).score = entry.score;
                }
            }
            owner.output[position] = record;
        }
    } else {
        let mut used = 0;
        for &record in &owner.order[..owner.records.len()] {
            // SAFETY: admitted owned records/strings remain initialized and stable.
            let Some(view) = (unsafe { borrowed_voice(&*record) }) else {
                return ptr::null_mut();
            };
            if crate::voice_catalog::visible(view, separator) {
                owner.output[used] = record;
                used += 1;
            }
        }
    }
    owner.output.as_mut_ptr()
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_catalog_destroy(owner: *mut c_void) {
    if !owner.is_null() {
        // SAFETY: serialized caller transfers exactly one live returned owner
        // after retiring all borrowed record/string pointers; no callback runs.
        drop(unsafe { Box::from_raw(owner.cast::<ForeignCatalog>()) });
    }
}
struct ForeignRoster {
    voices: *const *mut ForeignVoice,
    length: usize,
}
impl crate::voice_catalog::Roster for ForeignRoster {
    fn len(&self) -> usize {
        self.length
    }
    fn voice(&self, index: usize) -> Option<crate::voice_selection::Voice<'_>> {
        if index >= self.length {
            return None;
        }
        // SAFETY: private roster is bound by the serialized ABI entrypoint to
        // initialized nonnull retained records and their terminated text spans.
        unsafe { borrowed_voice(&**self.voices.add(index)) }
    }
    fn previous_score(&self, index: usize) -> i32 {
        if index >= self.length {
            return 0;
        }
        // SAFETY: validated private roster retains each initialized record.
        unsafe { (**self.voices.add(index)).score }
    }
}
unsafe fn foreign_roster(voices: *const *mut ForeignVoice) -> Option<ForeignRoster> {
    if voices.is_null() {
        return None;
    }
    for length in 0..500 {
        // SAFETY: owner supplies at most 499 initialized entries then a NULL.
        if unsafe { (*voices.add(length)).is_null() } {
            return Some(ForeignRoster { voices, length });
        }
    }
    None
}
unsafe fn foreign_properties<'a>(spec: &'a ForeignVoice) -> crate::voice_catalog::Properties<'a> {
    unsafe fn text<'a>(input: *const c_char) -> Option<&'a [u8]> {
        if input.is_null() {
            None
        } else {
            // SAFETY: every nonnull selector string is terminated and retained.
            Some(unsafe { CStr::from_ptr(input) }.to_bytes())
        }
    }
    // SAFETY: owner retains optional selector strings for the complete call.
    unsafe {
        crate::voice_catalog::Properties {
            name: text(spec.name),
            language: text(spec.languages),
            identifier: text(spec.identifier),
            gender: spec.gender,
            age: spec.age,
            variant: spec.variant,
        }
    }
}
#[no_mangle]
extern "C" fn espeak_rs_voice_workspace_create(
    capacity: usize,
) -> *mut crate::voice_catalog::Workspace {
    crate::voice_catalog::Workspace::new(capacity).map_or(ptr::null_mut(), |workspace| {
        Box::into_raw(Box::new(workspace))
    })
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_workspace_destroy(
    workspace: *mut crate::voice_catalog::Workspace,
) {
    if !workspace.is_null() {
        // SAFETY: sole serialized owner returns the created workspace exactly once.
        unsafe {
            drop(Box::from_raw(workspace));
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_filter(
    language: *const c_char,
    include_mbrola: u32,
    separator: u8,
    output: *mut [u8; 80],
    parts: *mut i32,
) -> c_int {
    if output.is_null() || parts.is_null() {
        return 2;
    }
    let language = if language.is_null() {
        None
    } else {
        // SAFETY: nonnull input is a retained terminated selector, disjoint from outputs.
        Some(unsafe { CStr::from_ptr(language) }.to_bytes())
    };
    let Ok(filter) =
        crate::voice_catalog::Filter::new(language, include_mbrola != 0, false, separator)
    else {
        return 2;
    };
    // SAFETY: owner supplies disjoint aligned exclusive writable outputs.
    unsafe {
        *output = filter.language;
        *parts = filter.parts;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_rank(
    workspace: *mut crate::voice_catalog::Workspace,
    spec: *const ForeignVoice,
    voices: *const *mut ForeignVoice,
    output: *mut *mut ForeignVoice,
    capacity: usize,
    include_mbrola: u32,
    directory: u32,
    separator: u8,
) -> c_int {
    if workspace.is_null() || spec.is_null() || output.is_null() {
        return -1;
    }
    // SAFETY: serialized owner retains a terminated roster with initialized
    // records; optional selector strings are readable and terminated.
    let (Some(roster), properties) =
        (unsafe { (foreign_roster(voices), foreign_properties(&*spec)) })
    else {
        return -1;
    };
    if capacity <= roster.length {
        return -1;
    }
    let Ok(filter) = crate::voice_catalog::Filter::new(
        properties.language,
        include_mbrola != 0,
        directory != 0,
        separator,
    ) else {
        return -1;
    };
    // SAFETY: exclusive workspace remains alive; all roster access is read-only
    // while planning. Output is disjoint from inputs and has the declared capacity.
    unsafe {
        let Ok(ranked) = (&mut *workspace).rank(
            &roster,
            crate::voice_selection::Selector {
                name: properties.name,
                gender: properties.gender,
                age: properties.age,
            },
            &filter,
            include_mbrola != 0,
        ) else {
            return -1;
        };
        for (position, rank) in ranked.iter().enumerate() {
            let voice = *voices.add(rank.index);
            if rank.update_score {
                (*voice).score = rank.score;
            }
            *output.add(position) = voice;
        }
        *output.add(ranked.len()) = ptr::null_mut();
        ranked.len() as c_int
    }
}
type VoiceDirectoryCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> u32;
#[repr(C)]
struct ForeignSelection {
    index: usize,
    found: u32,
    suffix: [u8; 40],
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_select(
    workspace: *mut crate::voice_catalog::Workspace,
    spec: *const ForeignVoice,
    voices: *const *mut ForeignVoice,
    separator: u8,
    opaque: *mut c_void,
    directory: Option<VoiceDirectoryCallback>,
    output: *mut ForeignSelection,
) -> c_int {
    if workspace.is_null() || spec.is_null() || output.is_null() {
        return 2;
    }
    // SAFETY: owner retains initialized terminated roster and optional selector
    // strings, with exclusive workspace and output disjoint from inputs/callback.
    let (Some(roster), properties) =
        (unsafe { (foreign_roster(voices), foreign_properties(&*spec)) })
    else {
        return 2;
    };
    // SAFETY: exclusive workspace is retained for the call and callback; callback
    // borrows a bounded normalized directory name and cannot reenter selection.
    let workspace = unsafe { &mut *workspace };
    let result = workspace.select_with_directory(&roster, properties, separator, b"en", |filter| {
        directory.is_some_and(|callback| {
            // SAFETY: synchronous owner callback borrows only this initialized span.
            unsafe { callback(opaque, filter.bytes().as_ptr(), filter.length) != 0 }
        })
    });
    let Ok(selection) = result else {
        return 2;
    };
    // SAFETY: planning succeeded; exclusive score fields and disjoint output are
    // writable. Every rank references a validated retained list entry.
    unsafe {
        for rank in workspace.ranked() {
            if rank.update_score {
                (**voices.add(rank.index)).score = rank.score;
            }
        }
        match selection {
            Some(selection) => {
                ptr::write(
                    output,
                    ForeignSelection {
                        index: selection.index,
                        found: u32::from(selection.found),
                        suffix: selection.suffix,
                    },
                );
                0
            }
            None => {
                ptr::write(
                    output,
                    ForeignSelection {
                        index: usize::MAX,
                        found: u32::from(!workspace.ranked().is_empty()),
                        suffix: [0; 40],
                    },
                );
                1
            }
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_order(voices: *mut *mut ForeignVoice, count: usize) -> c_int {
    if voices.is_null() || count > 499 {
        return 2;
    }
    // SAFETY: owner supplies an exclusive pointer array of exactly count entries;
    // pointed-to initialized records and their strings are immutable during sort.
    let entries = unsafe { std::slice::from_raw_parts_mut(voices, count) };
    // SAFETY: every record and terminated metadata span is retained by the owner.
    if entries.iter().any(|voice| {
        if voice.is_null() {
            return true;
        }
        // SAFETY: every nonnull record and its terminated spans are retained.
        unsafe { borrowed_voice(&**voice) }.is_none()
    }) {
        return 2;
    }
    // Catalogue setup preserves input order for equal keys. Its stable-sort
    // scratch is initialization-only; request ranking uses the reusable workspace.
    entries.sort_by(|a, b| {
        // SAFETY: admitted catalogue metadata contains a retained primary name
        // after its priority byte, including when that priority is zero.
        unsafe {
            let (a, b) = (&**a, &**b);
            CStr::from_ptr(a.languages.add(1))
                .to_bytes()
                .cmp(CStr::from_ptr(b.languages.add(1)).to_bytes())
                .then_with(|| (*a.languages).cmp(&*b.languages))
                .then_with(|| {
                    CStr::from_ptr(a.name)
                        .to_bytes()
                        .cmp(CStr::from_ptr(b.name).to_bytes())
                })
        }
    });
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_variant(
    name: *const c_char,
    number: i32,
    directory: u32,
    separator: u8,
    base_length: *mut usize,
    suffix: *mut [u8; 40],
) -> c_int {
    if base_length.is_null() || suffix.is_null() {
        return 2;
    }
    // SAFETY: optional name is terminated, retained and disjoint from exclusive
    // aligned outputs; input and outputs remain alive for this serialized call.
    let name = if name.is_null() {
        None
    } else {
        // SAFETY: nonnull name is a retained terminated input as required above.
        Some(unsafe { CStr::from_ptr(name) }.to_bytes())
    };
    let Ok(variant) = crate::voice_selection::variant(name, number, directory != 0, separator)
    else {
        return 2;
    };
    // SAFETY: both disjoint outputs are writable; validated selection is complete.
    unsafe {
        *base_length = variant.base_length;
        *suffix = *variant.terminated_suffix();
    }
    0
}
unsafe fn borrowed_voice<'a>(voice: &'a ForeignVoice) -> Option<crate::voice_selection::Voice<'a>> {
    if voice.name.is_null() || voice.identifier.is_null() || voice.languages.is_null() {
        return None;
    }
    let mut length = 0;
    loop {
        // SAFETY: caller retains a terminated priority/name list. Each current
        // priority and following terminated name lie within that allocation.
        if unsafe { *voice.languages.add(length) } == 0 {
            length += 1;
            break;
        }
        // SAFETY: every nonzero priority is followed by a readable terminated name.
        let name = unsafe { CStr::from_ptr(voice.languages.add(length + 1)) }.to_bytes();
        length += name.len() + 2;
        if length >= 300 {
            return None;
        }
    }
    // SAFETY: initialized voice borrows terminated names and the exact list
    // span discovered above, retained by the serialized caller for this call.
    unsafe {
        Some(crate::voice_selection::Voice {
            name: CStr::from_ptr(voice.name).to_bytes(),
            identifier: CStr::from_ptr(voice.identifier).to_bytes(),
            languages: std::slice::from_raw_parts(voice.languages.cast::<u8>(), length),
            gender: voice.gender,
            age: voice.age,
            variants: voice.variants,
        })
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_metadata_line(
    state: *mut crate::voice_selection::Metadata,
    input: *const c_char,
) -> c_int {
    if state.is_null() || input.is_null() {
        return 2;
    }
    // SAFETY: state is initialized, exclusive, aligned and disjoint from input;
    // owner retains the terminated input for this call.
    let (mut snapshot, bytes) = unsafe { (*state, CStr::from_ptr(input).to_bytes()) };
    let mut gender = false;
    let Ok(directives) = crate::voice::Directives::new(bytes, 120) else {
        return 2;
    };
    for (key, value) in directives {
        match snapshot.apply(key, value) {
            Ok(found) => gender |= found,
            Err(_) => return 2,
        }
    }
    // SAFETY: exclusively owned output was validated before mutation.
    unsafe {
        *state = snapshot;
    }
    i32::from(gender)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_metadata_gender(
    state: *const crate::voice_selection::Metadata,
) -> c_int {
    if state.is_null() {
        return 1;
    }
    // SAFETY: caller retains immutable initialized aligned metadata.
    unsafe { i32::from((*state).gender()) }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_score(
    spec: *const ForeignVoice,
    language: *const c_char,
    parts: i32,
    length: usize,
    voice: *const ForeignVoice,
) -> c_int {
    if length > 79 || parts > 80 {
        return 0;
    }
    if spec.is_null() || voice.is_null() || (language.is_null() && length != 0) {
        return 0;
    }
    // SAFETY: serialized owner retains initialized voice records and their
    // terminated names/lists; selector name is optional and language span exact.
    unsafe {
        let spec = &*spec;
        let Some(voice) = borrowed_voice(&*voice) else {
            return 0;
        };
        let name = if spec.name.is_null() {
            None
        } else {
            Some(CStr::from_ptr(spec.name).to_bytes())
        };
        let language = if length == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(language.cast::<u8>(), length)
        };
        crate::voice_selection::score(
            crate::voice_selection::Selector {
                name,
                gender: spec.gender,
                age: spec.age,
            },
            language,
            parts,
            voice,
        )
        .unwrap_or(0)
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_by_name(
    voices: *const *mut ForeignVoice,
    name: *const c_char,
    separator: u8,
) -> *mut ForeignVoice {
    if voices.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: owner retains terminated name and at most 499 initialized voice
    // records followed by a null pointer, disjoint from all mutations.
    let name = unsafe { CStr::from_ptr(name).to_bytes() };
    let mut terminated = false;
    let iterator = (0..500).map_while(|index| {
        if terminated {
            return None;
        }
        // SAFETY: each pointer is within the caller's terminated voice array.
        let voice = unsafe { *voices.add(index) };
        if voice.is_null() {
            terminated = true;
            return None;
        }
        // SAFETY: list entry is an initialized retained voice record.
        unsafe { borrowed_voice(&*voice) }
    });
    let index = crate::voice_selection::by_name(iterator, name, separator);
    // SAFETY: returned index came from an initialized entry in this retained list.
    index.map_or(ptr::null_mut(), |index| unsafe { *voices.add(index) })
}

#[cfg(windows)]
type LanguageWide = u16;
#[cfg(not(windows))]
type LanguageWide = u32;
#[repr(C)]
struct ForeignLanguageSetup {
    options: crate::language_options::Options,
    settings: crate::language::Settings,
    selector: u32,
    dictionary: [u8; 40],
    bits: *const u8,
    tones: *const u8,
    transpose_map: *const u8,
    pairs: *const i16,
    lengths: *const u8,
    last_lengths: *const u8,
    apostrophe: *const LanguageWide,
    punctuation: *const LanguageWide,
    ignored: *const u16,
    groups: [*const LanguageWide; 8],
    group_lengths: [usize; 8],
    ordinal: *const u8,
    roman: *const u8,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_language_setup(
    name: *const c_char,
    output: *mut ForeignLanguageSetup,
) -> c_int {
    if name.is_null() || output.is_null() {
        return 2;
    }
    // SAFETY: owner retains terminated readable name, disjoint from the aligned
    // exclusive output. Selection only borrows process-lifetime immutable tables.
    let Ok(language) = crate::language::Language::new(unsafe { CStr::from_ptr(name).to_bytes() })
    else {
        return 2;
    };
    let preset = language.preset;
    let mut dictionary = [0; 40];
    dictionary[..language.dictionary().len()].copy_from_slice(language.dictionary());
    #[cfg(windows)]
    let (apostrophe, punctuation, groups) = (
        preset.apostrophe_wide,
        preset.punctuation_wide,
        preset.groups_wide,
    );
    #[cfg(not(windows))]
    let (apostrophe, punctuation, groups) = (preset.apostrophe, preset.punctuation, preset.groups);
    let setup = ForeignLanguageSetup {
        options: language.options,
        settings: language.settings,
        selector: language.selector,
        dictionary,
        bits: preset.letter_bits.as_ptr(),
        tones: preset.punct_to_tone.as_ptr(),
        transpose_map: preset.transpose_map.map_or(ptr::null(), |map| map.as_ptr()),
        pairs: preset.pairs.map_or(ptr::null(), |pairs| pairs.as_ptr()),
        lengths: preset.lengths.as_ptr(),
        last_lengths: preset.last_lengths.as_ptr(),
        apostrophe: apostrophe.as_ptr(),
        punctuation: punctuation.as_ptr(),
        ignored: preset.ignored.as_ptr(),
        groups: groups.map(|units| units.map_or(ptr::null(), |units| units.as_ptr())),
        group_lengths: groups.map(|units| units.map_or(0, |units| units.len() - 1)),
        ordinal: preset.ordinal.map_or(ptr::null(), |units| units.as_ptr()),
        roman: preset.roman.as_ptr(),
    };
    // SAFETY: caller provides initialized or uninitialized writable setup storage;
    // no Rust-owned references escape, and all borrowed tables are static.
    unsafe {
        ptr::write(output, setup);
    }
    0
}
#[no_mangle]
extern "C" fn espeak_rs_alphabet_index(character: i32) -> i32 {
    crate::language::alphabet_index(character).map_or(-1, |index| index as i32)
}

type LanguageCallback = unsafe extern "C" fn(*mut c_void, u32, u32, *const u8, usize, i32) -> i32;
struct ForeignLanguage {
    opaque: *mut c_void,
    callback: LanguageCallback,
}
impl crate::language_options::Environment for ForeignLanguage {
    fn tune(&self, name: &[u8]) -> Option<i32> {
        // SAFETY: serialized owner callback borrows only the supplied name span.
        let result = unsafe { (self.callback)(self.opaque, 0, 0, name.as_ptr(), name.len(), 0) };
        (result >= 0).then_some(result)
    }
    fn bad_ordinal(&mut self, key: u32, number: i32) {
        // SAFETY: diagnostic callback receives values and an empty name span.
        unsafe {
            (self.callback)(self.opaque, 1, key, ptr::null(), 0, number);
        }
    }
    fn unknown_tune(&mut self, name: &[u8]) {
        // SAFETY: diagnostic callback borrows only the supplied name span.
        unsafe {
            (self.callback)(self.opaque, 2, 0, name.as_ptr(), name.len(), 0);
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_language_option(
    options: *mut crate::language_options::Options,
    key: u32,
    input: *const c_char,
    opaque: *mut c_void,
    callback: Option<LanguageCallback>,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if options.is_null() || input.is_null() {
        return 2;
    }
    // SAFETY: initialized aligned options are exclusive/disjoint from retained
    // immutable terminated input and callback state for the serialized call.
    let result = unsafe {
        (&mut *options).apply(
            key,
            CStr::from_ptr(input).to_bytes(),
            &mut ForeignLanguage { opaque, callback },
        )
    };
    if result.is_ok() {
        0
    } else {
        2
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_language_separators(
    numbers: u32,
    thousands: *mut i32,
    decimal: *mut i32,
) {
    if thousands.is_null() || decimal.is_null() {
        return;
    }
    // SAFETY: owner retains two disjoint exclusive initialized aligned scalars.
    unsafe {
        crate::language_options::separators(numbers, &mut *thousands, &mut *decimal);
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_language_ordinals(
    input: *const c_char,
    flags: *mut u32,
    maximum: i32,
    key: u32,
    opaque: *mut c_void,
    callback: Option<LanguageCallback>,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if input.is_null() || flags.is_null() {
        return 2;
    }
    // SAFETY: input is retained immutable terminated bytes for this call.
    let result = crate::language_options::ordinal_flags(
        unsafe { CStr::from_ptr(input) }.to_bytes(),
        maximum,
        key,
        &mut ForeignLanguage { opaque, callback },
    );
    let Ok((first, second)) = result else {
        return 2;
    };
    if second != 0 {
        return 2;
    }
    // SAFETY: flags is exclusive initialized aligned owner storage.
    unsafe {
        *flags |= first;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_reset(
    voice: *mut crate::voice::Voice,
    sample_rate: i32,
    points: *mut [i32; 12],
    rates: *mut [i32; 9],
    fast: *mut i32,
) -> c_int {
    if voice.is_null() || points.is_null() || rates.is_null() || fast.is_null() {
        return 2;
    }
    // SAFETY: owner retains initialized aligned disjoint exclusive snapshots.
    let result = unsafe { (&mut *voice).reset(sample_rate, &mut *points) };
    let Ok((setting, values)) = result else {
        return 2;
    };
    // SAFETY: rate/fast outputs are aligned, disjoint and exclusive for the call.
    unsafe {
        rates.write(values);
        fast.write(setting);
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_voice_attribute(
    voice: *mut crate::voice::Voice,
    keyword: *const c_char,
    input: *const c_char,
    klatt: u32,
    fast: *mut i32,
    speed: *mut u32,
) -> c_int {
    if voice.is_null() || keyword.is_null() || input.is_null() || fast.is_null() || speed.is_null()
    {
        return 2;
    }
    // SAFETY: owner retains immutable terminated input and disjoint initialized
    // exclusive aligned voice/fast snapshots and speed output for this call.
    let result = unsafe {
        (&mut *voice).apply(
            CStr::from_ptr(keyword).to_bytes(),
            CStr::from_ptr(input).to_bytes(),
            klatt != 0,
            &mut *fast,
        )
    };
    match result {
        Ok(Some(value)) => {
            // SAFETY: speed is an exclusive aligned disjoint output.
            unsafe {
                speed.write(u32::from(value));
            }
            0
        }
        Ok(None) => 1,
        Err(_) => 2,
    }
}
#[no_mangle]
unsafe extern "C" fn Read8Numbers(input: *const c_char, output: *mut [i32; 8]) -> c_int {
    if input.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: input is terminated immutable bytes retained for this call.
    let values = crate::voice::numbers::<8>(unsafe { CStr::from_ptr(input) }.to_bytes());
    let (values, count) = values.unwrap_or(([0; 8], 0));
    // SAFETY: output is aligned exclusive storage for eight integers.
    unsafe {
        output.write(values);
    }
    count
}
#[no_mangle]
unsafe extern "C" fn ReadTonePoints(input: *const c_char, output: *mut [i32; 12]) {
    if input.is_null() || output.is_null() {
        return;
    }
    // SAFETY: input is terminated immutable bytes retained for this call.
    let values =
        crate::voice::tone_points(unsafe { CStr::from_ptr(input) }.to_bytes()).unwrap_or([-1; 12]);
    // SAFETY: output is aligned exclusive storage for twelve integers.
    unsafe {
        output.write(values);
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_smooth_spectrum(
    queue: *mut crate::smoothing::Command<*mut c_void>,
    capacity: usize,
    start: *mut i32,
    end: i32,
    centre: i32,
    rates: *const [i32; 6],
    opaque: *mut c_void,
    callback: Option<FrameStorage>,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if queue.is_null()
        || start.is_null()
        || rates.is_null()
        || capacity == 0
        || capacity > crate::formant::MAX_POOL_FRAMES
    {
        return 2;
    }
    // SAFETY: caller retains an exclusive aligned initialized four-word ring,
    // disjoint start output/rate snapshot and owner frame storage for the call.
    let (queue, original_start, rates) = unsafe {
        (
            std::slice::from_raw_parts_mut(queue, capacity),
            *start,
            *rates,
        )
    };
    let (Ok(begin), Ok(end)) = (usize::try_from(original_start), usize::try_from(end)) else {
        return 2;
    };
    let mut syllable = crate::smoothing::Syllable {
        start: begin,
        end,
        centre: if centre < 0 {
            None
        } else {
            Some(centre as usize)
        },
    };
    let mut workspace = crate::smoothing::Workspace::new(ptr::null_mut());
    if crate::smoothing::smooth(
        &mut ForeignFrames { opaque, callback },
        queue,
        &mut syllable,
        &rates,
        &mut workspace,
    )
    .is_err()
    {
        return 2;
    }
    // SAFETY: start is an exclusive aligned disjoint output retained by caller.
    unsafe {
        start.write(syllable.start as i32);
    }
    0
}
type FrameStorage = unsafe extern "C" fn(*mut c_void, u32, *mut c_void) -> *mut c_void;
struct ForeignFrames {
    opaque: *mut c_void,
    callback: FrameStorage,
}
impl crate::formant::Storage<*mut c_void> for ForeignFrames {
    fn read(
        &self,
        handle: *mut c_void,
    ) -> Result<crate::formant::Frame, crate::phoneme_data::InvalidPhonemeData> {
        use crate::phoneme_data::InvalidPhonemeData as Error;
        if handle.is_null() || handle as usize % std::mem::align_of::<crate::formant::Frame>() != 0
        {
            return Err(Error("invalid formant frame pointer"));
        }
        // SAFETY: C owner supplies a live aligned frame, readable for its
        // flagged ordinary/Klatt size, or a full writable pool record.
        let flags = unsafe { handle.cast::<i16>().read() };
        if flags as u16 & crate::formant::COPIED != 0 {
            if !self.writable(handle) {
                return Err(Error("copied formant is outside owner pool"));
            }
            // SAFETY: the owner confirmed this is one initialized full pool frame.
            return Ok(unsafe { handle.cast::<crate::formant::Frame>().read() });
        }
        let size = if flags & 1 != 0 { 64 } else { 44 };
        // SAFETY: owner retains this flagged readable record for the call.
        crate::formant::Frame::decode(unsafe {
            std::slice::from_raw_parts(handle.cast::<u8>(), size)
        })
    }
    fn writable(&self, handle: *mut c_void) -> bool {
        // SAFETY: callback checks membership without dereferencing the handle.
        !unsafe { (self.callback)(self.opaque, 1, handle) }.is_null()
    }
    fn write(
        &mut self,
        handle: *mut c_void,
        frame: crate::formant::Frame,
    ) -> Result<(), crate::phoneme_data::InvalidPhonemeData> {
        if !self.writable(handle) {
            return Err(crate::phoneme_data::InvalidPhonemeData(
                "formant mutation requires an owner pool frame",
            ));
        }
        // SAFETY: owner membership guarantees an exclusive aligned writable
        // full frame. Native math holds only value snapshots, no live aliases.
        unsafe {
            handle.cast::<crate::formant::Frame>().write(frame);
        }
        Ok(())
    }
    fn allocate(
        &mut self,
        frame: crate::formant::Frame,
    ) -> Result<*mut c_void, crate::phoneme_data::InvalidPhonemeData> {
        // SAFETY: owner returns an admitted live writable full pool frame.
        let handle = unsafe { (self.callback)(self.opaque, 0, ptr::null_mut()) };
        if handle.is_null() || handle as usize % std::mem::align_of::<crate::formant::Frame>() != 0
        {
            return Err(crate::phoneme_data::InvalidPhonemeData(
                "owner formant pool unavailable",
            ));
        }
        self.write(handle, frame)?;
        Ok(handle)
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_frame_copy(
    handle: *mut c_void,
    force: u32,
    opaque: *mut c_void,
    callback: Option<FrameStorage>,
) -> *mut c_void {
    let Some(callback) = callback else {
        return ptr::null_mut();
    };
    crate::formant::copy_frame(&mut ForeignFrames { opaque, callback }, handle, force != 0)
        .unwrap_or(ptr::null_mut())
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_formant_transition(
    frames: *mut crate::spectrum::FrameRef<*mut c_void>,
    capacity: usize,
    count: *mut i32,
    data1: u32,
    data2: u32,
    settings: *const crate::formant::Settings,
    opaque: *mut c_void,
    callback: Option<FrameStorage>,
    out: *mut crate::formant::Effects,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if frames.is_null()
        || count.is_null()
        || settings.is_null()
        || out.is_null()
        || capacity > crate::spectrum::MAX_FRAMES
    {
        return 2;
    }
    // SAFETY: caller retains exclusive initialized capacity-entry references
    // and count, immutable aligned settings, and a disjoint exclusive output.
    let (frames, original_count, settings) = unsafe {
        (
            std::slice::from_raw_parts_mut(frames, capacity),
            *count,
            *settings,
        )
    };
    let Ok(mut length) = usize::try_from(original_count) else {
        return 2;
    };
    let result = crate::formant::transition(
        &mut ForeignFrames { opaque, callback },
        frames,
        &mut length,
        data1,
        data2,
        settings,
    );
    let Ok(result) = result else {
        return 2;
    };
    // SAFETY: caller supplies exclusive aligned count/effects outputs.
    unsafe {
        count.write(length as i32);
        out.write(result);
    }
    0
}
type SpectrumFrame = crate::spectrum::FrameRef<*mut c_void>;
type SpectrumTransition = unsafe extern "C" fn(
    *mut c_void,
    *mut SpectrumFrame,
    *mut i32,
    *const crate::spectrum::Parameters,
    i32,
    *mut i32,
    usize,
) -> i32;
struct BorrowedSpectra {
    base: *const u8,
    opaque: *mut c_void,
    transition: Option<SpectrumTransition>,
}
impl crate::spectrum::Environment<*mut c_void> for BorrowedSpectra {
    fn resident(&mut self, frame: crate::spectrum::Frame<'_>) -> *mut c_void {
        // SAFETY: parsed frame offset lies within the retained resident allocation.
        unsafe { self.base.add(frame.offset()).cast_mut().cast() }
    }
    fn transition(
        &mut self,
        frames: &mut [SpectrumFrame],
        count: &mut usize,
        parameters: &crate::spectrum::Parameters,
        which: i32,
        adjust: &mut i32,
    ) -> Result<i32, crate::phoneme_data::InvalidPhonemeData> {
        use crate::phoneme_data::InvalidPhonemeData as Error;
        let callback = self
            .transition
            .ok_or(Error("missing spectrum transition callback"))?;
        // A copied flag belongs only to a writable host-pool frame, not phondata.
        if frames[..*count]
            .iter()
            .any(|frame| frame.flags as u16 & 0x8000 != 0)
        {
            return Err(Error("resident spectrum marked as a writable copied frame"));
        }
        let flags = parameters.transition0 as u32 >> 12;
        let extends = which != 1
            && ((parameters.transition1 as u32 & 63) != 0 || flags != 0)
            && flags & 8 == 0
            && *count >= 2;
        if extends && *count == frames.len() {
            return Err(Error("no capacity for spectrum transition"));
        }
        let mut length = *count as i32;
        // SAFETY: callback is synchronous and only mutates the supplied frame
        // refs/host pool. Resident bytes stay immutable; capacity allows its
        // maximum one-frame extension. Owner retains all returned handles.
        let result = unsafe {
            callback(
                self.opaque,
                frames.as_mut_ptr(),
                &mut length,
                parameters,
                which,
                adjust,
                frames.len(),
            )
        };
        *count = usize::try_from(length).map_err(|_| Error("negative blended frame count"))?;
        Ok(result)
    }
}
#[repr(C)]
struct SpectrumSelection {
    start: usize,
    count: usize,
    length_adjust: i32,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_spectrum_lookup(
    bytes: *const u8,
    length: usize,
    parameters: *const crate::spectrum::Parameters,
    settings: *const crate::spectrum::Settings,
    opaque: *mut c_void,
    transition: Option<SpectrumTransition>,
    frames: *mut SpectrumFrame,
    out: *mut SpectrumSelection,
) -> c_int {
    if bytes.is_null()
        || bytes as usize % std::mem::align_of::<i16>() != 0
        || parameters.is_null()
        || settings.is_null()
        || frames.is_null()
        || out.is_null()
        || length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: caller supplies resident immutable bytes, aligned immutable
    // settings, and an exclusive 25-entry output disjoint from all inputs.
    let (bytes, parameters, settings, frames) = unsafe {
        (
            std::slice::from_raw_parts(bytes, length),
            &*parameters,
            *settings,
            &mut *frames.cast::<[SpectrumFrame; crate::spectrum::MAX_FRAMES]>(),
        )
    };
    let result = crate::spectrum::SpectrumData::new(bytes).lookup(
        parameters,
        settings,
        frames,
        &mut BorrowedSpectra {
            base: bytes.as_ptr(),
            opaque,
            transition,
        },
    );
    let Ok(result) = result else {
        return 2;
    };
    // SAFETY: caller supplies one exclusive aligned selection output.
    unsafe {
        out.write(SpectrumSelection {
            start: result.start,
            count: result.count,
            length_adjust: result.length_adjust,
        });
    }
    0
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_envelope(
    bytes: *const u8,
    length: usize,
    address: i32,
) -> *const u8 {
    if bytes.is_null() || length > isize::MAX as usize {
        return ptr::null();
    }
    let Ok(address) = usize::try_from(address) else {
        return ptr::null();
    };
    // SAFETY: caller retains length readable immutable resident bytes.
    crate::spectrum::SpectrumData::new(unsafe { std::slice::from_raw_parts(bytes, length) })
        .envelope(address)
        .map_or(ptr::null(), |envelope| envelope.as_ptr())
}
type PhonemeContext = unsafe extern "C" fn(*mut c_void, u32, usize) -> i32;
#[derive(Default)]
#[repr(C)]
struct RawPhonemeEntry {
    phoneme: crate::phoneme::Phoneme,
    present: u32,
    code: u32,
    stress: u32,
    word_stress: u32,
    source: u32,
    flags: u32,
}
type PhonemeStorage = unsafe extern "C" fn(*mut c_void, u32, usize, *mut RawPhonemeEntry) -> i32;
struct BorrowedPhonemes {
    opaque: *mut c_void,
    callback: PhonemeStorage,
}
impl BorrowedPhonemes {
    fn query(&self, kind: u32, value: usize) -> Option<RawPhonemeEntry> {
        let mut entry = RawPhonemeEntry::default();
        // SAFETY: caller retains exclusive live state, bounded indexed access
        // and a callback that only reads/resolves records for this call.
        let status = unsafe { (self.callback)(self.opaque, kind, value, &mut entry) };
        (status == 1).then_some(entry)
    }
}
impl crate::phoneme_context::Storage for BorrowedPhonemes {
    fn entry(
        &self,
        position: crate::phoneme_context::Position,
    ) -> Option<crate::phoneme_context::Entry> {
        use crate::phoneme_context::{Entry, Position};
        let entry = match position {
            Position::List(index) => self.query(0, index),
            Position::PreviousVowel => self.query(3, 0),
        }?;
        Some(Entry {
            phoneme: (entry.present != 0).then_some(entry.phoneme),
            code: entry.code as u8,
            stress: entry.stress as u8,
            word_stress: entry.word_stress as u8,
            source: entry.source as u16,
            flags: entry.flags as u16,
        })
    }
    fn phoneme(&self, code: u8) -> Option<crate::phoneme::Phoneme> {
        self.query(1, usize::from(code))
            .filter(|e| e.present != 0)
            .map(|e| e.phoneme)
    }
    fn refresh(&mut self, position: crate::phoneme_context::Position) {
        use crate::phoneme_context::Position;
        match position {
            Position::List(index) => {
                self.query(2, index);
            }
            Position::PreviousVowel => {
                self.query(4, 0);
            }
        }
    }
    fn invalid_instruction(&mut self, instruction: u16) {
        self.query(5, usize::from(instruction));
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_phoneme_condition(
    settings: *const crate::phoneme_context::Settings,
    instruction: u32,
    selector: i32,
    opaque: *mut c_void,
    callback: Option<PhonemeStorage>,
) -> c_int {
    let Some(callback) = callback else {
        return -1;
    };
    if settings.is_null()
        || instruction > u16::MAX as u32
        || selector < -1
        || selector > i32::from(u16::MAX)
    {
        return -1;
    }
    // SAFETY: caller provides one aligned immutable context description.
    let settings = unsafe { *settings };
    crate::phoneme_context::Context::new(
        crate::phoneme_program::Program::new(&[]).expect("empty words"),
        settings,
        BorrowedPhonemes { opaque, callback },
    )
    .and_then(|mut context| context.evaluate(instruction as u16, u16::try_from(selector).ok()))
    .map_or(-1, c_int::from)
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_phoneme_program_with_context(
    bytes: *const u8,
    length: usize,
    phoneme: *const crate::phoneme::Phoneme,
    settings: *const crate::phoneme_context::Settings,
    opaque: *mut c_void,
    callback: Option<PhonemeStorage>,
    out: *mut crate::phoneme_program::PhonemeData,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if bytes.is_null()
        || phoneme.is_null()
        || settings.is_null()
        || out.is_null()
        || length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: caller retains aligned immutable input records/resident bytes,
    // disjoint from output and the state modified by the storage callback.
    let (bytes, phoneme, settings) = unsafe {
        (
            std::slice::from_raw_parts(bytes, length),
            &*phoneme,
            *settings,
        )
    };
    let result = crate::phoneme_program::Program::new(bytes).and_then(|program| {
        let mut context = crate::phoneme_context::Context::new(
            program,
            settings,
            BorrowedPhonemes { opaque, callback },
        )?;
        program.interpret(
            phoneme,
            settings.control,
            settings.has_translator != 0,
            &mut context,
        )
    });
    let Ok(result) = result else {
        return 2;
    };
    // SAFETY: caller provides one aligned exclusive output with no aliases.
    unsafe {
        write_phoneme_data(out, result);
    }
    0
}
unsafe fn write_phoneme_data(
    out: *mut crate::phoneme_program::PhonemeData,
    result: crate::phoneme_program::PhonemeData,
) {
    // SAFETY: caller of this helper supplies one exclusive aligned output.
    unsafe {
        out.write_bytes(0, 1);
        (*out).control = result.control;
        (*out).parameters = result.parameters;
        (*out).sound_addresses = result.sound_addresses;
        (*out).sound_parameters = result.sound_parameters;
        (*out).vowel_transitions = result.vowel_transitions;
        (*out).pitch_envelope = result.pitch_envelope;
        (*out).amplitude_envelope = result.amplitude_envelope;
        (*out).ipa = result.ipa;
    }
}
struct PhonemeHost {
    opaque: *mut c_void,
    callback: PhonemeContext,
}
impl PhonemeHost {
    fn query(&mut self, kind: u32, value: usize) -> i32 {
        // SAFETY: the caller retains the live exclusive context and serializes
        // synchronous callbacks until interpretation returns.
        unsafe { (self.callback)(self.opaque, kind, value) }
    }
}
impl crate::phoneme_program::Environment for PhonemeHost {
    fn condition(
        &mut self,
        offset: usize,
    ) -> Result<bool, crate::phoneme_data::InvalidPhonemeData> {
        match self.query(0, offset) {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(crate::phoneme_data::InvalidPhonemeData(
                "invalid host phoneme condition",
            )),
        }
    }
    fn stress(&mut self, condition: u8) -> bool {
        self.query(1, usize::from(condition)) > 0
    }
    fn next_is_vowel(&mut self) -> bool {
        self.query(2, 0) > 0
    }
    fn vowel_type(&mut self, next: bool) -> Option<u8> {
        u8::try_from(self.query(if next { 3 } else { 4 }, 0)).ok()
    }
    fn invalid_instruction(&mut self, instruction: u16) {
        self.query(5, usize::from(instruction));
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_phoneme_program(
    bytes: *const u8,
    length: usize,
    phoneme: *const crate::phoneme::Phoneme,
    control: u32,
    has_translator: u32,
    opaque: *mut c_void,
    callback: Option<PhonemeContext>,
    out: *mut crate::phoneme_program::PhonemeData,
) -> c_int {
    let Some(callback) = callback else {
        return 2;
    };
    if bytes.is_null() || phoneme.is_null() || out.is_null() || length > isize::MAX as usize {
        return 2;
    }
    // SAFETY: caller retains immutable resident bytes and an aligned phoneme
    // record for the full synchronous call. No callback changes these inputs.
    let (bytes, phoneme) = unsafe { (std::slice::from_raw_parts(bytes, length), &*phoneme) };
    let mut host = PhonemeHost { opaque, callback };
    let result = crate::phoneme_program::Program::new(bytes)
        .and_then(|program| program.interpret(phoneme, control, has_translator != 0, &mut host));
    let Ok(result) = result else {
        return 2;
    };
    // SAFETY: caller provides one aligned output, exclusive and disjoint from
    // resident data and the context touched by callbacks.
    unsafe {
        // Keep C-visible tail padding deterministic, as in legacy memset.
        write_phoneme_data(out, result);
    }
    0
}

type RulePredicate = unsafe extern "C" fn(*mut c_void, u32, u32, usize, u32) -> i32;
type PrefixFlags = unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut u32);
type RuleTrace = unsafe extern "C" fn(*mut c_void, usize, usize, i32);
#[repr(C)]
struct RawLetters {
    bits: *const [u8; 256],
    groups: *const [*const c_void; 8],
    lengths: *const [usize; 8],
    offset: i32,
    wide_bytes: u32,
}
impl RawLetters {
    /// Host retains aligned, immutable configuration and wide lists throughout
    /// the synchronous call. Lengths exclude each list's terminating NUL.
    unsafe fn borrow(&self) -> Option<crate::letters::LetterSet<'_>> {
        if self.bits.is_null()
            || self.groups.is_null()
            || self.lengths.is_null()
            || !matches!(self.wide_bytes, 2 | 4)
        {
            return None;
        }
        // SAFETY: host supplies three live arrays of the declared fixed sizes.
        let (bits, pointers, lengths) = unsafe { (&*self.bits, &*self.groups, &*self.lengths) };
        let mut groups = [None; 8];
        for (index, &pointer) in pointers.iter().enumerate() {
            if pointer.is_null() {
                continue;
            }
            let length = lengths[index];
            if length > isize::MAX as usize / self.wide_bytes as usize {
                return None;
            }
            // SAFETY: each non-null pointer has length aligned readable units
            // of the host's wchar_t width, immutable until this borrow ends.
            groups[index] = Some(unsafe {
                if self.wide_bytes == 2 {
                    crate::letters::WideLetters::U16(std::slice::from_raw_parts(
                        pointer.cast(),
                        length,
                    ))
                } else {
                    crate::letters::WideLetters::U32(std::slice::from_raw_parts(
                        pointer.cast(),
                        length,
                    ))
                }
            });
        }
        Some(crate::letters::LetterSet {
            bits,
            offset: self.offset,
            groups,
        })
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_is_letter(
    letters: *const RawLetters,
    letter: i32,
    group: u32,
) -> c_int {
    if letters.is_null() {
        return 0;
    }
    // SAFETY: host supplies one aligned configuration valid for this call.
    unsafe { (&*letters).borrow() }.map_or(0, |set| c_int::from(set.mask(letter, group)))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_letter_group(
    patterns: *const u8,
    length: usize,
    text: *const u8,
    text_length: usize,
    position: usize,
    backwards: c_int,
) -> c_int {
    if patterns.is_null()
        || text.is_null()
        || length > isize::MAX as usize
        || text_length > isize::MAX as usize
    {
        return -1;
    }
    // SAFETY: the synchronous host declares readable resident patterns and text
    // of these lengths; neither buffer is modified during matching.
    let (patterns, text) = unsafe {
        (
            std::slice::from_raw_parts(patterns, length),
            std::slice::from_raw_parts(text, text_length),
        )
    };
    crate::rule_match::letter_group(patterns, text, position, backwards != 0)
        .ok()
        .flatten()
        .and_then(|n| c_int::try_from(n).ok())
        .unwrap_or(-1)
}
struct RuleHost<'a> {
    letters: crate::letters::LetterSet<'a>,
    opaque: *mut c_void,
    predicate: RulePredicate,
    prefix: PrefixFlags,
    trace: RuleTrace,
}
impl crate::rule_match::Environment for RuleHost<'_> {
    fn is_letter(&mut self, code: u32, group: u8) -> bool {
        self.letters.is_letter(code, group)
    }
    fn letter_group(
        &mut self,
        _text: &[u8],
        position: usize,
        group: u8,
        backwards: bool,
    ) -> Option<usize> {
        // SAFETY: matcher bounds-checks positions in the declared host text.
        let result = unsafe {
            (self.predicate)(
                self.opaque,
                1,
                u32::from(backwards),
                position,
                u32::from(group),
            )
        };
        usize::try_from(result).ok()
    }
    fn prefix_flags(&mut self, prefix: &[u8]) -> [u32; 2] {
        let mut flags = [0; 2];
        // SAFETY: prefix is a live NUL-terminated stack slice for this callback;
        // flags reserves two exclusively borrowed output words.
        unsafe {
            (self.prefix)(
                self.opaque,
                prefix.as_ptr(),
                prefix.len(),
                flags.as_mut_ptr(),
            )
        };
        flags
    }
    fn trace(&mut self, template: usize, phonemes: Option<usize>, points: i32) {
        // SAFETY: offsets borrow the live rule storage held by the C host.
        unsafe {
            (self.trace)(
                self.opaque,
                template,
                phonemes.unwrap_or(usize::MAX),
                points,
            )
        };
    }
}
#[repr(C)]
struct RawRuleMatch {
    phonemes: usize,
    delete_offset: usize,
    advance: usize,
    points: i32,
    ending: i32,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_match_group(
    rules: *const u8,
    rules_length: usize,
    text: *const u8,
    text_length: usize,
    position: usize,
    group_length: usize,
    context: *const crate::rule_match::Context,
    letters: *const RawLetters,
    opaque: *mut c_void,
    predicate: Option<RulePredicate>,
    prefix: Option<PrefixFlags>,
    trace: Option<RuleTrace>,
    out: *mut RawRuleMatch,
) -> c_int {
    let (Some(predicate), Some(prefix), Some(trace)) = (predicate, prefix, trace) else {
        return 2;
    };
    if rules.is_null()
        || text.is_null()
        || context.is_null()
        || letters.is_null()
        || out.is_null()
        || rules_length > isize::MAX as usize
        || text_length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: caller declares live, readable and immutable text/rule buffers
    // and aligned context, valid throughout this synchronous execution.
    let (rules, text, context) = unsafe {
        (
            std::slice::from_raw_parts(rules, rules_length),
            std::slice::from_raw_parts(text, text_length),
            &*context,
        )
    };
    // SAFETY: caller retains immutable language arrays throughout matching.
    let Some(letters) = (unsafe { (&*letters).borrow() }) else {
        return 2;
    };
    let mut host = RuleHost {
        letters,
        opaque,
        predicate,
        prefix,
        trace,
    };
    let Ok(result) =
        crate::rule_match::match_group(rules, text, position, group_length, context, &mut host)
    else {
        return 2;
    };
    // SAFETY: caller provides one aligned, exclusive output record.
    unsafe {
        out.write(RawRuleMatch {
            phonemes: result.phonemes.unwrap_or(usize::MAX),
            delete_offset: result.delete.unwrap_or(usize::MAX),
            advance: result.advance,
            points: result.points,
            ending: result.ending,
        })
    };
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_transpose(
    text: *mut u8,
    length: usize,
    min: u32,
    max: u32,
    map: *const u8,
    map_length: usize,
    pairs: *const i16,
    pairs_length: usize,
) -> c_int {
    if text.is_null()
        || length > isize::MAX as usize
        || map_length > isize::MAX as usize
        || pairs_length > isize::MAX as usize / 2
    {
        return -1;
    }
    // SAFETY: adapter declares exclusive NUL-containing text storage and live
    // map/pair arrays. All three allocations are disjoint.
    let text = unsafe { std::slice::from_raw_parts_mut(text, length) };
    let map = if map.is_null() {
        None
    } else {
        // SAFETY: the map has map_length readable byte entries.
        Some(unsafe { std::slice::from_raw_parts(map, map_length) })
    };
    let pairs = if pairs.is_null() {
        &[]
    } else {
        // SAFETY: pairs contains pairs_length aligned readable i16 entries.
        unsafe { std::slice::from_raw_parts(pairs, pairs_length) }
    };
    crate::word_key::Alphabet {
        min,
        max,
        map,
        pairs,
    }
    .transpose(text)
    .map_or(-1, |length| length as c_int)
}

#[repr(C)]
struct RawLookupOutcome {
    phonemes_offset: usize,
    phonemes_length: usize,
    word_end: usize,
    flags: [u32; 2],
    trace_flags: [u32; 2],
    copied: u32,
    has_flags: u32,
    found: u32,
    skipwords: i32,
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_lookup_bucket(
    bucket: *const u8,
    length: usize,
    key: *const c_char,
    descriptor: usize,
    next: *const c_char,
    next_length: usize,
    context: *const crate::lookup::Context,
    words: *const crate::lookup::WordInfo,
    word_count: usize,
    out: *mut RawLookupOutcome,
) -> c_int {
    if bucket.is_null()
        || key.is_null()
        || next.is_null()
        || context.is_null()
        || out.is_null()
        || length > isize::MAX as usize
        || next_length > isize::MAX as usize
        || word_count > 20
    {
        return 2;
    }
    // SAFETY: the C adapter borrows a validated resident bucket of this length.
    let bucket = unsafe { std::slice::from_raw_parts(bucket, length) };
    // SAFETY: compressed keys contain embedded NULs. The adapter guarantees
    // the descriptor's byte-count prefix is readable; it must not be shortened
    // by CStr. Next words are an independently bounded window.
    let (key, next, context) = unsafe {
        (
            std::slice::from_raw_parts(key.cast::<u8>(), descriptor & 0x3f),
            std::slice::from_raw_parts(next.cast::<u8>(), next_length),
            &*context,
        )
    };
    let words = if words.is_null() {
        None
    } else {
        // SAFETY: the word snapshot contains word_count initialized entries.
        Some(unsafe { std::slice::from_raw_parts(words, word_count) })
    };
    let Ok(result) = crate::lookup::lookup_bucket(bucket, key, descriptor, next, context, words)
    else {
        return 2;
    };
    let phonemes = result.phonemes.unwrap_or(&[]);
    let offset = if phonemes.is_empty() {
        0
    } else {
        phonemes.as_ptr() as usize - bucket.as_ptr() as usize
    };
    // SAFETY: caller reserves one exclusive output record of the C-declared layout.
    unsafe {
        out.write(RawLookupOutcome {
            phonemes_offset: offset,
            phonemes_length: phonemes.len(),
            word_end: result.word_end.unwrap_or(0),
            flags: result.flags.unwrap_or([0; 2]),
            trace_flags: result.trace_flags.unwrap_or([0; 2]),
            copied: u32::from(result.phonemes.is_some()),
            has_flags: u32::from(result.flags.is_some()),
            found: u32::from(result.word_end.is_some()),
            skipwords: result.skipwords.map_or(-1, |count| count as i32),
        })
    };
    0
}

#[no_mangle]
unsafe extern "C" fn HashDictionary(word: *const c_char) -> c_int {
    if word.is_null() {
        return 0;
    }
    // SAFETY: legacy API supplies a readable NUL-terminated word.
    crate::dictionary::hash(unsafe { CStr::from_ptr(word) }.to_bytes()) as c_int
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_dictionary_index(
    bytes: *const u8,
    length: usize,
    rules: *mut crate::rules::RuleIndex,
    buckets: *mut usize,
    rules_offset: *mut usize,
) -> c_int {
    if bytes.is_null()
        || rules.is_null()
        || buckets.is_null()
        || rules_offset.is_null()
        || length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: the adapter supplies length readable bytes and exclusive outputs.
    let data = unsafe { std::slice::from_raw_parts(bytes, length) };
    let Ok(dict) = crate::dictionary::Dictionary::parse(data) else {
        return 2;
    };
    let Ok(index) = dict.rule_index() else {
        return 2;
    };
    // SAFETY: outputs have the declared layout, with 1024 bucket slots.
    unsafe {
        rules.write(index);
        ptr::copy_nonoverlapping(dict.bucket_offsets().as_ptr(), buckets, 1024);
        rules_offset.write(dict.rules_offset());
    }
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_create(
    bytes: *const u8,
    length: usize,
    tables: *mut crate::phoneme_data::TableMeta,
    count: *mut c_int,
) -> *mut crate::phoneme_data::TableIndex {
    if bytes.is_null() || tables.is_null() || count.is_null() || length > isize::MAX as usize {
        return ptr::null_mut();
    }
    // SAFETY: the adapter provides a resident buffer of length readable bytes.
    let Ok(index) = crate::phoneme_data::TableIndex::parse(unsafe {
        std::slice::from_raw_parts(bytes, length)
    }) else {
        return ptr::null_mut();
    };
    // SAFETY: caller reserves 150 metadata slots and one count.
    unsafe {
        ptr::copy_nonoverlapping(index.tables().as_ptr(), tables, index.tables().len());
        count.write(index.tables().len() as c_int);
    }
    Box::into_raw(Box::new(index))
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_destroy(index: *mut crate::phoneme_data::TableIndex) {
    if !index.is_null() {
        // SAFETY: the adapter frees each handle returned by create exactly once.
        drop(unsafe { Box::from_raw(index) });
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_select(
    index: *const crate::phoneme_data::TableIndex,
    bytes: *const u8,
    length: usize,
    number: c_int,
    slots: *mut usize,
) -> c_int {
    if index.is_null()
        || bytes.is_null()
        || slots.is_null()
        || number < 0
        || length > isize::MAX as usize
    {
        return 2;
    }
    // SAFETY: live create handle, resident bytes and exclusive 256-slot output.
    let selected = unsafe { &*index }.select(
        unsafe { std::slice::from_raw_parts(bytes, length) },
        number as usize,
    );
    let Ok(selected) = selected else {
        return 2;
    };
    // SAFETY: exactly 256 exclusive usize output slots, as declared in the C header.
    unsafe { ptr::copy_nonoverlapping(selected.as_ptr(), slots, 256) };
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phontab_lookup(
    index: *const crate::phoneme_data::TableIndex,
    name: *const c_char,
) -> c_int {
    if index.is_null() || name.is_null() {
        return -1;
    }
    // SAFETY: live index handle and readable NUL-terminated table name.
    unsafe { &*index }
        .lookup(unsafe { CStr::from_ptr(name) }.to_bytes())
        .map_or(-1, |number| number as c_int)
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_phondata_header(
    bytes: *const u8,
    length: usize,
    fields: *mut u32,
) -> c_int {
    if bytes.is_null() || fields.is_null() || length > isize::MAX as usize {
        return 2;
    }
    // SAFETY: the adapter supplies length readable bytes and two exclusive u32 outputs.
    let Ok(header) =
        crate::phoneme_data::header(unsafe { std::slice::from_raw_parts(bytes, length) })
    else {
        return 2;
    };
    // SAFETY: the C adapter reserves exactly two u32 fields.
    unsafe { ptr::copy_nonoverlapping(header.as_ptr(), fields, 2) };
    0
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_sample_rate(bytes: *const u8, length: usize) -> c_int {
    if bytes.is_null() || length > isize::MAX as usize {
        return -1;
    }
    // SAFETY: caller supplies length readable bytes of phondata.
    crate::phoneme_data::sample_rate(unsafe { std::slice::from_raw_parts(bytes, length) })
        .map_or(-1, |rate| rate as c_int)
}

#[no_mangle]
unsafe extern "C" fn phoneme_feature_from_string(name: *const c_char) -> u32 {
    if name.is_null() {
        return 0;
    }
    // SAFETY: caller supplies a NUL-terminated feature string.
    crate::phoneme::feature_from_name(unsafe { CStr::from_ptr(name) }.to_bytes())
}
#[no_mangle]
unsafe extern "C" fn phoneme_add_feature(
    phoneme: *mut crate::phoneme::Phoneme,
    feature: u32,
) -> c_int {
    if phoneme.is_null() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: caller supplies an aligned, exclusive PHONEME_TAB record.
    unsafe { &mut *phoneme }
        .add_feature(feature)
        .map_or(0x10000fff, |()| 0)
}

struct RawDecoder {
    input: *const u8,
    length: usize,
    state: State,
}

#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_eof(pending: i32, decoder: *mut RawDecoder) -> i32 {
    if pending != 0 {
        return 0;
    }
    let cursor = crate::clause_input::Cursor { pending, count: 0 };
    // SAFETY: optional serialized live decoder and its retained input; no callback.
    i32::from(cursor.eof(unsafe { text_decoder_eof(decoder) } != 0))
}
#[no_mangle]
unsafe extern "C" fn espeak_rs_clause_getc(
    pending: *mut i32,
    count: *mut i32,
    decoder: *mut RawDecoder,
) -> i32 {
    if pending.is_null() || count.is_null() {
        return 0;
    }
    // SAFETY: initialized exclusive/disjoint scalar fields retained by owner.
    let mut cursor = unsafe {
        crate::clause_input::Cursor {
            pending: *pending,
            count: *count,
        }
    };
    let result = cursor.read(|| {
        // SAFETY: optional serialized decoder and borrowed input stay live.
        // Count admission precedes source advancement; no C/user callback runs.
        unsafe { text_decoder_getc(decoder) }
    });
    let Ok(value) = result else { return 0 };
    // SAFETY: publish copied admitted cursor to the same exclusive fields.
    unsafe {
        *pending = cursor.pending;
        *count = cursor.count
    };
    value
}

impl RawDecoder {
    unsafe fn read(&mut self) -> Option<u32> {
        if self.input.is_null() || self.state.offset >= self.length {
            return None;
        }
        // SAFETY: C's borrowed-buffer contract guarantees length readable bytes.
        let bytes = unsafe { std::slice::from_raw_parts(self.input, self.length) };
        self.state.read(bytes)
    }
}

#[no_mangle]
extern "C" fn create_text_decoder() -> *mut RawDecoder {
    // SAFETY: correct layout; allocation failure is checked before writing.
    let result = unsafe { alloc(Layout::new::<RawDecoder>()) }.cast::<RawDecoder>();
    if !result.is_null() {
        // SAFETY: newly allocated, aligned storage of the correct size.
        unsafe {
            result.write(RawDecoder {
                input: ptr::null(),
                length: 0,
                state: State::new(Encoding::UsAscii, Mode::Bytes).expect("known encoding"),
            });
        }
    }
    result
}

#[no_mangle]
unsafe extern "C" fn destroy_text_decoder(decoder: *mut RawDecoder) {
    if !decoder.is_null() {
        // SAFETY: pointer comes from create_text_decoder and is freed once.
        unsafe {
            dealloc(decoder.cast(), Layout::new::<RawDecoder>());
        }
    }
}

#[no_mangle]
unsafe extern "C" fn espeak_ng_EncodingFromName(name: *const c_char) -> u32 {
    if name.is_null() {
        return Encoding::Unknown as u32;
    }
    // SAFETY: C supplies a live NUL-terminated string.
    unsafe { CStr::from_ptr(name) }
        .to_str()
        .map_or(0, |s| Encoding::from_name(s) as u32)
}

unsafe fn bind(
    decoder: *mut RawDecoder,
    input: *const u8,
    length: c_int,
    encoding: u32,
    mode: Mode,
) -> c_int {
    let Some(encoding) = Encoding::from_id(encoding) else {
        return UNKNOWN_ENCODING;
    };
    let Ok(state) = State::new(encoding, mode) else {
        return UNKNOWN_ENCODING;
    };
    if decoder.is_null() {
        return INVALID_ARGUMENT;
    }
    let length = if input.is_null() {
        0
    } else if length < 0 {
        // SAFETY: for a negative length the C contract requires a NUL string.
        unsafe { CStr::from_ptr(input.cast()) }
            .to_bytes_with_nul()
            .len()
    } else {
        length as usize
    };
    // SAFETY: decoder is a live, exclusively accessed instance from create.
    unsafe {
        *decoder = RawDecoder {
            input,
            length,
            state,
        };
    }
    0
}

#[no_mangle]
unsafe extern "C" fn text_decoder_decode_string(
    decoder: *mut RawDecoder,
    input: *const c_char,
    length: c_int,
    encoding: u32,
) -> c_int {
    // SAFETY: forwards the same borrowed-buffer contract.
    unsafe { bind(decoder, input.cast(), length, encoding, Mode::Bytes) }
}
#[no_mangle]
unsafe extern "C" fn text_decoder_decode_string_auto(
    decoder: *mut RawDecoder,
    input: *const c_char,
    length: c_int,
    encoding: u32,
) -> c_int {
    // SAFETY: forwards the same borrowed-buffer contract.
    unsafe { bind(decoder, input.cast(), length, encoding, Mode::Auto) }
}

#[cfg(windows)]
type WChar = u16;
#[cfg(not(windows))]
type WChar = u32;

#[no_mangle]
unsafe extern "C" fn text_decoder_decode_wstring(
    decoder: *mut RawDecoder,
    input: *const WChar,
    length: c_int,
) -> c_int {
    if decoder.is_null() {
        return INVALID_ARGUMENT;
    }
    let units = if input.is_null() {
        0
    } else if length < 0 {
        let mut count = 0;
        // SAFETY: C promises a live, NUL-terminated wchar_t array.
        while unsafe { input.add(count).read() } != 0 {
            count += 1;
        }
        count + 1
    } else {
        length as usize
    };
    let Some(length) = units
        .checked_mul(std::mem::size_of::<WChar>())
        .filter(|len| *len <= isize::MAX as usize)
    else {
        return INVALID_ARGUMENT;
    };
    let mode = if std::mem::size_of::<WChar>() == 2 {
        Mode::Wide16
    } else {
        Mode::Wide32
    };
    let state = State::new(Encoding::UsAscii, mode).expect("known encoding");
    // SAFETY: live, exclusively accessed decoder; input extent checked above.
    unsafe {
        *decoder = RawDecoder {
            input: input.cast(),
            length,
            state,
        };
    }
    0
}

#[no_mangle]
unsafe extern "C" fn text_decoder_decode_string_multibyte(
    decoder: *mut RawDecoder,
    input: *const c_void,
    encoding: u32,
    flags: c_int,
) -> c_int {
    // SAFETY: each mode inherits the corresponding C API buffer contract.
    unsafe {
        match flags & 7 {
            0 => text_decoder_decode_string_auto(decoder, input.cast(), -1, encoding),
            1 => text_decoder_decode_string(decoder, input.cast(), -1, Encoding::Utf8 as u32),
            2 => text_decoder_decode_string(decoder, input.cast(), -1, encoding),
            3 => text_decoder_decode_wstring(decoder, input.cast(), -1),
            4 => text_decoder_decode_string(decoder, input.cast(), -1, Encoding::Ucs2 as u32),
            _ => UNKNOWN_ENCODING,
        }
    }
}

#[no_mangle]
unsafe extern "C" fn text_decoder_eof(decoder: *mut RawDecoder) -> c_int {
    if decoder.is_null() {
        return 1;
    }
    // SAFETY: C supplies a live decoder with serialized access.
    let d = unsafe { &*decoder };
    c_int::from(d.state.offset >= d.length)
}
#[no_mangle]
unsafe extern "C" fn text_decoder_getc(decoder: *mut RawDecoder) -> u32 {
    if decoder.is_null() {
        return 0;
    }
    // SAFETY: C supplies a live decoder and retains its bound input.
    unsafe { (&mut *decoder).read() }.unwrap_or(0)
}
#[no_mangle]
unsafe extern "C" fn text_decoder_peekc(decoder: *mut RawDecoder) -> u32 {
    if decoder.is_null() {
        return 0;
    }
    // SAFETY: C supplies a live decoder with serialized access.
    let d = unsafe { &mut *decoder };
    let offset = d.state.offset;
    // SAFETY: C retains the input bound to this decoder.
    let result = unsafe { d.read() }.unwrap_or(0);
    d.state.offset = offset; // intentionally retains AUTO fallback selected by read
    result
}
#[no_mangle]
unsafe extern "C" fn text_decoder_get_buffer(decoder: *mut RawDecoder) -> *const c_void {
    if decoder.is_null() {
        return ptr::null();
    }
    // SAFETY: C supplies a live decoder with serialized access.
    let d = unsafe { &*decoder };
    if d.input.is_null() || d.state.offset >= d.length {
        return ptr::null();
    }
    // SAFETY: cursor is bounded by the bound input's extent.
    unsafe { d.input.add(d.state.offset) }.cast()
}

#[repr(C)]
struct Mnem {
    name: *const c_char,
    value: c_int,
}

#[no_mangle]
unsafe extern "C" fn LookupMnem(mut table: *const Mnem, name: *const c_char) -> c_int {
    if table.is_null() {
        return 0;
    }
    let name = if name.is_null() {
        None
    } else {
        // SAFETY: caller supplies a NUL-terminated name.
        Some(unsafe { CStr::from_ptr(name) }.to_bytes())
    };
    loop {
        // SAFETY: C supplies a readable table with a NULL-name sentinel.
        let item = unsafe { &*table };
        if item.name.is_null() {
            return item.value;
        }
        // SAFETY: each non-sentinel name is NUL-terminated.
        if name == Some(unsafe { CStr::from_ptr(item.name) }.to_bytes()) {
            return item.value;
        }
        // SAFETY: the sentinel contract permits visiting the next entry.
        table = unsafe { table.add(1) };
    }
}
#[no_mangle]
unsafe extern "C" fn LookupMnemName(mut table: *const Mnem, value: c_int) -> *const c_char {
    if !table.is_null() {
        loop {
            // SAFETY: C supplies a readable table with a NULL-name sentinel.
            let item = unsafe { &*table };
            if item.name.is_null() {
                break;
            }
            if item.value == value {
                return item.name;
            }
            // SAFETY: the sentinel contract permits visiting the next entry.
            table = unsafe { table.add(1) };
        }
    }
    c"".as_ptr()
}
#[no_mangle]
unsafe extern "C" fn ieee_extended_to_double(input: *const u8) -> f64 {
    if input.is_null() {
        return f64::NAN;
    }
    // SAFETY: caller supplies ten readable bytes; no alignment requirement.
    let bytes = unsafe { input.cast::<[u8; 10]>().read() };
    crate::ieee80::extended_to_double(&bytes)
}

macro_rules! unicode_exports {
    ($( $name:ident => $rust:ident ),* $(,)?) => { $(
        #[no_mangle]
        extern "C" fn $name(c: u32) -> c_int { c_int::from(unicode::$rust(c)) }
    )* };
}
unicode_exports! {
    ucd_isalnum => is_alnum, ucd_isalpha => is_alpha, ucd_isblank => is_blank,
    ucd_iscntrl => is_control, ucd_isdigit => is_digit, ucd_isgraph => is_graph,
    ucd_islower => is_lower, ucd_isprint => is_print, ucd_ispunct => is_punct,
    ucd_isspace => is_space, ucd_isupper => is_upper, ucd_isxdigit => is_hex_digit,
}
#[no_mangle]
extern "C" fn ucd_toupper(c: u32) -> u32 {
    unicode::to_upper(c)
}
#[no_mangle]
extern "C" fn ucd_tolower(c: u32) -> u32 {
    unicode::to_lower(c)
}
#[no_mangle]
extern "C" fn ucd_totitle(c: u32) -> u32 {
    unicode::to_title(c)
}
#[no_mangle]
extern "C" fn ucd_lookup_category(c: u32) -> u32 {
    unicode::category(c) as u32
}
#[no_mangle]
extern "C" fn ucd_lookup_category_group(c: u32) -> u32 {
    unicode::category(c).group()
}
#[no_mangle]
extern "C" fn ucd_lookup_script(c: u32) -> u32 {
    unicode::script(c)
}
#[no_mangle]
extern "C" fn ucd_get_category_group_for_category(cat: u32) -> u32 {
    Category::from_id(cat).map_or(1, Category::group)
}
#[no_mangle]
extern "C" fn ucd_properties(c: u32, cat: u32) -> u64 {
    Category::from_id(cat).map_or(0, |cat| unicode::properties(c, cat))
}
#[no_mangle]
extern "C" fn ucd_get_category_string(cat: u32) -> *const c_char {
    unicode::category_c_string(cat).as_ptr().cast()
}
#[no_mangle]
extern "C" fn ucd_get_script_string(id: u32) -> *const c_char {
    unicode::script_c_string(id).as_ptr().cast()
}
#[no_mangle]
extern "C" fn ucd_get_category_group_string(id: u32) -> *const c_char {
    [c"C", c"I", c"L", c"M", c"N", c"P", c"S", c"Z"]
        .get(id as usize)
        .copied()
        .unwrap_or(c"-")
        .as_ptr()
}
