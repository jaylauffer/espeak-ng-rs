//! Private speechPlayer engine lifecycle and admitted resident output views.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case)]
use crate::{
    formant::Frame,
    output::Output,
    speech_player::Error,
    splayer::{Engine, Mix, Spectrum, Voice as AcousticVoice},
    voice::Voice,
    wave_memory::{WaveMemory, N_WCMDQ},
    wavegen::WgenData,
};
use std::{ptr, slice, sync::Mutex};
extern "C" {
    static mut espeak_rs_wave_memory: WaveMemory;
    static mut espeak_rs_output: Output;
    fn rand() -> libc::c_int;
}
struct Slot {
    owner: Option<Box<Engine>>,
    leased: bool,
}
static SLOT: Mutex<Slot> = Mutex::new(Slot {
    owner: None,
    leased: false,
});
struct Lease(Option<Box<Engine>>);
impl Lease {
    fn enter(create: bool) -> Result<Self, i32> {
        let mut slot = SLOT.try_lock().map_err(|_| libc::EAGAIN)?;
        if slot.leased {
            return Err(libc::EAGAIN);
        }
        if slot.owner.is_none() && !create {
            return Err(libc::EINVAL);
        }
        slot.leased = true;
        let owner = slot.owner.take();
        drop(slot);
        Ok(Self(Some(owner.unwrap_or_else(|| Box::new(Engine::new())))))
    }
    fn engine(&mut self) -> &mut Engine {
        self.0.as_deref_mut().expect("live private owner")
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut slot = SLOT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slot.owner = self.0.take();
        slot.leased = false;
    }
}
#[no_mangle]
extern "C" fn espeak_rs_splayer_initialize() -> i32 {
    match Lease::enter(true) {
        Ok(mut lease) => {
            lease.engine().initialize();
            0
        }
        Err(s) => s,
    }
}
#[no_mangle]
extern "C" fn espeak_rs_splayer_finish() -> i32 {
    match Lease::enter(false) {
        Ok(mut lease) => {
            lease.engine().finish();
            0
        }
        Err(libc::EINVAL) => 0,
        Err(s) => s,
    }
}
#[no_mangle]
extern "C" fn KlattInitSP() {
    let _ = espeak_rs_splayer_initialize();
}
#[no_mangle]
extern "C" fn KlattResetSP() {
    let _ = espeak_rs_splayer_initialize();
}
#[no_mangle]
extern "C" fn KlattFiniSP() {
    let _ = espeak_rs_splayer_finish();
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Shared {
    memory: *const WaveMemory,
    output: *mut Output,
    pitch: *const u8,
    pitch_size: usize,
    mix: *const u8,
    mix_size: usize,
}
fn span(pointer: usize, length: usize) -> Result<(usize, usize), i32> {
    if length > isize::MAX as usize || (length > 0 && pointer == 0) {
        return Err(libc::EINVAL);
    }
    Ok((pointer, pointer.checked_add(length).ok_or(libc::EINVAL)?))
}
unsafe fn read_frame(pointer: *const Frame) -> Result<Spectrum, i32> {
    if pointer.is_null() {
        return Err(libc::EINVAL);
    }
    // SAFETY: exactly the initialized ordinary fields used by the original C
    // controller. No flags, unused fields or optional extension are read.
    Ok(unsafe {
        Spectrum {
            frequencies: std::array::from_fn(|i| {
                ptr::addr_of!((*pointer).frequencies[i + 1]).read_unaligned()
            }),
            bandwidths: ptr::addr_of!((*pointer).bandwidths).read_unaligned(),
            nasal: ptr::addr_of!((*pointer).klatt[1]).read_unaligned(),
        }
    })
}
unsafe fn read_voice(pointer: *const Voice) -> AcousticVoice {
    // SAFETY: exactly the initialized scalar/array fields used by C; snapshots
    // retain no name, tone, unused acoustic fields or foreign loans.
    unsafe {
        AcousticVoice {
            voicing: ptr::addr_of!((*pointer).voicing).read(),
            breath: ptr::addr_of!((*pointer).breath[1]).read(),
            frequency: std::array::from_fn(|i| ptr::addr_of!((*pointer).frequency[i + 1]).read()),
            frequency_add: std::array::from_fn(|i| {
                ptr::addr_of!((*pointer).frequency_add[i + 1]).read()
            }),
            width: std::array::from_fn(|i| ptr::addr_of!((*pointer).width[i + 1]).read()),
        }
    }
}
/// Inputs aligned/live, output and mutable owner metadata disjoint from all
/// source views/result. Frame/pitch/voice/queue snapshots precede effects. Mixed
/// bytes stay immutable until return. Result unchanged on refusal; a late bad
/// mix read retains completed data/DSP/PCM/cursor effects and reports EINVAL.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn espeak_rs_splayer_fill(
    shared: *const Shared,
    data: *mut WgenData,
    voice: *const Voice,
    length: i32,
    resume: i32,
    first: *const Frame,
    last: *const Frame,
    result: *mut i32,
) -> i32 {
    if shared.is_null() || data.is_null() || result.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: initialized live descriptor, copied before effects.
    let shared = unsafe { shared.read() };
    if shared.memory.is_null() || shared.output.is_null() {
        return libc::EINVAL;
    }
    // SAFETY: owner retains initialized WGEN_DATA and output cursor metadata.
    let mut data_snapshot = unsafe { data.read() };
    // SAFETY: only live cursor scalar fields are read, never copy/drop Output.
    let (start, cursor, end) = unsafe {
        (
            (*shared.output).start,
            (*shared.output).ptr,
            (*shared.output).end,
        )
    };
    if (start as usize) > (cursor as usize) || (cursor as usize) > (end as usize) {
        return libc::EINVAL;
    }
    let capacity = end as usize - cursor as usize;
    let out_span = match span(cursor as usize, capacity) {
        Ok(s) => s,
        Err(s) => return s,
    };
    let mix_span = match span(shared.mix as usize, shared.mix_size) {
        Ok(s) => s,
        Err(s) => return s,
    };
    if out_span.0 < mix_span.1 && mix_span.0 < out_span.1 {
        return libc::EINVAL;
    }
    // SAFETY: initialized queue metadata and command words, copied before DSP.
    let (head, tail) = unsafe { ((*shared.memory).head, (*shared.memory).tail) };
    if !(0..N_WCMDQ).contains(&head) || !(0..N_WCMDQ).contains(&tail) {
        return libc::EINVAL;
    }
    let mut commands = [0isize; N_WCMDQ as usize];
    let mut count = 0;
    let mut index = (head + 1) % N_WCMDQ;
    for _ in 0..N_WCMDQ {
        if index == tail {
            break;
        }
        // SAFETY: validated ring index and initialized live command word.
        commands[count] = unsafe { (*shared.memory).queue[index as usize][0] };
        count += 1;
        index = (index + 1) % N_WCMDQ;
    }
    let mut envelope = [0u8; 128];
    let (voice, frames) = if resume == 0 {
        if voice.is_null()
            || shared.pitch_size < 128
            || shared.pitch as usize != data_snapshot.pitch_env
            || span(shared.pitch as usize, shared.pitch_size).is_err()
        {
            return libc::EINVAL;
        }
        // SAFETY: admitted initialized 128-byte pitch source, copied before DSP.
        unsafe { ptr::copy_nonoverlapping(shared.pitch, envelope.as_mut_ptr(), 128) };
        // SAFETY: initialized acoustic voice snapshot and flagged spectrum extents.
        let (a, b) = match unsafe { (read_frame(first), read_frame(last)) } {
            (Ok(a), Ok(b)) => (a, b),
            _ => return libc::EINVAL,
        };
        // SAFETY: admitted initialized acoustic voice; no source loan retained.
        (unsafe { read_voice(voice) }, Some((a, b)))
    } else {
        (AcousticVoice::default(), None)
    };
    let mut lease = match Lease::enter(false) {
        Ok(l) => l,
        Err(s) => return s,
    };
    let output_pointer = if capacity == 0 {
        ptr::NonNull::<u8>::dangling().as_ptr()
    } else {
        cursor
    };
    let mix_pointer = if shared.mix_size == 0 {
        ptr::NonNull::<u8>::dangling().as_ptr()
    } else {
        shared.mix
    };
    // SAFETY: live initialized exclusive output and disjoint immutable mix span.
    let (output, mix) = unsafe {
        (
            slice::from_raw_parts_mut(output_pointer, capacity),
            slice::from_raw_parts(mix_pointer, shared.mix_size),
        )
    };
    let outcome = lease.engine().fill(
        &mut data_snapshot,
        &voice,
        length,
        frames.as_ref().map(|(a, b)| (a, b)),
        &envelope,
        crate::splayer::following(&commands[..count]),
        output,
        Mix {
            base: shared.mix as usize,
            bytes: mix,
        },
        &mut || {
            // SAFETY: standard C RNG has no engine/resource callback or retained loan.
            f64::from(unsafe { rand() }) / f64::from(libc::RAND_MAX)
        },
    );
    let written = lease.engine().written();
    // SAFETY: exclusive initialized owner metadata, no output/mix alias.
    unsafe {
        ptr::addr_of_mut!((*data).pitch_ix).write(data_snapshot.pitch_ix);
        ptr::addr_of_mut!((*data).pitch).write(data_snapshot.pitch);
        ptr::addr_of_mut!((*data).mix_wavefile_ix).write(data_snapshot.mix_wavefile_ix);
        ptr::addr_of_mut!((*data).mix_wavefile_offset).write(data_snapshot.mix_wavefile_offset);
        (*shared.output).ptr = cursor.wrapping_add(written);
    }
    match outcome {
        Ok(_) => {
            // SAFETY: writable result disjoint from admitted owner buffers.
            unsafe { result.write(i32::from(written >= capacity)) };
            0
        }
        Err(Error::Invalid) => libc::EINVAL,
        Err(Error::Full) => libc::EAGAIN,
    }
}
#[no_mangle]
unsafe extern "C" fn Wavegen_KlattSP(
    data: *mut WgenData,
    voice: *const Voice,
    length: i32,
    resume: i32,
    first: *const Frame,
    last: *const Frame,
) -> i32 {
    if data.is_null() {
        return 0;
    }
    // SAFETY: legacy engine retains initialized serialized WGEN_DATA.
    let snapshot = unsafe { data.read() };
    let invoke = |mix: &[u8]| {
        let shared = Shared {
            memory: ptr::addr_of!(espeak_rs_wave_memory),
            output: ptr::addr_of_mut!(espeak_rs_output),
            pitch: snapshot.pitch_env as *const u8,
            pitch_size: 128,
            mix: mix.as_ptr(),
            mix_size: mix.len(),
        };
        let mut result = 0;
        // SAFETY: serialized engine globals and resident frame/voice/pitch; mix
        // is an immutable Rust-owned asset loan; rand cannot reenter its owner.
        let status = unsafe {
            espeak_rs_splayer_fill(
                &shared,
                data,
                voice,
                length,
                resume,
                first,
                last,
                &mut result,
            )
        };
        if status == 0 {
            Ok(result)
        } else {
            Err(status)
        }
    };
    if snapshot.mix_wavefile_ix < snapshot.n_mix_wavefile {
        super::phoneme_assets_compat::with_phondata(invoke).unwrap_or(0)
    } else {
        invoke(&[]).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_OWNER: Mutex<()> = Mutex::new(());
    #[test]
    fn private_owner_reuses_storage_and_live_calls_refuse_reset_or_finish() {
        let _test_owner = TEST_OWNER.lock().unwrap();
        assert_eq!(espeak_rs_splayer_initialize(), 0);
        let mut lease = Lease::enter(false).unwrap();
        let address = lease.engine() as *mut Engine;
        assert_eq!(espeak_rs_splayer_initialize(), libc::EAGAIN);
        assert_eq!(espeak_rs_splayer_finish(), libc::EAGAIN);
        assert_eq!(
            std::thread::spawn(|| espeak_rs_splayer_initialize())
                .join()
                .unwrap(),
            libc::EAGAIN
        );
        drop(lease);
        for _ in 0..200 {
            assert_eq!(espeak_rs_splayer_finish(), 0);
            assert_eq!(espeak_rs_splayer_initialize(), 0);
            assert_eq!(
                Lease::enter(false).unwrap().engine() as *mut Engine,
                address
            );
        }
        assert_eq!(espeak_rs_splayer_finish(), 0);
    }
}
