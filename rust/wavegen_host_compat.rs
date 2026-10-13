//! Fresh raw native wave-generator views and serialized resource primitives.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{output::Output, wave_memory::WaveMemory, wavegen::Wavegen, wavegen_host as host};
use std::{
    ffi::{c_char, c_void},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};

#[repr(C)]
pub struct Hooks {
    pub(super) alignment: Option<unsafe extern "C" fn(*mut c_char, i32)>,
    pub(super) silence: Option<unsafe extern "C" fn(i16)>,
    pub(super) voiced: Option<unsafe extern "C" fn(i16)>,
    pub(super) unvoiced: Option<unsafe extern "C" fn(i16)>,
}
#[repr(C)]
pub struct Sonic {
    pub(super) speed: f64,
    pub(super) stream: *mut c_void,
}
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut samplerate: i32 = 0;
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut output_hooks: *mut Hooks = ptr::null_mut();
#[no_mangle]
#[allow(non_upper_case_globals)]
static mut espeak_rs_sonic_state: Sonic = Sonic {
    speed: 1.0,
    stream: ptr::null_mut(),
};
// Preserve the historical process-wide generator lifetime. Compiler/RMS callers
// also borrow it; retiring these raw views requires later engine-instance work.
static GENERATOR: OnceLock<usize> = OnceLock::new();
#[no_mangle]
pub(super) extern "C" fn espeak_rs_native_wavegen_state() -> *mut Wavegen {
    *GENERATOR.get_or_init(|| Box::into_raw(Box::<Wavegen>::default()) as usize) as *mut Wavegen
}
#[no_mangle]
extern "C" fn espeak_rs_native_output_hooks(hooks: *mut Hooks) -> i32 {
    // SAFETY: serialized registration, also permitted from a hook callback;
    // retained table is caller-owned and no reference to it spans callbacks.
    unsafe { ptr::addr_of_mut!(output_hooks).write(hooks) };
    0
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Effect {
    pub(super) op: i32,
    pub(super) index: i32,
    pub(super) a: i32,
    pub(super) b: i32,
    pub(super) c: i32,
    pub(super) value: usize,
    pub(super) value2: usize,
    pub(super) data: *mut c_void,
    pub(super) voice: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub(super) context: *mut c_void,
    pub(super) memory: *mut WaveMemory,
    pub(super) output: *mut Output,
    pub(super) hooks: *mut *mut Hooks,
    pub(super) sonic: *mut Sonic,
    pub(super) mbrola_name: *const c_char,
    pub(super) features: u32,
    pub(super) marker: Option<unsafe extern "C" fn(i32, u32, i32, i32, *mut u8)>,
    pub(super) release: Option<unsafe extern "C" fn(*mut c_void)>,
    pub(super) random: Option<unsafe extern "C" fn(i32, i32) -> i32>,
    pub(super) klatt_reset: Option<unsafe extern "C" fn(i32)>,
    pub(super) klatt: Option<unsafe extern "C" fn(*const Effect) -> i32>,
    pub(super) mbrola: Option<unsafe extern "C" fn(i32, i32, i32) -> i32>,
    pub(super) mbrola_reset: Option<unsafe extern "C" fn()>,
    pub(super) create: Option<unsafe extern "C" fn(i32, i32) -> *mut c_void>,
    pub(super) destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub(super) flush: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) read: Option<unsafe extern "C" fn(*mut c_void, *mut i16, i32) -> i32>,
    pub(super) write: Option<unsafe extern "C" fn(*mut c_void, *mut i16, i32) -> i32>,
    pub(super) get_speed: Option<unsafe extern "C" fn(*mut c_void) -> f32>,
    pub(super) set_speed: Option<unsafe extern "C" fn(*mut c_void, f32)>,
    pub(super) fill: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) rescale: Option<unsafe extern "C" fn(i32, i32)>,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(
    std::mem::size_of::<Descriptor>() == 184
        && std::mem::size_of::<Effect>() == 56
        && std::mem::size_of::<Sonic>() == 16
);
static EFFECT: AtomicBool = AtomicBool::new(false);
static FILL: AtomicBool = AtomicBool::new(false);
struct Guard(&'static AtomicBool);
impl Guard {
    fn enter(slot: &'static AtomicBool) -> Option<Self> {
        slot.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self(slot))
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release)
    }
}
struct Native {
    d: Descriptor,
}
impl Native {
    fn features(&self) -> host::Features {
        host::Features {
            klatt: self.d.features & 1 != 0,
            mbrola: self.d.features & 2 != 0,
            sonic: self.d.features & 4 != 0,
            big_endian: self.d.features & 8 != 0,
        }
    }
}
impl host::Host for Native {
    fn queue(&mut self, index: usize) -> [isize; 4] {
        // SAFETY: live initialized queue; bounded index from safe controller,
        // copied before a primitive can mutate any native storage.
        unsafe {
            ptr::addr_of!((*self.d.memory).queue)
                .cast::<[isize; 4]>()
                .add(index)
                .read()
        }
    }
    fn output(&mut self) -> (usize, usize) {
        // SAFETY: initialized live cursor fields, immediate raw copies.
        unsafe {
            (
                ptr::addr_of!((*self.d.output).ptr).read() as usize,
                ptr::addr_of!((*self.d.output).end).read() as usize,
            )
        }
    }
    fn publish_output(&mut self, pointer: usize) {
        // SAFETY: bounded admitted cursor publication without a retained loan.
        unsafe { ptr::addr_of_mut!((*self.d.output).ptr).write(pointer as *mut u8) };
    }
    fn hook(&mut self, kind: i32, sample: i16) -> Result<(), host::Invalid> {
        // SAFETY: live slot, with null denoting unregistered hooks; callback
        // pointer copied before the callback can replace/mutate its table.
        let callback = unsafe {
            let hooks = self.d.hooks.read();
            if hooks.is_null() {
                return Err(host::Invalid);
            }
            match kind {
                1 => ptr::addr_of!((*hooks).voiced).read(),
                2 => ptr::addr_of!((*hooks).silence).read(),
                _ => ptr::addr_of!((*hooks).unvoiced).read(),
            }
        }
        .ok_or(host::Invalid)?;
        // SAFETY: admitted synchronous selected hook and narrowed C short.
        unsafe { callback(sample) };
        Ok(())
    }
    fn alignment(&mut self, pointer: usize, kind: i32) -> Result<(), host::Invalid> {
        // SAFETY: initialized retained hook slot/table; no table reference.
        let callback = unsafe {
            let hooks = self.d.hooks.read();
            if hooks.is_null() {
                return Err(host::Invalid);
            }
            ptr::addr_of!((*hooks).alignment).read()
        }
        .ok_or(host::Invalid)?;
        // SAFETY: queue transfers a live terminated C allocation; synchronous
        // callback borrows it, release follows after the callback returns.
        unsafe { callback(pointer as *mut c_char, kind) };
        Ok(())
    }
    fn marker(&mut self, kind: i32, position: u32, value: i32, value2: i32, pointer: usize) {
        // SAFETY: admitted primitive and copied scalar/cursor values.
        unsafe { self.d.marker.unwrap()(kind, position, value, value2, pointer as *mut u8) };
    }
    fn release(&mut self, pointer: usize) {
        // SAFETY: admitted C destructor consumes one transferred allocation;
        // compiler/engine caller has drained every other view before release.
        unsafe { self.d.release.unwrap()(pointer as *mut c_void) };
    }
    fn random(&mut self, min: i32, max: i32) -> i32 {
        // SAFETY: admitted historical random primitive, copied arguments.
        unsafe { self.d.random.unwrap()(min, max) }
    }
    fn klatt_reset(&mut self) {
        // SAFETY: selected compiled-in backend primitive, historical control1.
        unsafe { self.d.klatt_reset.unwrap()(1) };
    }
    fn klatt(&mut self, e: host::Effect) -> i32 {
        let e = Effect {
            op: e.op,
            index: e.index,
            a: e.a,
            b: e.b,
            c: e.c,
            value: e.value,
            value2: e.value2,
            data: e.data as *mut c_void,
            voice: e.voice as *mut c_void,
        };
        // SAFETY: owned initialized effect; backend borrows generator-supplied
        // live data/voice/frame pointers synchronously without retaining them.
        unsafe { self.d.klatt.unwrap()(&e) }
    }
    fn mbrola(&mut self, length: i32, resume: i32, amplitude: i32) -> i32 {
        // SAFETY: selected compiled-in backend primitive, copied arguments.
        unsafe { self.d.mbrola.unwrap()(length, resume, amplitude) }
    }
    fn fill(&mut self) -> i32 {
        // SAFETY: serialized core generator; it may call the guarded effect
        // entry, but cannot reenter fill/stop or retain descriptor views.
        unsafe { self.d.fill.unwrap()(self.d.context) }
    }
    fn rescale(&mut self, before: i32, after: i32) {
        // SAFETY: admitted native event rescaler, copied bounded sample counts.
        unsafe { self.d.rescale.unwrap()(before, after) };
    }
    fn stop_queue(&mut self) {
        // SAFETY: live unique queue fields; references do not span effects.
        unsafe {
            ptr::addr_of_mut!((*self.d.memory).head).write(0);
            ptr::addr_of_mut!((*self.d.memory).tail).write(0)
        };
    }
    fn mbrola_active(&mut self) -> bool {
        // SAFETY: selected initialized stable backend name, first byte only.
        unsafe { self.d.mbrola_name.read() != 0 }
    }
    fn mbrola_reset(&mut self) {
        // SAFETY: selected compiled-in backend reset, no native loan retained.
        unsafe { self.d.mbrola_reset.unwrap()() };
    }
    fn speed(&mut self) -> f64 {
        // SAFETY: initialized live scalar copied immediately.
        unsafe { ptr::addr_of!((*self.d.sonic).speed).read() }
    }
    fn set_speed(&mut self, speed: f64) {
        // SAFETY: serialized live writable scalar; no callback during write.
        unsafe { ptr::addr_of_mut!((*self.d.sonic).speed).write(speed) };
    }
    fn stream(&mut self) -> usize {
        // SAFETY: initialized live nullable opaque resource slot.
        unsafe { ptr::addr_of!((*self.d.sonic).stream).read() as usize }
    }
    fn set_stream(&mut self, stream: usize) {
        // SAFETY: serialized initialized resource slot; historical ordering.
        unsafe { ptr::addr_of_mut!((*self.d.sonic).stream).write(stream as *mut c_void) };
    }
    fn sonic_create(&mut self, rate: i32, channels: i32) -> usize {
        // SAFETY: admitted resource constructor, outside any storage loan.
        unsafe { self.d.create.unwrap()(rate, channels) as usize }
    }
    fn sonic_destroy(&mut self, stream: usize) {
        // SAFETY: one admitted live opaque owner transferred to its destructor.
        unsafe { self.d.destroy.unwrap()(stream as *mut c_void) };
    }
    fn sonic_flush(&mut self, stream: usize) {
        // SAFETY: admitted live stream, synchronous resource operation.
        unsafe { self.d.flush.unwrap()(stream as *mut c_void) };
    }
    fn sonic_speed(&mut self, stream: usize) -> f32 {
        // SAFETY: admitted live stream, synchronous copied scalar result.
        unsafe { self.d.get_speed.unwrap()(stream as *mut c_void) }
    }
    fn sonic_set_speed(&mut self, stream: usize, speed: f32) {
        // SAFETY: admitted live stream and copied finite native speed command.
        unsafe { self.d.set_speed.unwrap()(stream as *mut c_void, speed) };
    }
    fn sonic_write(&mut self, stream: usize, pointer: usize, samples: i32) {
        // SAFETY: live aligned initialized PCM extent admitted by caller and
        // checked controller; borrowed only during the resource primitive.
        unsafe { self.d.write.unwrap()(stream as *mut c_void, pointer as *mut i16, samples) };
    }
    fn sonic_read(&mut self, stream: usize, pointer: usize, samples: i32) -> i32 {
        // SAFETY: live aligned writable capacity, exclusive synchronous loan;
        // primitive may report only its written prefix, never retain storage.
        unsafe { self.d.read.unwrap()(stream as *mut c_void, pointer as *mut i16, samples) }
    }
    fn transcode(&mut self, pointer: usize, samples: i32, to_native: bool) {
        for i in 0..samples as usize {
            let p = (pointer as *mut u8).wrapping_add(i * 2);
            // SAFETY: admitted initialized complete PCM extent; immediate
            // unaligned byte-pair conversion with no callback or retained loan.
            unsafe {
                let pair = [p.read(), p.add(1).read()];
                let pair = if to_native {
                    u16::from_le_bytes(pair).to_ne_bytes()
                } else {
                    u16::from_ne_bytes(pair).to_le_bytes()
                };
                p.write(pair[0]);
                p.add(1).write(pair[1]);
            }
        }
    }
}
unsafe fn admit(pointer: *const Descriptor) -> Option<Native> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: initialized readable descriptor, copied before effects.
    let d = unsafe { pointer.read() };
    if d.memory.is_null()
        || d.output.is_null()
        || d.hooks.is_null()
        || d.sonic.is_null()
        || d.features & !15 != 0
        || d.marker.is_none()
        || d.release.is_none()
        || d.random.is_none()
        || (d.features & 1 != 0 && (d.klatt_reset.is_none() || d.klatt.is_none()))
        || (d.features & 2 != 0
            && (d.mbrola.is_none() || d.mbrola_reset.is_none() || d.mbrola_name.is_null()))
        || (d.features & 4 != 0
            && (d.create.is_none()
                || d.destroy.is_none()
                || d.flush.is_none()
                || d.read.is_none()
                || d.write.is_none()
                || d.get_speed.is_none()
                || d.set_speed.is_none()))
    {
        return None;
    }
    Some(Native { d })
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_wavegen_host_effect(
    d: *const Descriptor,
    e: *const Effect,
    result: *mut i32,
) -> i32 {
    if e.is_null() || result.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: forwarded live descriptor/effect caller contract.
    let Some(mut native) = (unsafe { admit(d) }) else {
        return libc::EINVAL;
    };
    let Some(_guard) = Guard::enter(&EFFECT) else {
        return libc::EAGAIN;
    };
    // SAFETY: initialized effect copied before native effects/output publication.
    let e = unsafe { e.read() };
    let features = native.features();
    let e = host::Effect {
        op: e.op,
        index: e.index,
        a: e.a,
        b: e.b,
        c: e.c,
        value: e.value,
        value2: e.value2,
        data: e.data as usize,
        voice: e.voice as usize,
    };
    match host::effect(&mut native, features, e) {
        Ok(value) => {
            // SAFETY: aligned writable result, disjoint from live engine state.
            unsafe { result.write(value) };
            0
        }
        Err(_) => libc::EINVAL,
    }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_wavegen_host_fill(
    d: *const Descriptor,
    result: *mut i32,
) -> i32 {
    if result.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: forwarded live descriptor contract.
    let Some(mut native) = (unsafe { admit(d) }) else {
        return libc::EINVAL;
    };
    if native.d.fill.is_none() || (native.d.features & 4 != 0 && native.d.rescale.is_none()) {
        return libc::EINVAL;
    }
    let Some(_guard) = Guard::enter(&FILL) else {
        return libc::EAGAIN;
    };
    if EFFECT.load(Ordering::Acquire) {
        return libc::EAGAIN;
    }
    let features = native.features();
    match host::fill(&mut native, features) {
        Ok(value) => {
            // SAFETY: aligned writable result, disjoint from live engine state.
            unsafe { result.write(value) };
            0
        }
        Err(_) => libc::EINVAL,
    }
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_wavegen_host_stop(d: *const Descriptor) -> i32 {
    // SAFETY: forwarded live descriptor contract.
    let Some(mut native) = (unsafe { admit(d) }) else {
        return libc::EINVAL;
    };
    let Some(_guard) = Guard::enter(&FILL) else {
        return libc::EAGAIN;
    };
    if EFFECT.load(Ordering::Acquire) {
        return libc::EAGAIN;
    }
    let features = native.features();
    host::stop(&mut native, features);
    0
}
