//! Delivery of events as their audio plays (`event.c`), on a loadngo
//! proactor.
//!
//! In playback modes the synthesis thread declares each buffer's events
//! after writing its audio. Each declared event is copied (with its mark or
//! sound-icon name), queued, and given a proactor timer for when its sample
//! should be heard: the audio still queued for the device, less what follows
//! the event in the buffer just written. Without a device that knows its
//! queue, the delay is zero and events go out as soon as they are declared,
//! as `event.c` did. Normal deliveries call the owner on the proactor worker.
//! Teardown may report dropped terminations on the caller after worker exit. Events keep
//! their declared order: an event is due no earlier than the one before it,
//! and a timer delivers the due events at the front of the queue.
//!
//! As in `event.c`, a message's events start with a sentence event (one is
//! synthesized when a new message starts with another kind), and a clear
//! still reports the message-terminated events it drops. A clear waits for
//! a callback already running: it runs on the delivery thread, after any
//! delivery in progress. Events admitted after that clear are independent.
//! Full admission suspends on the caller's port, with 16 bounded waiters;
//! four separate clear reservations keep cleanup available under saturation.
//! Command cancellation interrupts ordinary capacity waits. Mandatory message
//! completions survive command cancellation and clear; worker exit releases
//! both kinds. Queued jobs retain weak owners, not their own port/context.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::events::Event;
use crate::synthesis_loop::{bind_io, run_on, Interrupt, Step, Wake};
use loadngo_proactor::{
    new_platform_proactor, CompletionKind, CompletionPort, PlatformPort, Proactor, ProactorHandle,
};
use std::collections::VecDeque;
use std::ffi::CString;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, ThreadId};
use std::time::{Duration, Instant};

pub const LIST_TERMINATED: i32 = 0;
pub const WORD: i32 = 1;
pub const SENTENCE: i32 = 2;
pub const MARK: i32 = 3;
pub const PLAY: i32 = 4;
pub const END: i32 = 5;
pub const MSG_TERMINATED: i32 = 6;
pub const PHONEME: i32 = 7;

/// `event.c`'s bound on queued events.
pub const MAX_EVENTS: usize = 1000;
/// Bounded concurrent admission waits, independent of event capacity.
pub const MAX_WAITERS: usize = 16;
/// Cleanup has separate reservations so full admission cannot block clear.
pub const MAX_CLEAR_WAITERS: usize = 4;

/// Why a declare was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    /// [`MAX_EVENTS`] are queued (`ENS_EVENT_BUFFER_FULL`).
    Full,
    /// The delivery thread has stopped.
    Stopped,
}

/// A declared event with its own copy of any name.
#[derive(Clone, Debug)]
pub struct Declared {
    pub event: Event,
    pub name: Option<CString>,
}

impl Declared {
    /// The event as the callback sees it, with `id` pointing at the copy.
    fn view(&self) -> Event {
        let mut event = self.event;
        if let Some(name) = &self.name {
            event.id = (name.as_ptr() as usize).to_ne_bytes();
        }
        event
    }
}

/// Receives each delivered event list: the event, then a terminator.
pub type Callback = Box<dyn FnMut(&[Event; 2]) + Send>;

struct Queue {
    /// Each event with when it is due; due times never decrease.
    pending: VecDeque<(Instant, Declared)>,
    stopped: bool,
    finished: bool,
    epoch: u64,
    waiters: [Option<WaitEntry>; MAX_WAITERS + MAX_CLEAR_WAITERS],
}

trait Notice: Send + Sync {
    fn notify(&self, cancel: bool);
}
struct WaitEntry {
    control: bool,
    mandatory: bool,
    notice: Arc<dyn Notice>,
}
fn notify(queue: &Queue, control: bool, cancel: bool) {
    for entry in queue
        .waiters
        .iter()
        .flatten()
        .filter(|e| e.control == control)
    {
        // Completion notifications release the caller's user data. Clear
        // frees their capacity rather than discarding an unadmitted one.
        entry
            .notice
            .notify(cancel && (!entry.mandatory || queue.stopped));
    }
}
struct Wait<P: CompletionPort>(Mutex<WaitState<P>>);
struct WaitState<P: CompletionPort> {
    wake: Option<Wake<P>>,
    cancelled: bool,
    done: bool,
    error: Option<io::Error>,
}
impl<P: CompletionPort> Wait<P> {
    fn new() -> Self {
        Self(Mutex::new(WaitState {
            wake: None,
            cancelled: false,
            done: false,
            error: None,
        }))
    }
    fn complete(&self) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).done = true;
        self.notify(false);
    }
}
impl<P: CompletionPort> Notice for Wait<P> {
    fn notify(&self, cancel: bool) {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        state.cancelled |= cancel;
        if let Some(wake) = &state.wake {
            if state.cancelled {
                let _ = wake.cancel();
            } else {
                let _ = wake.wake();
            }
        }
    }
}
impl<P: CompletionPort> Interrupt for Wait<P> {
    fn interrupt(&self) {
        self.notify(true);
    }
}
struct Registration {
    shared: Arc<Shared>,
    slot: usize,
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.shared.lock().waiters[self.slot] = None;
    }
}

/// What only the delivery thread uses while calling back (and a final
/// clear after it stops).
struct Notifier {
    previous_uid: u32,
}
struct Callbacks {
    current: Option<Callback>,
    epoch: u64,
}
impl Callbacks {
    fn invoke(callbacks: &Mutex<Self>, list: &[Event; 2]) -> bool {
        let (callback, epoch) = {
            let mut state = callbacks.lock().unwrap_or_else(|p| p.into_inner());
            (state.current.take(), state.epoch)
        };
        let Some(mut callback) = callback else {
            return false;
        };
        callback(list);
        let mut state = callbacks.lock().unwrap_or_else(|p| p.into_inner());
        if state.epoch == epoch {
            state.current = Some(callback);
        } else {
            drop(state);
            drop(callback);
        }
        true
    }
}

impl Notifier {
    /// `event_notify`: the callback, preceded by a sentence event when a
    /// new message does not start with one.
    fn notify(
        &mut self,
        declared: &Declared,
        callbacks: &Mutex<Callbacks>,
        stopped: impl Fn() -> bool,
    ) {
        let event = declared.view();
        let mut list = [event, event];
        list[1].kind = LIST_TERMINATED;
        let notified = match event.kind {
            SENTENCE => Callbacks::invoke(callbacks, &list),
            MSG_TERMINATED | MARK | WORD | END | PHONEME => {
                let mut notified = false;
                if self.previous_uid != event.unique_identifier {
                    list[0].kind = SENTENCE;
                    notified = Callbacks::invoke(callbacks, &list);
                    if stopped() {
                        if notified {
                            self.previous_uid = event.unique_identifier;
                        }
                        return;
                    }
                    list[0].kind = event.kind;
                }
                Callbacks::invoke(callbacks, &list) || notified
            }
            _ => return,
        };
        if notified {
            self.previous_uid = event.unique_identifier;
        }
    }
}

struct Shared {
    queue: Mutex<Queue>,
    notifier: Mutex<Notifier>,
    callbacks: Mutex<Callbacks>,
    thread: Mutex<Option<ThreadId>>,
    handle: ProactorHandle<PlatformPort>,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn pop_due(&self) -> Option<Declared> {
        let mut queue = self.lock();
        if queue.stopped {
            return None;
        }
        if queue.pending.front()?.0 > Instant::now() {
            return None;
        }
        let popped = queue.pending.pop_front().map(|(_, declared)| declared);
        notify(&queue, false, false);
        popped
    }

    /// Delivers the due events at the front of the queue, in order. The
    /// queue is not held during the callback, so the synthesis thread can
    /// keep declaring.
    fn deliver_due(&self) {
        let mut notifier = self.notifier.lock().unwrap_or_else(|p| p.into_inner());
        for _ in 0..MAX_EVENTS {
            let Some(declared) = self.pop_due() else {
                break;
            };
            notifier.notify(&declared, &self.callbacks, || self.lock().stopped);
        }
    }

    /// Drops what is queued, still reporting terminated messages. From
    /// inside a callback (which holds the notifier) they cannot be
    /// reported; `event.c` deadlocked there.
    fn clear(&self) {
        let notifier = self.notifier.try_lock();
        let (count, epoch) = {
            let mut queue = self.lock();
            queue.epoch = queue.epoch.wrapping_add(1);
            notify(&queue, false, true);
            if notifier.is_err() {
                queue.pending.clear();
                return;
            }
            (queue.pending.len(), queue.epoch)
        };
        let mut notifier = notifier.expect("checked notifier");
        for _ in 0..count {
            let declared = {
                let mut queue = self.lock();
                if queue.epoch != epoch {
                    break; // a reentrant clear invalidated this snapshot
                }
                let Some((_, declared)) = queue.pending.pop_front() else {
                    break;
                };
                notify(&queue, false, false);
                declared
            };
            if declared.event.kind == MSG_TERMINATED {
                notifier.notify(&declared, &self.callbacks, || false);
            }
        }
    }

    fn admit(
        self: &Arc<Self>,
        queue: &mut Queue,
        declared: &mut Option<Declared>,
        delay: Duration,
    ) -> Result<(), Refused> {
        if queue.stopped || !self.handle.is_running() {
            return Err(Refused::Stopped);
        }
        if queue.pending.len() >= MAX_EVENTS {
            return Err(Refused::Full);
        }
        let due = Instant::now() + delay;
        let due = queue.pending.back().map_or(due, |&(last, _)| due.max(last));
        queue
            .pending
            .push_back((due, declared.take().expect("unadmitted event")));
        // Posting is serialized with admission/removal. Failure rolls back
        // this event rather than another producer's equal-deadline tail.
        let weak = Arc::downgrade(self);
        let deliver = move |_| {
            if let Some(shared) = weak.upgrade() {
                shared.deliver_due();
            }
        };
        let posted = if delay.is_zero() {
            self.handle.enqueue_work(deliver)
        } else {
            self.handle
                .defer_for(delay, CompletionKind::Job, 0, deliver)
        };
        posted.map_err(|_| {
            *declared = queue.pending.pop_back().map(|(_, declared)| declared);
            Refused::Stopped
        })
    }
}

struct WorkerFinished(Arc<Shared>);
impl Drop for WorkerFinished {
    fn drop(&mut self) {
        let mut queue = self.0.lock();
        queue.stopped = true;
        queue.finished = true;
        let _ = self.0.handle.stop();
        notify(&queue, false, true);
        notify(&queue, true, false);
    }
}

/// The delivery thread and its proactor.
pub struct Delivery {
    shared: Arc<Shared>,
    handle: ProactorHandle<PlatformPort>,
    thread: Mutex<Option<JoinHandle<io::Result<()>>>>,
}

impl Delivery {
    pub fn new(callback: Option<Callback>) -> io::Result<Self> {
        let proactor = new_platform_proactor()?;
        let handle = proactor.handle();
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: VecDeque::with_capacity(MAX_EVENTS),
                stopped: false,
                finished: false,
                epoch: 0,
                waiters: std::array::from_fn(|_| None),
            }),
            notifier: Mutex::new(Notifier { previous_uid: 0 }),
            callbacks: Mutex::new(Callbacks {
                current: callback,
                epoch: 0,
            }),
            thread: Mutex::new(None),
            handle: handle.clone(),
        });
        let finished = WorkerFinished(shared.clone());
        let thread = std::thread::Builder::new()
            .name("espeak-events".into())
            .spawn(move || {
                let _finished = finished;
                proactor.run_until_stopped()
            })?;
        *shared.thread.lock().unwrap_or_else(|p| p.into_inner()) = Some(thread.thread().id());
        Ok(Self {
            shared,
            handle,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn set_callback(&self, callback: Option<Callback>) {
        let old = {
            let mut state = self
                .shared
                .callbacks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            state.epoch = state.epoch.wrapping_add(1);
            std::mem::replace(&mut state.current, callback)
        };
        drop(old);
    }

    /// Queues `declared` for delivery after `delay`.
    pub fn declare(&self, declared: Declared, delay: Duration) -> Result<(), Refused> {
        self.shared
            .admit(&mut self.shared.lock(), &mut Some(declared), delay)
    }

    /// Events queued and not yet delivered.
    pub fn pending(&self) -> usize {
        self.shared.lock().pending.len()
    }

    fn reserve<P: CompletionPort>(
        &self,
        queue: &mut Queue,
        wait: Arc<Wait<P>>,
        control: bool,
        mandatory: bool,
    ) -> io::Result<Registration> {
        let slots = if control {
            MAX_WAITERS..MAX_WAITERS + MAX_CLEAR_WAITERS
        } else {
            0..MAX_WAITERS
        };
        let slot = slots
            .into_iter()
            .find(|&slot| queue.waiters[slot].is_none())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::WouldBlock, "event waiter limit reached")
            })?;
        queue.waiters[slot] = Some(WaitEntry {
            control,
            mandatory,
            notice: wait,
        });
        Ok(Registration {
            shared: self.shared.clone(),
            slot,
        })
    }

    /// Declare once room is available, suspending on capacity completions.
    /// The event/name remains owned while Pending. Clear/command cancellation
    /// invalidate ordinary admissions; mandatory message completions survive
    /// both so caller user-data cleanup is not lost. Delivery termination or
    /// failure releases either kind with an error.
    pub fn declare_wait(&self, declared: Declared, delay: Duration) -> io::Result<()> {
        let host = crate::synthesis_loop::host()?;
        self.declare_on(&host, declared, delay)
    }

    pub fn declare_on<P: CompletionPort>(
        &self,
        host: &Proactor<P>,
        declared: Declared,
        delay: Duration,
    ) -> io::Result<()> {
        let mut declared = Some(declared);
        let mandatory = declared
            .as_ref()
            .is_some_and(|e| e.event.kind == MSG_TERMINATED);
        let (wait, epoch, _registration) = {
            let mut queue = self.shared.lock();
            match self.shared.admit(&mut queue, &mut declared, delay) {
                Ok(()) => return Ok(()),
                Err(Refused::Stopped) => return Err(io::ErrorKind::Interrupted.into()),
                Err(Refused::Full) if self.on_delivery_thread() => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "event callback cannot wait for its own queue",
                    ));
                }
                Err(Refused::Full) => {}
            }
            let wait = Arc::new(Wait::<P>::new());
            let registration = self.reserve(&mut queue, wait.clone(), false, mandatory)?;
            (wait, queue.epoch, registration)
        };
        // A cancelled command must still complete its caller's user-data
        // lifetime. This cleanup wait is shielded from command cancellation;
        // delivery termination/failure still releases it with an error.
        let binding = (!mandatory).then(|| bind_io(wait.clone()));
        if binding.as_ref().is_some_and(|binding| binding.requested) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let shared = self.shared.clone();
        let active = wait.clone();
        run_on(host, move |wake| {
            let mut queue = shared.lock();
            let mut state = active.0.lock().unwrap_or_else(|p| p.into_inner());
            state.wake = Some(wake.clone());
            if state.cancelled || queue.stopped || (!mandatory && queue.epoch != epoch) {
                let _ = wake.cancel();
                return Step::Done;
            }
            match shared.admit(&mut queue, &mut declared, delay) {
                Ok(()) => Step::Done,
                Err(Refused::Full) => Step::Pending,
                Err(Refused::Stopped) => {
                    state.error = Some(io::ErrorKind::BrokenPipe.into());
                    Step::Done
                }
            }
        })?;
        let error = wait
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .error
            .take();
        error.map_or(Ok(()), Err)
    }

    /// A callback barrier using the caller's port. Worker exit wakes this
    /// wait even when a previously accepted clear job never executes.
    pub fn clear_on<P: CompletionPort>(&self, host: &Proactor<P>) -> io::Result<()> {
        if self.on_delivery_thread() {
            self.shared.clear();
            return Ok(());
        }
        let wait = Arc::new(Wait::<P>::new());
        let _registration = self.reserve(&mut self.shared.lock(), wait.clone(), true, false)?;
        let shared = self.shared.clone();
        let active = wait.clone();
        let mut posted = false;
        run_on(host, move |wake| {
            let queue = shared.lock();
            if queue.finished {
                drop(queue);
                shared.clear();
                return Step::Done;
            }
            {
                let mut state = active.0.lock().unwrap_or_else(|p| p.into_inner());
                state.wake = Some(wake.clone());
                if state.done {
                    return Step::Done;
                }
            }
            drop(queue);
            if !posted {
                posted = true;
                let owner = Arc::downgrade(&shared);
                let request = Arc::downgrade(&active);
                if let Err(error) = shared.handle.enqueue_work(move |_| {
                    if let (Some(shared), Some(request)) = (owner.upgrade(), request.upgrade()) {
                        shared.clear();
                        request.complete();
                    }
                }) {
                    active.0.lock().unwrap_or_else(|p| p.into_inner()).error = Some(error);
                    return Step::Done;
                }
            }
            Step::Pending
        })?;
        let error = wait
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .error
            .take();
        error.map_or(Ok(()), Err)
    }

    /// `event_clear_all`: drops queued events (reporting terminated
    /// messages) on the delivery thread, and returns once it has.
    pub fn clear(&self) -> io::Result<()> {
        let on_delivery_thread = *self.shared.thread.lock().unwrap_or_else(|p| p.into_inner())
            == Some(std::thread::current().id());
        if on_delivery_thread {
            self.shared.clear();
            return Ok(());
        }
        let host = crate::synthesis_loop::host()?;
        self.clear_on(&host)
    }

    /// Stop admission immediately; external callers join the worker before
    /// releasing the callback owner. A worker callback never joins itself.
    pub fn terminate(&self) {
        {
            let mut queue = self.shared.lock();
            queue.stopped = true;
            notify(&queue, false, true);
        }
        let _ = self.handle.stop();
        if !self.on_delivery_thread() {
            let thread = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take();
            if let Some(thread) = thread {
                let _ = thread.join();
            }
        }
    }

    fn on_delivery_thread(&self) -> bool {
        *self.shared.thread.lock().unwrap_or_else(|p| p.into_inner())
            == Some(std::thread::current().id())
    }
}

impl Drop for Delivery {
    /// `event_terminate`: stops the thread; queued events are dropped.
    fn drop(&mut self) {
        self.terminate();
        self.shared.clear();
    }
}

/// The C API replacing `event.c`'s functions.
#[cfg(feature = "c-abi")]
mod c_api {
    use super::{Callback, Declared, Delivery, Refused, MARK, PLAY};
    use crate::events::Event;
    use std::ffi::{c_char, c_int, c_void, CStr};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    type SynthCallback = unsafe extern "C" fn(*mut i16, c_int, *mut Event) -> c_int;

    static DELIVERY: Mutex<Option<Arc<Delivery>>> = Mutex::new(None);
    static CALLBACK: Mutex<Option<SynthCallback>> = Mutex::new(None);

    const ENS_OK: c_int = 0;
    const EINVAL: c_int = 22;
    /// `ENS_EVENT_BUFFER_FULL`
    const BUFFER_FULL: c_int = 0x1000_09FF;
    const ENS_AUDIO_ERROR: c_int = 0x1000_05FF;

    fn callback() -> Option<Callback> {
        let callback = (*CALLBACK.lock().unwrap_or_else(|p| p.into_inner()))?;
        Some(Box::new(move |list: &[Event; 2]| {
            let mut list = *list;
            // SAFETY: the owner's callback over a terminated two-event list
            // that lives for the call.
            unsafe { callback(std::ptr::null_mut(), 0, list.as_mut_ptr()) };
        }))
    }

    fn with<R>(f: impl FnOnce(&Delivery) -> R) -> Option<R> {
        let delivery = DELIVERY.lock().unwrap_or_else(|p| p.into_inner()).clone();
        delivery.as_deref().map(f)
    }

    /// `event_set_callback`.
    #[no_mangle]
    extern "C" fn espeak_rs_event_set_callback(callback: Option<SynthCallback>) {
        *CALLBACK.lock().unwrap_or_else(|p| p.into_inner()) = callback;
        with(|delivery| delivery.set_callback(self::callback()));
    }

    /// `event_init`: starts the delivery thread if it is not running.
    /// Returns 0, or nonzero when no proactor or thread could be made.
    #[no_mangle]
    extern "C" fn espeak_rs_event_init() -> c_int {
        let mut delivery = DELIVERY.lock().unwrap_or_else(|p| p.into_inner());
        if delivery.is_none() {
            match Delivery::new(callback()) {
                Ok(started) => *delivery = Some(Arc::new(started)),
                Err(_) => return ENS_AUDIO_ERROR,
            }
        }
        ENS_OK
    }

    /// `event_declare`, delivered `delay_ms` from now.
    ///
    /// # Safety
    /// `event` is null or a live event whose mark or sound-icon name, if
    /// any, is a C string.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_event_declare(event: *const Event, delay_ms: c_int) -> c_int {
        // SAFETY: same caller contract.
        unsafe { declare(event, delay_ms, false) }
    }

    /// Blocking admission with completion-driven capacity/back-pressure.
    /// # Safety
    /// Same event/name lifetime as espeak_rs_event_declare.
    #[no_mangle]
    unsafe extern "C" fn espeak_rs_event_declare_wait(
        event: *const Event,
        delay_ms: c_int,
    ) -> c_int {
        // SAFETY: same caller contract.
        unsafe { declare(event, delay_ms, true) }
    }

    unsafe fn declare(event: *const Event, delay_ms: c_int, wait: bool) -> c_int {
        // SAFETY: caller contract.
        let Some(event) = (unsafe { event.as_ref() }).copied() else {
            return EINVAL;
        };
        let name = match event.kind {
            MARK | PLAY => {
                let pointer = usize::from_ne_bytes(event.id) as *const c_char;
                // SAFETY: caller contract.
                (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_owned())
            }
            _ => None,
        };
        let delay = Duration::from_millis(u64::try_from(delay_ms).unwrap_or(0));
        let declared = Declared { event, name };
        // event.c tolerated a declare before its init; start the thread
        if espeak_rs_event_init() != ENS_OK {
            return ENS_AUDIO_ERROR;
        }
        if wait {
            return match with(|delivery| delivery.declare_wait(declared, delay)) {
                Some(Ok(())) => ENS_OK,
                Some(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => 0x1000_0EFF,
                Some(Err(error)) if error.kind() == std::io::ErrorKind::WouldBlock => EINVAL,
                _ => ENS_AUDIO_ERROR,
            };
        }
        match with(|delivery| delivery.declare(declared, delay)) {
            Some(Ok(())) => ENS_OK,
            Some(Err(Refused::Full)) => BUFFER_FULL,
            Some(Err(Refused::Stopped)) | None => ENS_AUDIO_ERROR,
        }
    }

    /// `event_clear_all`.
    #[no_mangle]
    extern "C" fn espeak_rs_event_clear_all() -> c_int {
        if with(|delivery| delivery.clear()).is_none_or(|result| result.is_ok()) {
            ENS_OK
        } else {
            ENS_AUDIO_ERROR
        }
    }

    /// `event_terminate`: stops the delivery thread.
    #[no_mangle]
    extern "C" fn espeak_rs_event_terminate() {
        let delivery = DELIVERY.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(delivery) = &delivery {
            delivery.terminate();
        }
        drop(delivery);
    }

    /// Events queued and not yet delivered (for tests).
    #[no_mangle]
    extern "C" fn espeak_rs_event_pending() -> c_int {
        with(|delivery| delivery.pending() as c_int).unwrap_or(0)
    }

    const _: () = assert!(std::mem::size_of::<*mut c_void>() == std::mem::size_of::<usize>());

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::mpsc;
        struct Gate {
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
            completed: mpsc::Sender<bool>,
        }
        static GATE: Mutex<Option<Gate>> = Mutex::new(None);
        unsafe extern "C" fn reenter(_: *mut i16, _: c_int, _: *mut Event) -> c_int {
            let gate = GATE.lock().unwrap_or_else(|p| p.into_inner()).take();
            let Some(gate) = gate else { return 0 };
            let _ = gate.entered.send(());
            if gate.release.recv_timeout(Duration::from_secs(5)).is_err() {
                let _ = gate.completed.send(false);
                return 0;
            }
            // A failed global-lock check stays bounded even with the old
            // adapter; it must not hang an extern C callback or unwind it.
            let free = DELIVERY.try_lock().is_ok();
            if free {
                espeak_rs_event_set_callback(None);
                let _ = espeak_rs_event_pending();
                let _ = espeak_rs_event_clear_all();
                espeak_rs_event_terminate();
            }
            let _ = gate.completed.send(free);
            0
        }
        #[test]
        fn c_adapter_releases_global_owner_lock_before_waiting_for_callback() {
            espeak_rs_event_terminate();
            let (entered, blocked) = mpsc::channel();
            let (release, released) = mpsc::channel();
            let (completed, report) = mpsc::channel();
            *GATE.lock().unwrap() = Some(Gate {
                entered,
                release: released,
                completed,
            });
            espeak_rs_event_set_callback(Some(reenter));
            let event = Event {
                kind: super::super::SENTENCE,
                unique_identifier: 55,
                ..Event::default()
            };
            // SAFETY: live event with no borrowed name.
            assert_eq!(unsafe { espeak_rs_event_declare(&event, 0) }, ENS_OK);
            blocked.recv_timeout(Duration::from_secs(5)).unwrap();
            let (started, starting) = mpsc::channel();
            let clear = std::thread::spawn(move || {
                // This is the same ownership helper as the C clear API.
                // The signal fires after acquiring its owner and before
                // waiting for the currently blocked callback to return.
                with(|delivery| {
                    started.send(()).unwrap();
                    delivery.clear()
                })
                .unwrap()
            });
            starting.recv_timeout(Duration::from_secs(5)).unwrap();
            release.send(()).unwrap();
            let free = report.recv_timeout(Duration::from_secs(5)).unwrap();
            clear.join().unwrap().unwrap();
            espeak_rs_event_terminate();
            espeak_rs_event_set_callback(None);
            assert!(
                free,
                "event adapter held its global owner across a callback barrier"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn event(kind: i32, uid: u32, position: i32) -> Declared {
        Declared {
            event: Event {
                kind,
                unique_identifier: uid,
                text_position: position,
                ..Event::default()
            },
            name: None,
        }
    }

    type Log = Arc<Mutex<Vec<(i32, u32, i32, Instant, std::thread::ThreadId)>>>;

    fn recorder() -> (Log, Callback, mpsc::Receiver<()>) {
        let log: Log = Arc::default();
        let sink = Arc::clone(&log);
        let (done, completed) = mpsc::channel();
        let callback: Callback = Box::new(move |list: &[Event; 2]| {
            assert_eq!(list[1].kind, LIST_TERMINATED);
            sink.lock().unwrap().push((
                list[0].kind,
                list[0].unique_identifier,
                list[0].text_position,
                Instant::now(),
                std::thread::current().id(),
            ));
            let _ = done.send(());
        });
        (log, callback, completed)
    }

    /// Waits until `done` holds (the last event is popped before its
    /// callback runs, so the queue emptying is not enough).
    fn wait_until(done: impl Fn() -> bool, completed: &mpsc::Receiver<()>) {
        while !done() {
            completed.recv_timeout(Duration::from_secs(5)).unwrap();
        }
    }

    #[test]
    fn events_arrive_in_order_at_their_time_on_the_delivery_thread() {
        let (log, callback, completed) = recorder();
        let delivery = Delivery::new(Some(callback)).unwrap();
        let start = Instant::now();
        delivery
            .declare(event(SENTENCE, 1, 1), Duration::ZERO)
            .unwrap();
        delivery
            .declare(event(WORD, 1, 2), Duration::from_millis(60))
            .unwrap();
        // an earlier timer still waits for the event declared before it
        delivery
            .declare(event(WORD, 1, 3), Duration::from_millis(20))
            .unwrap();
        delivery
            .declare(event(MSG_TERMINATED, 1, 4), Duration::from_millis(80))
            .unwrap();
        wait_until(|| log.lock().unwrap().len() == 4, &completed);
        let log = log.lock().unwrap();
        let order: Vec<_> = log.iter().map(|e| (e.0, e.2)).collect();
        assert_eq!(
            order,
            [(SENTENCE, 1), (WORD, 2), (WORD, 3), (MSG_TERMINATED, 4)]
        );
        assert!(log[1].3 - start >= Duration::from_millis(60));
        assert!(log[3].3 - start >= Duration::from_millis(80));
        assert!(log[0].3 - start < Duration::from_millis(60));
        assert!(log.iter().all(|e| e.4 != std::thread::current().id()));
    }

    #[test]
    fn a_new_message_starts_with_a_sentence() {
        let (log, callback, completed) = recorder();
        let delivery = Delivery::new(Some(callback)).unwrap();
        delivery.declare(event(WORD, 7, 1), Duration::ZERO).unwrap();
        delivery.declare(event(WORD, 7, 2), Duration::ZERO).unwrap();
        delivery.declare(event(PLAY, 7, 3), Duration::ZERO).unwrap();
        delivery.declare(event(END, 8, 4), Duration::ZERO).unwrap();
        wait_until(|| log.lock().unwrap().len() == 5, &completed);
        let kinds: Vec<_> = log.lock().unwrap().iter().map(|e| (e.0, e.1)).collect();
        // PLAY is not delivered
        assert_eq!(
            kinds,
            [(SENTENCE, 7), (WORD, 7), (WORD, 7), (SENTENCE, 8), (END, 8)]
        );
    }

    #[test]
    fn names_are_copied_and_presented() {
        let seen = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&seen);
        let (done, completed) = mpsc::channel();
        let delivery = Delivery::new(Some(Box::new(move |list: &[Event; 2]| {
            if list[0].kind == MARK {
                let pointer = usize::from_ne_bytes(list[0].id) as *const std::ffi::c_char;
                // SAFETY: the delivered mark's name, live for the call.
                let name = unsafe { std::ffi::CStr::from_ptr(pointer) };
                *sink.lock().unwrap() = name.to_string_lossy().into_owned();
                let _ = done.send(());
            }
        })))
        .unwrap();
        let mut mark = event(MARK, 0, 1);
        mark.name = Some(CString::new("here").unwrap());
        delivery.declare(mark, Duration::ZERO).unwrap();
        wait_until(|| !seen.lock().unwrap().is_empty(), &completed);
        assert_eq!(*seen.lock().unwrap(), "here");
    }

    #[test]
    fn clear_drops_pending_but_reports_terminations() {
        let (log, callback, _completed) = recorder();
        let delivery = Delivery::new(Some(callback)).unwrap();
        for i in 0..3 {
            delivery
                .declare(event(WORD, 3, i), Duration::from_secs(30))
                .unwrap();
        }
        delivery
            .declare(event(MSG_TERMINATED, 3, 9), Duration::from_secs(30))
            .unwrap();
        delivery.clear().unwrap();
        assert_eq!(delivery.pending(), 0);
        let kinds: Vec<_> = log.lock().unwrap().iter().map(|e| (e.0, e.2)).collect();
        assert_eq!(kinds, [(SENTENCE, 9), (MSG_TERMINATED, 9)]);
        // the bound
        for i in 0..MAX_EVENTS {
            delivery
                .declare(event(WORD, 3, i as i32), Duration::from_secs(30))
                .unwrap();
        }
        assert_eq!(
            delivery.declare(event(WORD, 3, 0), Duration::ZERO),
            Err(Refused::Full)
        );
        // dropping stops the thread without waiting for the timers
        let start = Instant::now();
        drop(delivery);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[cfg(test)]
#[path = "event_delivery_tests.rs"]
mod wait_tests;
