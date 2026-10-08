//! Delivery of events as their audio plays (`event.c`), on a loadngo
//! proactor.
//!
//! In playback modes the synthesis thread declares each buffer's events
//! after writing its audio. Each declared event is copied (with its mark or
//! sound-icon name), queued, and given a proactor timer for when its sample
//! should be heard: the audio still queued for the device, less what follows
//! the event in the buffer just written. Without a device that knows its
//! queue, the delay is zero and events go out as soon as they are declared,
//! as `event.c` did. One thread runs the proactor and calls the owner's
//! callback, so the callback never runs on the synthesis thread. Events keep
//! their declared order: an event is due no earlier than the one before it,
//! and a timer delivers the due events at the front of the queue.
//!
//! As in `event.c`, a message's events start with a sentence event (one is
//! synthesized when a new message starts with another kind), and a clear
//! still reports the message-terminated events it drops. A clear waits for
//! a callback already running: it runs on the delivery thread, after any
//! delivery in progress, so nothing is delivered after it returns.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::events::Event;
use loadngo_proactor::{new_platform_proactor, CompletionKind, PlatformPort, ProactorHandle};
use std::collections::VecDeque;
use std::ffi::CString;
use std::io;
use std::sync::{mpsc, Arc, Mutex};
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
}

/// What only the delivery thread uses while calling back (and a final
/// clear after it stops).
struct Notifier {
    previous_uid: u32,
    callback: Option<Callback>,
}

impl Notifier {
    /// `event_notify`: the callback, preceded by a sentence event when a
    /// new message does not start with one.
    fn notify(&mut self, declared: &Declared) {
        let Some(callback) = self.callback.as_mut() else {
            return;
        };
        let event = declared.view();
        let mut list = [event, event];
        list[1].kind = LIST_TERMINATED;
        match event.kind {
            SENTENCE => callback(&list),
            MSG_TERMINATED | MARK | WORD | END | PHONEME => {
                if self.previous_uid != event.unique_identifier {
                    list[0].kind = SENTENCE;
                    callback(&list);
                    list[0].kind = event.kind;
                }
                callback(&list);
            }
            _ => return,
        }
        self.previous_uid = event.unique_identifier;
    }
}

struct Shared {
    queue: Mutex<Queue>,
    notifier: Mutex<Notifier>,
    thread: Mutex<Option<ThreadId>>,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn pop_due(&self) -> Option<Declared> {
        let mut queue = self.lock();
        if queue.pending.front()?.0 > Instant::now() {
            return None;
        }
        queue.pending.pop_front().map(|(_, declared)| declared)
    }

    /// Delivers the due events at the front of the queue, in order. The
    /// queue is not held during the callback, so the synthesis thread can
    /// keep declaring.
    fn deliver_due(&self) {
        let mut notifier = self.notifier.lock().unwrap_or_else(|p| p.into_inner());
        while let Some(declared) = self.pop_due() {
            notifier.notify(&declared);
        }
    }

    /// Drops what is queued, still reporting terminated messages. From
    /// inside a callback (which holds the notifier) they cannot be
    /// reported; `event.c` deadlocked there.
    fn clear(&self) {
        let dropped = std::mem::take(&mut self.lock().pending);
        let Ok(mut notifier) = self.notifier.try_lock() else {
            return;
        };
        for (_, declared) in dropped {
            if declared.event.kind == MSG_TERMINATED {
                notifier.notify(&declared);
            }
        }
    }
}

/// The delivery thread and its proactor.
pub struct Delivery {
    shared: Arc<Shared>,
    handle: ProactorHandle<PlatformPort>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl Delivery {
    pub fn new(callback: Option<Callback>) -> io::Result<Self> {
        let proactor = new_platform_proactor()?;
        let handle = proactor.handle();
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: VecDeque::new(),
            }),
            notifier: Mutex::new(Notifier {
                previous_uid: 0,
                callback,
            }),
            thread: Mutex::new(None),
        });
        let thread = std::thread::Builder::new()
            .name("espeak-events".into())
            .spawn(move || proactor.run_until_stopped())?;
        *shared.thread.lock().unwrap_or_else(|p| p.into_inner()) = Some(thread.thread().id());
        Ok(Self {
            shared,
            handle,
            thread: Some(thread),
        })
    }

    pub fn set_callback(&self, callback: Option<Callback>) {
        self.shared
            .notifier
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .callback = callback;
    }

    /// Queues `declared` for delivery after `delay`.
    pub fn declare(&self, declared: Declared, delay: Duration) -> Result<(), Refused> {
        let due = {
            let mut queue = self.shared.lock();
            if queue.pending.len() >= MAX_EVENTS {
                return Err(Refused::Full);
            }
            let due = Instant::now() + delay;
            let due = queue.pending.back().map_or(due, |&(last, _)| due.max(last));
            queue.pending.push_back((due, declared));
            due
        };
        let shared = Arc::clone(&self.shared);
        let deliver = move |_| shared.deliver_due();
        let posted = if delay.is_zero() {
            self.handle.enqueue_work(deliver)
        } else {
            self.handle
                .defer_for(delay, CompletionKind::Job, 0, deliver)
        };
        posted.map_err(|_| {
            let mut queue = self.shared.lock();
            if queue.pending.back().is_some_and(|&(last, _)| last == due) {
                queue.pending.pop_back();
            }
            Refused::Stopped
        })
    }

    /// Events queued and not yet delivered.
    pub fn pending(&self) -> usize {
        self.shared.lock().pending.len()
    }

    /// `event_clear_all`: drops queued events (reporting terminated
    /// messages) on the delivery thread, and returns once it has.
    pub fn clear(&self) {
        let on_delivery_thread = *self.shared.thread.lock().unwrap_or_else(|p| p.into_inner())
            == Some(std::thread::current().id());
        if on_delivery_thread {
            self.shared.clear();
            return;
        }
        let (done, cleared) = mpsc::channel();
        let shared = Arc::clone(&self.shared);
        let posted = self.handle.enqueue_work(move |_| {
            shared.clear();
            let _ = done.send(());
        });
        if posted.is_err() || cleared.recv().is_err() {
            // the thread has stopped: clear here
            self.shared.clear();
        }
    }
}

impl Drop for Delivery {
    /// `event_terminate`: stops the thread; queued events are dropped.
    fn drop(&mut self) {
        let _ = self.handle.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.shared.clear();
    }
}

/// The C API replacing `event.c`'s functions.
#[cfg(feature = "c-abi")]
mod c_api {
    use super::{Callback, Declared, Delivery, Refused, MARK, PLAY};
    use crate::events::Event;
    use std::ffi::{c_char, c_int, c_void, CStr};
    use std::sync::Mutex;
    use std::time::Duration;

    type SynthCallback = unsafe extern "C" fn(*mut i16, c_int, *mut Event) -> c_int;

    static DELIVERY: Mutex<Option<Delivery>> = Mutex::new(None);
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
        DELIVERY
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(f)
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
                Ok(started) => *delivery = Some(started),
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
        match with(|delivery| delivery.declare(declared, delay)) {
            Some(Ok(())) => ENS_OK,
            Some(Err(Refused::Full)) => BUFFER_FULL,
            Some(Err(Refused::Stopped)) | None => ENS_AUDIO_ERROR,
        }
    }

    /// `event_clear_all`.
    #[no_mangle]
    extern "C" fn espeak_rs_event_clear_all() -> c_int {
        let delivery = DELIVERY.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(delivery) = delivery.as_ref() {
            delivery.clear();
        }
        ENS_OK
    }

    /// `event_terminate`: stops the delivery thread.
    #[no_mangle]
    extern "C" fn espeak_rs_event_terminate() {
        let delivery = DELIVERY.lock().unwrap_or_else(|p| p.into_inner()).take();
        drop(delivery);
    }

    /// Events queued and not yet delivered (for tests).
    #[no_mangle]
    extern "C" fn espeak_rs_event_pending() -> c_int {
        with(|delivery| delivery.pending() as c_int).unwrap_or(0)
    }

    const _: () = assert!(std::mem::size_of::<*mut c_void>() == std::mem::size_of::<usize>());
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn recorder() -> (Log, Callback) {
        let log: Log = Arc::default();
        let sink = Arc::clone(&log);
        let callback: Callback = Box::new(move |list: &[Event; 2]| {
            assert_eq!(list[1].kind, LIST_TERMINATED);
            sink.lock().unwrap().push((
                list[0].kind,
                list[0].unique_identifier,
                list[0].text_position,
                Instant::now(),
                std::thread::current().id(),
            ));
        });
        (log, callback)
    }

    /// Waits until `done` holds (the last event is popped before its
    /// callback runs, so the queue emptying is not enough).
    fn wait_until(done: impl Fn() -> bool) {
        let start = Instant::now();
        while !done() && start.elapsed() < Duration::from_secs(5) {
            std::thread::yield_now();
        }
    }

    #[test]
    fn events_arrive_in_order_at_their_time_on_the_delivery_thread() {
        let (log, callback) = recorder();
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
        wait_until(|| log.lock().unwrap().len() == 4);
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
        let (log, callback) = recorder();
        let delivery = Delivery::new(Some(callback)).unwrap();
        delivery.declare(event(WORD, 7, 1), Duration::ZERO).unwrap();
        delivery.declare(event(WORD, 7, 2), Duration::ZERO).unwrap();
        delivery.declare(event(PLAY, 7, 3), Duration::ZERO).unwrap();
        delivery.declare(event(END, 8, 4), Duration::ZERO).unwrap();
        wait_until(|| log.lock().unwrap().len() == 5);
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
        let delivery = Delivery::new(Some(Box::new(move |list: &[Event; 2]| {
            if list[0].kind == MARK {
                let pointer = usize::from_ne_bytes(list[0].id) as *const std::ffi::c_char;
                // SAFETY: the delivered mark's name, live for the call.
                let name = unsafe { std::ffi::CStr::from_ptr(pointer) };
                *sink.lock().unwrap() = name.to_string_lossy().into_owned();
            }
        })))
        .unwrap();
        let mut mark = event(MARK, 0, 1);
        mark.name = Some(CString::new("here").unwrap());
        delivery.declare(mark, Duration::ZERO).unwrap();
        wait_until(|| !seen.lock().unwrap().is_empty());
        assert_eq!(*seen.lock().unwrap(), "here");
    }

    #[test]
    fn clear_drops_pending_but_reports_terminations() {
        let (log, callback) = recorder();
        let delivery = Delivery::new(Some(callback)).unwrap();
        for i in 0..3 {
            delivery
                .declare(event(WORD, 3, i), Duration::from_secs(30))
                .unwrap();
        }
        delivery
            .declare(event(MSG_TERMINATED, 3, 9), Duration::from_secs(30))
            .unwrap();
        delivery.clear();
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
