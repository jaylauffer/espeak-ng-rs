//! Retained clause scratch and admission, independent of the native C host.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    intonation, lengths,
    phoneme_list::{Output, Source},
};
use std::sync::Mutex;

// Standalone tests exercise admission/storage without linking the C host that
// consumes these buffers. CMake's native differential fixtures use all fields.
#[cfg_attr(not(espeak_native_host), allow(dead_code))]
pub(super) struct Scratch {
    pub(super) source: [Source; 1000],
    pub(super) output: [Output; 1001],
    pub(super) pitch: [intonation::Entry; 1001],
    pub(super) lengths: [lengths::Entry; 1001],
    pub(super) more: i32,
}
impl Scratch {
    fn new() -> Self {
        Self {
            source: [Source::default(); 1000],
            output: [Output::default(); 1001],
            pitch: [intonation::Entry::default(); 1001],
            lengths: [lengths::Entry::default(); 1001],
            more: 0,
        }
    }
}
struct Slot {
    owner: Option<Box<Scratch>>,
    active: bool,
}
static SLOT: Mutex<Slot> = Mutex::new(Slot {
    owner: None,
    active: false,
});
pub(super) struct Lease(Option<Box<Scratch>>);
impl Lease {
    pub(super) fn enter() -> Result<Self, i32> {
        let mut slot = SLOT.try_lock().map_err(|_| libc::EAGAIN)?;
        if slot.active {
            return Err(libc::EAGAIN);
        }
        slot.active = true;
        let owner = slot.owner.take();
        drop(slot);
        Ok(Self(Some(
            owner.unwrap_or_else(|| Box::new(Scratch::new())),
        )))
    }
    pub(super) fn scratch(&mut self) -> &mut Scratch {
        self.0.as_deref_mut().expect("active clause scratch")
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut slot = SLOT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slot.owner = self.0.take();
        slot.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_scratch_refuses_reentry_and_reuses_one_address() {
        let mut lease = Lease::enter().unwrap();
        let address = lease.scratch() as *mut Scratch;
        assert!(matches!(Lease::enter(), Err(libc::EAGAIN)));
        assert!(
            std::thread::spawn(|| matches!(Lease::enter(), Err(libc::EAGAIN)))
                .join()
                .unwrap()
        );
        lease.scratch().more = 7;
        drop(lease);
        for _ in 0..200 {
            let mut lease = Lease::enter().unwrap();
            assert_eq!(lease.scratch() as *mut Scratch, address);
            assert_eq!(lease.scratch().more, 7);
        }
        Lease::enter().unwrap().scratch().more = 0;
    }
}
