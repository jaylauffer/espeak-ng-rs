// SPDX-License-Identifier: GPL-3.0-or-later
use loadngo_proactor::{CompletionKind, PlatformPort, ProactorHandle};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// The watchdog closure must not retain its own host through a deferred queue
/// after stop. Drop clears that reference so the file workers can shut down.
pub struct HostDeadline(Arc<Mutex<Option<ProactorHandle<PlatformPort>>>>);
impl HostDeadline {
    pub fn new(handle: &ProactorHandle<PlatformPort>) -> Self {
        let owner = Arc::new(Mutex::new(Some(handle.clone())));
        let callback = Arc::clone(&owner);
        handle
            .defer_for(
                Duration::from_secs(10),
                CompletionKind::Timer,
                0,
                move |_| {
                    if let Some(handle) = callback.lock().unwrap().as_ref() {
                        handle.stop().unwrap();
                    }
                },
            )
            .unwrap();
        Self(owner)
    }
}
impl Drop for HostDeadline {
    fn drop(&mut self) {
        self.0.lock().unwrap().take();
    }
}
