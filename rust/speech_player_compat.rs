//! speechPlayer C API over bounded Rust owners; no loans escape calls.
// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(non_snake_case)]
use crate::speech_player::{Error, Frame, Player};
use std::{
    ffi::c_void,
    ptr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
const OWNERS: usize = 64;
struct Slot {
    id: usize,
    owner: Option<Box<Player>>,
}
static SLOTS: Mutex<Vec<Slot>> = Mutex::new(Vec::new());
static NEXT: AtomicUsize = AtomicUsize::new(1);
// Android's libc crate omits rand, but the native C runtime supplies this
// same standard symbol on every supported target.
extern "C" {
    fn rand() -> libc::c_int;
}
struct Lease {
    id: usize,
    owner: Option<Box<Player>>,
}
impl Lease {
    fn enter(handle: *mut c_void) -> Result<Self, i32> {
        let id = handle as usize;
        if id == 0 {
            return Err(libc::EINVAL);
        }
        let mut slots = SLOTS.try_lock().map_err(|_| libc::EAGAIN)?;
        let slot = slots.iter_mut().find(|s| s.id == id).ok_or(libc::EINVAL)?;
        let owner = slot.owner.take().ok_or(libc::EAGAIN)?;
        Ok(Self {
            id,
            owner: Some(owner),
        })
    }
    fn player(&mut self) -> &mut Player {
        self.owner.as_deref_mut().expect("lease owns player")
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if self.owner.is_none() {
            return;
        }
        // Only short registration/restoration operations hold the registry;
        // synthesis runs after unlocking. A live lease cannot be terminated.
        let mut slots = SLOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = slots
            .iter_mut()
            .find(|s| s.id == self.id)
            .expect("live lease slot");
        slot.owner = self.owner.take();
    }
}
fn status(error: Error) -> i32 {
    match error {
        Error::Invalid => libc::EINVAL,
        Error::Full => libc::EAGAIN,
    }
}
#[no_mangle]
extern "C" fn speechPlayer_initialize(rate: i32) -> *mut c_void {
    if rate <= 0 {
        return ptr::null_mut();
    }
    let Ok(mut slots) = SLOTS.try_lock() else {
        return ptr::null_mut();
    };
    if slots.is_empty() {
        slots.reserve_exact(OWNERS);
    }
    // Registration already holds the registry lock, so no CAS retry is needed.
    let id = NEXT.load(Ordering::Relaxed);
    let Some(next) = id.checked_add(1) else {
        return ptr::null_mut();
    };
    NEXT.store(next, Ordering::Relaxed);
    let retained = if let Some(slot) = slots.iter_mut().find(|s| s.id == 0) {
        slot.id = id;
        slot.owner.take()
    } else if slots.len() < OWNERS {
        slots.push(Slot { id, owner: None });
        None
    } else {
        return ptr::null_mut();
    };
    drop(slots);
    let owner = match retained {
        Some(mut owner) => {
            owner.reset(rate).expect("positive rate");
            owner
        }
        None => Box::new(Player::new(rate).expect("positive rate")),
    };
    drop(Lease {
        id,
        owner: Some(owner),
    });
    id as *mut c_void
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_speech_player_queue_frame(
    handle: *mut c_void,
    frame: *const Frame,
    minimum: u32,
    fade: u32,
    index: i32,
    purge: bool,
) -> i32 {
    let frame = if frame.is_null() {
        None
    } else {
        // SAFETY: caller supplies one aligned initialized readable C frame;
        // it is copied before registration/effects and is never retained.
        Some(unsafe { frame.read() })
    };
    let mut lease = match Lease::enter(handle) {
        Ok(l) => l,
        Err(s) => return s,
    };
    lease
        .player()
        .frames
        .queue(frame, minimum, fade, index, purge)
        .map_or_else(status, |_| 0)
}
#[no_mangle]
pub unsafe extern "C" fn speechPlayer_queueFrame(
    handle: *mut c_void,
    frame: *mut Frame,
    minimum: u32,
    fade: u32,
    index: i32,
    purge: bool,
) {
    // SAFETY: forwarded legacy live frame/serialized owner contract. Checked
    // callers use the status API to observe backpressure/refusal.
    let _ =
        unsafe { espeak_rs_speech_player_queue_frame(handle, frame, minimum, fade, index, purge) };
}
#[no_mangle]
pub unsafe extern "C" fn espeak_rs_speech_player_synthesize(
    handle: *mut c_void,
    count: u32,
    samples: *mut i16,
    result: *mut u32,
) -> i32 {
    if result.is_null()
        || (count > 0 && samples.is_null())
        || count > i32::MAX as u32
        || count as usize > isize::MAX as usize / 2
    {
        return libc::EINVAL;
    }
    let mut lease = match Lease::enter(handle) {
        Ok(l) => l,
        Err(s) => return s,
    };
    let pointer = if count == 0 {
        ptr::NonNull::<i16>::dangling().as_ptr()
    } else {
        samples
    };
    // SAFETY: live exclusive initialized writable PCM extent of count shorts,
    // disjoint from result/owner. It is borrowed only for this synchronous call.
    let output = unsafe { std::slice::from_raw_parts_mut(pointer, count as usize) };
    let n = lease.player().synthesize(output, &mut || {
        // SAFETY: native C runtime rand has no engine callback/retained loan;
        // preserve its state/order and the target runtime's RAND_MAX.
        f64::from(unsafe { rand() }) / f64::from(libc::RAND_MAX)
    });
    // SAFETY: aligned writable result disjoint from live initialized PCM.
    unsafe { result.write(n as u32) };
    0
}
#[no_mangle]
pub unsafe extern "C" fn speechPlayer_synthesize(
    handle: *mut c_void,
    count: u32,
    samples: *mut i16,
) -> i32 {
    let mut result = 0;
    // SAFETY: forwarded PCM/owner contract; local result cannot alias output.
    if unsafe { espeak_rs_speech_player_synthesize(handle, count, samples, &mut result) } == 0 {
        result as i32
    } else {
        0
    }
}
#[no_mangle]
extern "C" fn speechPlayer_getLastIndex(handle: *mut c_void) -> i32 {
    Lease::enter(handle).map_or(-1, |mut l| l.player().frames.last_index())
}
#[no_mangle]
extern "C" fn espeak_rs_speech_player_terminate(handle: *mut c_void) -> i32 {
    let mut lease = match Lease::enter(handle) {
        Ok(l) => l,
        Err(s) => return s,
    };
    lease.player().frames.clear();
    let mut slots = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let slot = slots
        .iter_mut()
        .find(|s| s.id == lease.id)
        .expect("live lease slot");
    slot.id = 0;
    slot.owner = lease.owner.take();
    // Retire every request/identity, retaining at most 64 initialized owner
    // buffers for reuse. Reset restores DSP histories before the next lease.
    0
}
#[no_mangle]
extern "C" fn speechPlayer_terminate(handle: *mut c_void) {
    let _ = espeak_rs_speech_player_terminate(handle);
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_POOL: Mutex<()> = Mutex::new(());
    #[test]
    fn live_lease_refuses_reentry_and_stale_handle_cannot_address_replacement() {
        let _test_pool = TEST_POOL.lock().unwrap();
        let handle = speechPlayer_initialize(22050);
        assert!(!handle.is_null());
        let lease = Lease::enter(handle).unwrap();
        assert!(matches!(Lease::enter(handle), Err(libc::EAGAIN)));
        let key = handle as usize;
        assert!(std::thread::spawn(move || matches!(
            Lease::enter(key as *mut c_void),
            Err(libc::EAGAIN)
        ))
        .join()
        .unwrap());
        assert_eq!(espeak_rs_speech_player_terminate(handle), libc::EAGAIN);
        drop(lease);
        assert_eq!(espeak_rs_speech_player_terminate(handle), 0);
        let next = speechPlayer_initialize(22050);
        assert_ne!(next, handle);
        assert!(matches!(Lease::enter(handle), Err(libc::EINVAL)));
        assert_eq!(espeak_rs_speech_player_terminate(next), 0);
    }
    #[test]
    fn pending_frames_teardown_and_repeated_initialization_reuse_bounded_slots() {
        let _test_pool = TEST_POOL.lock().unwrap();
        let first = speechPlayer_initialize(22050);
        let address = Lease::enter(first).unwrap().player() as *mut Player;
        assert_eq!(espeak_rs_speech_player_terminate(first), 0);
        for _ in 0..200 {
            let handle = speechPlayer_initialize(22050);
            assert!(!handle.is_null());
            let mut lease = Lease::enter(handle).unwrap();
            assert_eq!(lease.player() as *mut Player, address);
            for _ in 0..256 {
                lease.player().frames.queue(None, 1, 1, -1, false).unwrap();
            }
            drop(lease);
            assert_eq!(espeak_rs_speech_player_terminate(handle), 0);
        }
        assert!(SLOTS.lock().unwrap().len() <= OWNERS);
        assert!(speechPlayer_initialize(0).is_null());
    }
}
