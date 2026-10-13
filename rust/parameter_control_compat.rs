//! Stable Rust-owned parameter storage and bounded native control projections.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    parameter_control::{self, Effect, SpeedHost, EMBEDDED_LIMIT},
    ssml_parameters::{Frame, PARAMETERS, STACK},
};
use std::ffi::c_int;
// The serialized hybrid engine still borrows these initialized stable views.
// No Rust reference to them survives an engine callback; no instance is moved.
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut param_stack: [Frame; STACK] = [Frame {
    kind: 0,
    values: [0; PARAMETERS],
}; STACK];
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut saved_parameters: [i32; PARAMETERS] = [0; PARAMETERS];
#[no_mangle]
#[allow(non_upper_case_globals)]
static param_defaults: [i32; PARAMETERS] = parameter_control::DEFAULTS;
#[repr(C)]
pub struct Parameter {
    pub(super) defaults: *const i32,
    pub(super) current: *mut i32,
    pub(super) saved: *mut i32,
    pub(super) embedded: *mut i32,
    pub(super) line: *mut i32,
    pub(super) gap: *mut i32,
    pub(super) tone: *mut i32,
    pub(super) group: *mut i32,
    pub(super) speed: Option<unsafe extern "C" fn(c_int)>,
    pub(super) amplitude: Option<unsafe extern "C" fn() -> c_int>,
}
#[repr(C)]
pub struct Embedded {
    pub(super) words: *const u32,
    pub(super) length: usize,
    pub(super) cursor: *mut i32,
    pub(super) speeds: *const i32,
    pub(super) set: Option<unsafe extern "C" fn(c_int, c_int)>,
    pub(super) speed: Option<unsafe extern "C" fn(c_int)>,
}
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<Parameter>() == 80);
    assert!(std::mem::size_of::<Embedded>() == 48);
    assert!(std::mem::size_of::<Frame>() == 64);
};
/// Initialized descriptor/defaults, aligned writable current/saved/embedded
/// arrays (15 each) and option scalars, mutually disjoint. Defaults/descriptor
/// may overlap outputs; snapshots precede all writes. Engine access serialized,
/// callbacks synchronous without retention/unwind or access to input scratch.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_set_parameter(
    d: *const Parameter,
    parameter: c_int,
    value: c_int,
    relative: c_int,
) -> c_int {
    if d.is_null() {
        return 22;
    }
    // SAFETY: initialized aligned descriptor copied before writes/effects.
    let d = unsafe { d.read() };
    if d.defaults.is_null()
        || d.current.is_null()
        || d.saved.is_null()
        || d.embedded.is_null()
        || d.line.is_null()
        || d.gap.is_null()
        || d.tone.is_null()
    {
        return 22;
    }
    // SAFETY: 15 initialized defaults, copied before any overlapping publication.
    let defaults = unsafe { d.defaults.cast::<[i32; PARAMETERS]>().read() };
    let Ok(plan) = parameter_control::parameter(&defaults, parameter, value, relative != 0) else {
        return 22;
    };
    if matches!(plan.effect, Effect::Rate(_)) && d.speed.is_none()
        || matches!(plan.effect, Effect::Volume(_)) && d.amplitude.is_none()
        || matches!(plan.effect, Effect::Tone { group: Some(_), .. }) && d.group.is_null()
    {
        return 22;
    }
    // SAFETY: complete admission precedes publication, disjoint initialized
    // scalars/arrays. No reference to foreign state spans a primitive effect.
    unsafe {
        d.current.add(plan.index).write(plan.stored);
        d.saved.add(plan.index).write(plan.stored);
        match plan.effect {
            Effect::Rate(v) => {
                d.embedded.add(2).write(v);
                d.embedded.add(8).write(v);
                d.speed.unwrap()(3);
            }
            Effect::Volume(v) => {
                d.embedded.add(3).write(v);
                d.amplitude.unwrap()();
            }
            Effect::Pitch(v) => d.embedded.add(1).write(v),
            Effect::Range(v) => d.embedded.add(4).write(v),
            Effect::Line(v) => d.line.write(v),
            Effect::Gap(v) => d.gap.write(v),
            Effect::Tone { flags, group } => {
                if let Some(v) = group {
                    d.group.write(v);
                }
                d.tone.write(flags);
            }
            Effect::None => {}
        }
    }
    plan.status
}
struct SpeedEffects {
    d: Embedded,
}
impl SpeedHost for SpeedEffects {
    fn advance(&mut self, cursor: usize) {
        // SAFETY: copied descriptor's admitted disjoint writable scalar.
        unsafe { self.d.cursor.write(cursor as i32) };
    }
    fn speed(&mut self, command: i32, value: i32) {
        // SAFETY: synchronous engine primitives, no foreign source/reference
        // loan spans either effect; input values fit the declared ABI.
        unsafe {
            self.d.set.unwrap()(command, value);
            self.d.speed.unwrap()(1);
        }
    }
}
/// Read only through the admitted terminator within length (<=250), copy that
/// span before effects. Cursor/output/speed scalars initialized and mutually
/// disjoint; input/descriptor may overlap them. Callbacks serialized, cannot
/// retain loans or unwind. Reject incomplete spans without effects/publication.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_length_embedded(d: *const Embedded, out: *mut i32) -> c_int {
    if d.is_null() || out.is_null() {
        return 22;
    }
    // SAFETY: aligned initialized descriptor copied before source/output access.
    let d = unsafe { d.read() };
    if d.words.is_null()
        || d.cursor.is_null()
        || d.speeds.is_null()
        || d.set.is_none()
        || d.speed.is_none()
        || d.length > EMBEDDED_LIMIT
    {
        return 22;
    }
    // SAFETY: initialized admitted cursor copied before any effect/publication.
    let cursor = unsafe { d.cursor.read() };
    let Some(start) = usize::try_from(cursor).ok().filter(|i| *i < d.length) else {
        return 22;
    };
    let mut words = [0; EMBEDDED_LIMIT];
    let mut end = 0;
    for (i, word) in words.iter_mut().enumerate().take(d.length).skip(start) {
        // SAFETY: source initialized through its terminator or entire bounded
        // admitted suffix on rejection; snapshot ends before callbacks/writes.
        *word = unsafe { *d.words.add(i) };
        if *word & 128 != 0 {
            end = i + 1;
            break;
        }
    }
    if end == 0 {
        return 22;
    }
    let mut host = SpeedEffects { d };
    if parameter_control::embedded_speed(&words[..end], start, &mut host).is_err() {
        return 22;
    }
    // SAFETY: completed effects before reading the fresh initialized speeds;
    // owned copy precedes publication to the disjoint initialized output.
    let speeds = unsafe { host.d.speeds.cast::<[i32; 3]>().read() };
    // SAFETY: three admitted aligned writable output integers.
    unsafe { out.cast::<[i32; 3]>().write(speeds) };
    0
}
/// Mutually disjoint writable pointer outputs; refusal leaves both unchanged.
/// Published pointers refer to immutable process-lifetime Rust tables.
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_length_mods(
    value: c_int,
    first: *mut *const u8,
    second: *mut *const u8,
) -> c_int {
    if first.is_null() || second.is_null() {
        return 22;
    }
    let Ok((a, b)) = parameter_control::length_mods(value) else {
        return 22;
    };
    // SAFETY: admitted writable disjoint pointer scalars; stable owned tables.
    unsafe {
        first.write(a.as_ptr());
        second.write(b.as_ptr());
    }
    0
}
