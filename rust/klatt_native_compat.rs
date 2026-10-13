//! Complete native Klatt host and stable process-wide kernel slot.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case)]
use super::{klatt_compat as kernel, wavegen_native_compat as backend};
use crate::{
    formant::Frame, klatt::Klatt, output::Output, voice::Voice, wave_memory::WaveMemory,
    wavegen::WgenData,
};
use std::{
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
extern "C" {
    static mut espeak_rs_wave_memory: WaveMemory;
    static mut espeak_rs_output: Output;
    fn espeak_rs_rand(minimum: std::ffi::c_long, maximum: std::ffi::c_long) -> std::ffi::c_long;
    fn KlattInitSP();
    fn KlattResetSP();
    fn KlattFiniSP();
    fn Wavegen_KlattSP(
        data: *mut WgenData,
        voice: *const Voice,
        length: i32,
        resume: i32,
        first: *const Frame,
        last: *const Frame,
    ) -> i32;
}
// Historical kernel/source histories survive Fini/Init. Reclaiming this slot
// requires engine-wide quiescence and retirement of escaped compatibility views.
static STATE: OnceLock<usize> = OnceLock::new();
static BUSY: AtomicBool = AtomicBool::new(false);
struct Lease;
impl Lease {
    fn enter() -> Option<Self> {
        BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
fn state() -> *mut Klatt {
    *STATE.get_or_init(|| Box::into_raw(Box::new(Klatt::new())) as usize) as *mut Klatt
}
unsafe extern "C" fn random() -> i32 {
    // SAFETY: serialized native random stream, matching old int narrowing.
    unsafe { espeak_rs_rand(-8191, 8191) as i32 }
}
unsafe extern "C" fn reset_player() {
    // SAFETY: separate initialized player state; no Klatt loan is accessed.
    unsafe {
        if backend::features() & 16 != 0 {
            KlattResetSP();
        }
    }
}
#[no_mangle]
unsafe extern "C" fn KlattInit() {
    let Some(_lease) = Lease::enter() else {
        return;
    };
    // SAFETY: admitted serialized kernel; player callbacks touch separate state.
    unsafe {
        if backend::features() & 16 != 0 {
            KlattInitSP();
        }
        kernel::espeak_rs_klatt_init(state());
    }
}
#[no_mangle]
unsafe extern "C" fn KlattReset(control: i32) {
    let Some(_lease) = Lease::enter() else {
        return;
    };
    // SAFETY: reset callback affects only player, then exclusive kernel reset.
    unsafe {
        reset_player();
        kernel::espeak_rs_klatt_reset(state(), control);
    }
}
#[no_mangle]
unsafe extern "C" fn KlattFini() {
    let Some(_lease) = Lease::enter() else {
        return;
    };
    // SAFETY: serialized player shutdown; preserve historical Klatt histories.
    unsafe {
        if backend::features() & 16 != 0 {
            KlattFiniSP();
        }
    }
}
#[no_mangle]
unsafe extern "C" fn Wavegen_Klatt(
    length: i32,
    resume: i32,
    first: *const Frame,
    last: *const Frame,
    data: *mut WgenData,
    voice: *const Voice,
) -> i32 {
    if voice.is_null() || data.is_null() {
        return 0;
    }
    let Some(_lease) = Lease::enter() else {
        return 0;
    };
    // SAFETY: caller retains used voice/data/frame extents; copy routing scalar
    // before player/kernel callbacks. Shared storage is disjoint from the slot.
    unsafe {
        if backend::features() & 16 != 0 && ptr::addr_of!((*voice).klatt[0]).read() == 6 {
            return Wavegen_KlattSP(data, voice, length, resume, first, last);
        }
        let shared = kernel::Shared {
            memory: ptr::addr_of_mut!(espeak_rs_wave_memory),
            output: ptr::addr_of_mut!(espeak_rs_output),
            random: Some(random),
            reset: Some(reset_player),
        };
        kernel::espeak_rs_klatt_fill(state(), &shared, length, resume, first, last, data, voice)
    }
}
