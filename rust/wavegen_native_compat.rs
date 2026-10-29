//! Complete native waveform host: descriptor construction, entry points and
//! backend routing over retained Rust queue/output/kernel owners.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case, non_upper_case_globals)]
use super::{wavegen_compat as kernel, wavegen_host_compat as host};
use crate::{
    formant::Frame,
    output::Output,
    synthesis_parameters::{Embedded, Pitch},
    voice::Voice,
    wave_memory::WaveMemory,
    wavegen::{Peak, Wavegen, WgenData, N_PEAKS},
};
use std::{
    ffi::{c_char, c_long, c_void},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy)]
#[repr(C)]
struct Config {
    features: u32,
    mbrola_name: *const c_char,
    mbrola_fill: Option<unsafe extern "C" fn(i32, bool, i32) -> i32>,
    mbrola_reset: Option<unsafe extern "C" fn()>,
    create: Option<unsafe extern "C" fn(i32, i32) -> *mut c_void>,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    flush: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    read: Option<unsafe extern "C" fn(*mut c_void, *mut i16, i32) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, *const i16, i32) -> i32>,
    get_speed: Option<unsafe extern "C" fn(*mut c_void) -> f32>,
    set_speed: Option<unsafe extern "C" fn(*mut c_void, f32)>,
    rescale: Option<unsafe extern "C" fn(i32, i32)>,
}
const _: () = assert!(std::mem::size_of::<Config>() == crate::by_pointer_width(96, 48));
extern "C" {
    static espeak_rs_wavegen_config: Config;
    static mut espeak_rs_wave_memory: WaveMemory;
    static mut espeak_rs_output: Output;
    static mut samplerate: i32;
    static mut embedded_value: [i32; 15];
    static mut output_hooks: *mut host::Hooks;
    static mut espeak_rs_sonic_state: host::Sonic;
    #[link_name = "voice"]
    static mut engine_voice: *const Voice;
    fn espeak_rs_native_output_hooks(hooks: *mut host::Hooks) -> i32;
    fn espeak_rs_rand(minimum: c_long, maximum: c_long) -> c_long;
    fn espeak_rs_wcmdq_free(memory: *mut WaveMemory) -> i32;
    fn espeak_rs_wcmdq_used(memory: *mut WaveMemory) -> i32;
    fn espeak_rs_wcmdq_inc(memory: *mut WaveMemory);
    fn MarkerEvent(kind: i32, position: u32, value: i32, value2: i32, pointer: *mut u8);
    fn KlattInit();
    fn KlattReset(control: i32);
    fn KlattFini();
    fn Wavegen_Klatt(
        length: i32,
        resume: i32,
        first: *const Frame,
        last: *const Frame,
        data: *mut WgenData,
        voice: *const Voice,
    ) -> i32;
}
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
pub(super) unsafe fn features() -> u32 {
    // SAFETY: immutable configuration provided by the compiled host.
    unsafe { ptr::addr_of!(espeak_rs_wavegen_config.features).read() }
}
fn shared() -> kernel::Shared {
    kernel::Shared {
        memory: ptr::addr_of_mut!(espeak_rs_wave_memory),
        output: ptr::addr_of_mut!(espeak_rs_output),
        samplerate: ptr::addr_of_mut!(samplerate),
        embedded: ptr::addr_of_mut!(embedded_value).cast(),
    }
}
#[allow(clippy::useless_conversion)] // C long is i32 on Windows, i64 on this host.
unsafe extern "C" fn random(minimum: i32, maximum: i32) -> i32 {
    // SAFETY: serialized owned native random stream, same C-long conversion.
    unsafe { espeak_rs_rand(minimum.into(), maximum.into()) as i32 }
}
unsafe extern "C" fn mbrola(length: i32, resume: i32, amplitude: i32) -> i32 {
    // SAFETY: immutable optional binding, preserving int-to-C-bool conversion.
    unsafe {
        espeak_rs_wavegen_config
            .mbrola_fill
            .map_or(0, |fill| fill(length, resume != 0, amplitude))
    }
}
unsafe extern "C" fn sonic_write(stream: *mut c_void, samples: *mut i16, count: i32) -> i32 {
    // SAFETY: private admitted Sonic loan; the foreign library retains only its
    // own stream state and reads the input through its declared const pointer.
    unsafe {
        espeak_rs_wavegen_config
            .write
            .map_or(0, |write| write(stream, samples.cast_const(), count))
    }
}
unsafe extern "C" fn klatt(effect: *const host::Effect) -> i32 {
    // SAFETY: private initialized effect; raw copies before backend callbacks.
    unsafe {
        let e = effect.read();
        Wavegen_Klatt(
            e.a,
            e.b,
            e.value as *const Frame,
            e.value2 as *const Frame,
            e.data.cast(),
            e.voice.cast(),
        )
    }
}
unsafe fn descriptor() -> host::Descriptor {
    // SAFETY: immutable compiled feature/function bindings; retain only raw
    // owner addresses and copied callbacks, never references across effects.
    unsafe {
        let c = ptr::addr_of!(espeak_rs_wavegen_config).read();
        host::Descriptor {
            context: ptr::null_mut(),
            memory: ptr::addr_of_mut!(espeak_rs_wave_memory),
            output: ptr::addr_of_mut!(espeak_rs_output),
            hooks: ptr::addr_of_mut!(output_hooks),
            sonic: ptr::addr_of_mut!(espeak_rs_sonic_state),
            mbrola_name: c.mbrola_name,
            features: c.features & 15,
            marker: Some(MarkerEvent),
            release: Some(libc::free),
            random: Some(random),
            klatt_reset: if c.features & 1 != 0 {
                Some(KlattReset)
            } else {
                None
            },
            klatt: if c.features & 1 != 0 {
                Some(klatt)
            } else {
                None
            },
            mbrola: if c.mbrola_fill.is_some() {
                Some(mbrola)
            } else {
                None
            },
            mbrola_reset: c.mbrola_reset,
            create: c.create,
            destroy: c.destroy,
            flush: c.flush,
            read: c.read,
            write: if c.write.is_some() {
                Some(sonic_write)
            } else {
                None
            },
            get_speed: c.get_speed,
            set_speed: c.set_speed,
            fill: Some(fill),
            rescale: c.rescale,
        }
    }
}
unsafe extern "C" fn effect(_: *mut c_void, e: *mut kernel::Effect) -> i32 {
    // SAFETY: private matching repr(C) effect (56 bytes, checked in both ABI
    // units); underlying callbacks cannot touch the borrowed waveform kernel.
    unsafe {
        let d = descriptor();
        let mut result = 0;
        if host::espeak_rs_wavegen_host_effect(&d, e.cast(), &mut result) != 0 {
            0
        } else {
            result
        }
    }
}
unsafe extern "C" fn fill(_: *mut c_void) -> i32 {
    // SAFETY: called only by admitted WavegenFill; waveform state is disjoint
    // from host fill/stop state. Hook tables may change during a hook callback;
    // only presence bits are copied here and each effect reads the current hook.
    unsafe {
        let bits = features();
        let hooks = output_hooks;
        let options = kernel::FfiOptions {
            klatt: i32::from(bits & 1 != 0),
            mbrola: i32::from(bits & 2 != 0),
            sonic: i32::from(bits & 4 != 0),
            roughness: if engine_voice.is_null() {
                0
            } else {
                ptr::addr_of!((*engine_voice).roughness).read()
            },
            hooks: if hooks.is_null() {
                0
            } else {
                i32::from(ptr::addr_of!((*hooks).voiced).read().is_some())
                    | (i32::from(ptr::addr_of!((*hooks).silence).read().is_some()) << 1)
                    | (i32::from(ptr::addr_of!((*hooks).unvoiced).read().is_some()) << 2)
            },
        };
        kernel::espeak_rs_wavegen_fill(
            host::espeak_rs_native_wavegen_state(),
            &shared(),
            ptr::null_mut(),
            Some(effect),
            &options,
            super::env_fall.as_ptr(),
        )
    }
}
/// Admit one public kernel operation; private effects touch separate storage.
unsafe fn run<R>(invalid: R, body: impl FnOnce(*mut Wavegen, &kernel::Shared) -> R) -> R {
    let Some(_lease) = Lease::enter() else {
        return invalid;
    };
    // SAFETY: admitted serialized instance; pointer views remain caller-owned.
    body(host::espeak_rs_native_wavegen_state(), &shared())
}
#[no_mangle]
unsafe extern "C" fn RustWavegenState() -> *mut Wavegen {
    // SAFETY: stable historical process-wide allocation; no Rust loan escapes.
    host::espeak_rs_native_wavegen_state()
}
#[no_mangle]
unsafe extern "C" fn WcmdqFree() -> i32 {
    // SAFETY: serialized queue scalar read.
    unsafe { espeak_rs_wcmdq_free(ptr::addr_of_mut!(espeak_rs_wave_memory)) }
}
#[no_mangle]
unsafe extern "C" fn WcmdqUsed() -> i32 {
    // SAFETY: serialized queue scalar read.
    unsafe { espeak_rs_wcmdq_used(ptr::addr_of_mut!(espeak_rs_wave_memory)) }
}
#[no_mangle]
unsafe extern "C" fn WcmdqInc() {
    // SAFETY: serialized queue mutation.
    unsafe { espeak_rs_wcmdq_inc(ptr::addr_of_mut!(espeak_rs_wave_memory)) }
}
#[no_mangle]
unsafe extern "C" fn WcmdqStop() {
    let Some(_lease) = Lease::enter() else {
        return;
    };
    // SAFETY: no waveform loan survives stop callbacks; admitted host stops
    // queue/Sonic/MBROLA in the historical order without active fill reentry.
    unsafe {
        host::espeak_rs_wavegen_host_stop(&descriptor());
    }
}
#[no_mangle]
unsafe extern "C" fn WavegenInit(rate: i32, factor: i32) {
    let Some(_lease) = Lease::enter() else {
        return;
    };
    // SAFETY: serialized init; private effects touch disjoint host/backend state.
    unsafe {
        kernel::espeak_rs_wavegen_init(
            host::espeak_rs_native_wavegen_state(),
            &shared(),
            ptr::null_mut(),
            Some(effect),
            rate,
            factor,
        );
        if features() & 1 != 0 {
            KlattInit();
        }
    }
}
#[no_mangle]
unsafe extern "C" fn GetAmplitude() -> i32 {
    // SAFETY: serialized kernel/embedded snapshot operation.
    unsafe { run(0, |w, s| kernel::espeak_rs_wavegen_amplitude(w, s)) }
}
#[no_mangle]
unsafe extern "C" fn PeaksToHarmspect(
    peaks: *const [Peak; N_PEAKS],
    pitch: i32,
    output: *mut i32,
    control: i32,
) -> i32 {
    // SAFETY: caller retains admitted peak and MAX_HARMONIC output arrays.
    unsafe {
        run(1, |w, _| {
            kernel::espeak_rs_wavegen_harmonics(w, samplerate, peaks, pitch, output, control)
        })
    }
}
#[no_mangle]
unsafe extern "C" fn InitBreath() {
    // SAFETY: serialized retained kernel.
    unsafe {
        run((), |w, _| {
            kernel::espeak_rs_wavegen_init_breath(w, samplerate)
        });
    }
}
#[no_mangle]
unsafe extern "C" fn SetEmbedded(control: i32, value: i32) {
    // SAFETY: serialized embedded values; no state loans reach effects.
    unsafe {
        run((), |w, s| {
            kernel::espeak_rs_wavegen_set_embedded(
                w,
                s,
                ptr::null_mut(),
                Some(effect),
                control,
                value,
            )
        });
    }
}
#[no_mangle]
unsafe extern "C" fn WavegenSetVoice(voice: *const Voice) {
    // SAFETY: live used voice representation copied by the kernel adapter.
    unsafe {
        run((), |w, s| {
            kernel::espeak_rs_wavegen_set_voice(w, s, ptr::null_mut(), Some(effect), voice)
        });
    }
}
#[no_mangle]
unsafe extern "C" fn SetPitch2(
    voice: *const Voice,
    first: i32,
    second: i32,
    base: *mut i32,
    range: *mut i32,
) {
    if base.is_null() || range.is_null() {
        return;
    }
    // SAFETY: serialized used embedded scalars, valid voice and writable outputs;
    // planning holds no foreign references across a callback and writes in order.
    unsafe {
        let values = Embedded {
            pitch: embedded_value[1],
            tone: embedded_value[6],
            range: embedded_value[4],
        };
        let mut pitch = Pitch::default();
        if super::espeak_rs_pitch(voice, first, second, &values, &mut pitch) == 0 {
            base.write(pitch.base);
            range.write(pitch.range);
        }
    }
}
#[no_mangle]
unsafe extern "C" fn WavegenFill() -> i32 {
    let Some(_lease) = Lease::enter() else {
        return 1;
    };
    // SAFETY: serialized complete host fill; its private fill callback uses the
    // admitted kernel, without reacquiring public admission or polling a lock.
    unsafe {
        let mut result = 0;
        if host::espeak_rs_wavegen_host_fill(&descriptor(), &mut result) != 0 {
            1
        } else {
            result
        }
    }
}
#[no_mangle]
unsafe extern "C" fn WavegenFini() {
    let Some(_lease) = Lease::enter() else {
        return;
    };
    // SAFETY: serialized backend shutdown; match old source-history lifetime.
    unsafe {
        if features() & 1 != 0 {
            KlattFini();
        }
    }
}
#[no_mangle]
unsafe extern "C" fn Write4Bytes(file: *mut libc::FILE, value: i32) {
    if file.is_null() {
        return;
    }
    // SAFETY: caller owns the live C stream; retain its text-mode/error behavior.
    unsafe {
        for byte in value.to_le_bytes() {
            libc::fputc(i32::from(byte), file);
        }
    }
}
#[no_mangle]
unsafe extern "C" fn espeak_ng_SetOutputHooks(hooks: *mut host::Hooks) -> i32 {
    // SAFETY: caller retains its table; replacement during a callback is allowed
    // because host effects capture selected function pointers by value.
    unsafe { espeak_rs_native_output_hooks(hooks) }
}
#[no_mangle]
unsafe extern "C" fn espeak_ng_SetConstF0(value: i32) -> i32 {
    // SAFETY: admitted serialized kernel scalar update.
    unsafe {
        run(libc::EAGAIN, |w, _| {
            kernel::espeak_rs_wavegen_set_const_f0(w, value);
            0
        })
    }
}
