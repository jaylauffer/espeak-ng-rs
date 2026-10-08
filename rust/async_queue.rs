//! The asynchronous API's command queue (`fifo.c`) on a loadngo proactor.
//!
//! Commands are opaque to Rust and are run by the owner's callback. One
//! worker thread runs the proactor loop. Adding commands posts a drain job
//! to it, and the job runs them in order. After the queue empties, a
//! proactor timer stands in for C's inactivity wait, which was three timed
//! condition waits of 50 ms. Stop and terminate are a flag plus the
//! proactor's `stop`; no thread sleeps or polls.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::synthesis_loop::{with_cancellation, Cancellation};
use loadngo_proactor::{new_platform_proactor, CompletionKind, ProactorHandle};
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// C's `MAX_NODE_COUNTER`.
pub const MAX_COMMANDS: usize = 400;
/// C's inactivity wait: three checks of 50 ms.
pub const INACTIVITY: Duration = Duration::from_millis(150);

/// What the owner does with a command, and the audio it cancels.
pub trait Runner: Send + Sync + 'static {
    fn process(&self, command: usize);
    fn delete(&self, command: usize);
    /// Parameter and voice commands still run when a stop discards the rest.
    fn is_setting(&self, command: usize) -> bool;
    fn cancel_audio(&self);
}

#[derive(Debug, Eq, PartialEq)]
pub enum AddError {
    Full,
    Invalid,
    Stopped,
}

#[derive(Default)]
struct State {
    queue: VecDeque<usize>,
    running: bool,
    draining: bool,
    stop: bool,
    acknowledged: bool,
    terminate: bool,
    /// Bumped by each drain, so an older inactivity timer is ignored.
    epoch: u64,
}

struct Shared<R> {
    runner: R,
    state: Mutex<State>,
    changed: Condvar,
    cancellation: Arc<Cancellation>,
}

type Handle = ProactorHandle<loadngo_proactor::PlatformPort>;

pub struct Queue<R: Runner> {
    shared: Arc<Shared<R>>,
    handle: Handle,
    worker: Mutex<Option<JoinHandle<io::Result<()>>>>,
    worker_thread: std::thread::ThreadId,
}

impl<R: Runner> Queue<R> {
    /// Starts the worker thread and its proactor.
    pub fn new(runner: R) -> io::Result<Self> {
        let proactor = new_platform_proactor()?;
        let handle = proactor.handle();
        let worker = std::thread::Builder::new()
            .name("espeak-fifo".into())
            .spawn(move || proactor.run_until_stopped())?;
        let worker_thread = worker.thread().id();
        Ok(Self {
            worker_thread,
            shared: Arc::new(Shared {
                runner,
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
                cancellation: Arc::new(Cancellation::default()),
            }),
            handle,
            worker: Mutex::new(Some(worker)),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.shared.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `fifo_add_command`/`fifo_add_commands`: queues the commands (both or
    /// neither) and waits only for an idle worker to start draining, as C
    /// did. A running worker already acknowledges admission; waiting for a
    /// text's following terminated-message command would make speech
    /// submission synchronous and prevent its caller from cancelling it.
    /// A command a running command
    /// queues (SSML changing a parameter) is added from the worker itself,
    /// which does not wait: the worker is already running.
    pub fn add(&self, commands: &[usize]) -> Result<(), AddError> {
        if commands.is_empty() || commands.contains(&0) {
            return Err(AddError::Invalid);
        }
        let mut state = self.lock();
        if state.queue.len() + commands.len() > MAX_COMMANDS {
            return Err(AddError::Full);
        }
        state.queue.extend(commands);
        if !state.draining {
            state.draining = true;
            let shared = Arc::clone(&self.shared);
            let handle = self.handle.clone();
            if self
                .handle
                .enqueue_work(move |_| drain(shared, handle))
                .is_err()
            {
                state.draining = false;
                return Err(AddError::Stopped);
            }
        }
        let on_worker = std::thread::current().id() == self.worker_thread;
        while !on_worker && state.draining && !state.running && !state.terminate {
            state = self
                .shared
                .changed
                .wait(state)
                .unwrap_or_else(|p| p.into_inner());
        }
        Ok(())
    }

    /// `fifo_stop`: when a command is running, discards the queue (running
    /// parameter and voice commands) and waits for the acknowledgment.
    pub fn stop(&self) {
        let mut state = self.lock();
        if !(state.running || state.draining) {
            return;
        }
        state.stop = true;
        state.acknowledged = false;
        // This reaches the command's nested synthesis port directly. Posting
        // only to this queue's port cannot wake that command's Pending wait.
        self.shared.cancellation.request();
        while !state.acknowledged && !state.terminate {
            state = self
                .shared
                .changed
                .wait(state)
                .unwrap_or_else(|p| p.into_inner());
        }
        if !state.terminate {
            state.stop = false;
            self.shared.cancellation.reset();
        }
    }

    /// `fifo_is_busy`.
    pub fn is_busy(&self) -> bool {
        let state = self.lock();
        state.running || state.draining
    }

    /// `fifo_is_command_enabled`.
    pub fn is_command_enabled(&self) -> bool {
        !self.lock().stop
    }
}

impl<R: Runner> Queue<R> {
    /// `fifo_terminate`: stops the worker and deletes what is left unrun.
    pub fn terminate(&self) {
        {
            let mut state = self.lock();
            state.terminate = true;
            self.shared.cancellation.request();
            self.shared.changed.notify_all();
        }
        let _ = self.handle.stop();
        let worker = self.worker.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        let rest: Vec<usize> = self.lock().queue.drain(..).collect();
        for command in rest {
            self.shared.runner.delete(command);
        }
    }
}

impl<R: Runner> Drop for Queue<R> {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// The worker's drain job: runs queued commands in order until the queue is
/// empty, a stop discards it, or the queue terminates.
fn drain<R: Runner>(shared: Arc<Shared<R>>, handle: Handle) {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    state.running = true;
    state.epoch += 1;
    loop {
        if state.terminate {
            break;
        }
        let Some(command) = state.queue.pop_front() else {
            break;
        };
        shared.changed.notify_all();
        let stopping = state.stop;
        drop(state);
        if !stopping {
            with_cancellation(Arc::clone(&shared.cancellation), || {
                shared.runner.process(command)
            });
        }
        shared.runner.delete(command);
        state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.stop {
            break;
        }
    }
    if state.stop && !state.terminate {
        // discard the rest, keeping settings, then acknowledge
        let rest: Vec<usize> = state.queue.drain(..).collect();
        drop(state);
        for command in rest {
            if shared.runner.is_setting(command) {
                shared.runner.process(command);
            }
            shared.runner.delete(command);
        }
        state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.acknowledged = true;
    }
    state.running = false;
    state.draining = false;
    let epoch = state.epoch;
    shared.changed.notify_all();
    drop(state);
    // the inactivity wait, as a timer rather than timed condition waits
    let timer = Arc::clone(&shared);
    let _ = handle.defer_for(INACTIVITY, CompletionKind::Job, 0, move |_| {
        inactive(timer, epoch)
    });
}

/// No command started during the inactivity wait (C's `close_stream`): a
/// stop that arrived meanwhile cancels the audio early and is acknowledged.
fn inactive<R: Runner>(shared: Arc<Shared<R>>, epoch: u64) {
    let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    if state.epoch != epoch || state.draining || state.terminate {
        return; // a later command started
    }
    if state.stop && !state.acknowledged {
        drop(state);
        shared.runner.cancel_audio();
        state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.acknowledged = true;
        shared.changed.notify_all();
    }
}

/// The C API (`fifo.c` in proactor builds). The queue is shared, never
/// locked across a call: the worker calls back into `fifo_is_command_enabled`
/// while `fifo_stop` waits.
#[cfg(feature = "c-abi")]
mod c_api {
    use super::{AddError, Queue, Runner};
    use std::ffi::c_void;
    use std::sync::{Arc, Mutex};

    type Command = unsafe extern "C" fn(*mut c_void);
    type IsSetting = unsafe extern "C" fn(*mut c_void) -> i32;
    type Cancel = unsafe extern "C" fn();

    /// `RustFifoCallbacks`.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Callbacks {
        process: Option<Command>,
        delete: Option<Command>,
        is_setting: Option<IsSetting>,
        cancel_audio: Option<Cancel>,
    }

    struct CRunner(Callbacks);
    // SAFETY: the callbacks are plain C functions over commands the queue
    // owns until it deletes them; the owner serializes the engine on the
    // worker, as the C fifo did.
    unsafe impl Send for CRunner {}
    // SAFETY: as above.
    unsafe impl Sync for CRunner {}
    impl Runner for CRunner {
        fn process(&self, command: usize) {
            if let Some(process) = self.0.process {
                // SAFETY: a command the owner queued and has not deleted.
                unsafe { process(command as *mut c_void) }
            }
        }
        fn delete(&self, command: usize) {
            if let Some(delete) = self.0.delete {
                // SAFETY: as above; deleted once.
                unsafe { delete(command as *mut c_void) }
            }
        }
        fn is_setting(&self, command: usize) -> bool {
            // SAFETY: as above.
            self.0
                .is_setting
                .is_some_and(|is_setting| unsafe { is_setting(command as *mut c_void) } != 0)
        }
        fn cancel_audio(&self) {
            if let Some(cancel) = self.0.cancel_audio {
                // SAFETY: the owner's audio cancel.
                unsafe { cancel() }
            }
        }
    }

    static QUEUE: Mutex<Option<Arc<Queue<CRunner>>>> = Mutex::new(None);

    fn queue() -> Option<Arc<Queue<CRunner>>> {
        QUEUE.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// `fifo_init`: 0, or -1 when the proactor or worker cannot start.
    #[no_mangle]
    extern "C" fn espeak_rs_fifo_init(callbacks: Callbacks) -> i32 {
        let mut slot = QUEUE.lock().unwrap_or_else(|p| p.into_inner());
        if slot.is_some() {
            return 0;
        }
        match Queue::new(CRunner(callbacks)) {
            Ok(queue) => {
                *slot = Some(Arc::new(queue));
                0
            }
            Err(_) => -1,
        }
    }

    /// `fifo_add_command` (`second` null) or `fifo_add_commands`: 0, 1 full,
    /// 2 invalid, 3 not running.
    #[no_mangle]
    extern "C" fn espeak_rs_fifo_add(first: *mut c_void, second: *mut c_void) -> i32 {
        let Some(queue) = queue() else {
            return 3;
        };
        let result = if second.is_null() {
            queue.add(&[first as usize])
        } else {
            queue.add(&[first as usize, second as usize])
        };
        match result {
            Ok(()) => 0,
            Err(AddError::Full) => 1,
            Err(AddError::Invalid) => 2,
            Err(AddError::Stopped) => 3,
        }
    }

    #[no_mangle]
    extern "C" fn espeak_rs_fifo_stop() {
        if let Some(queue) = queue() {
            queue.stop();
        }
    }

    #[no_mangle]
    extern "C" fn espeak_rs_fifo_is_busy() -> i32 {
        queue().is_some_and(|queue| queue.is_busy()) as i32
    }

    #[no_mangle]
    extern "C" fn espeak_rs_fifo_is_command_enabled() -> i32 {
        queue().is_none_or(|queue| queue.is_command_enabled()) as i32
    }

    /// `fifo_terminate`: stops the worker, waits for it, and deletes the rest.
    #[no_mangle]
    extern "C" fn espeak_rs_fifo_terminate() {
        let taken = QUEUE.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(queue) = taken {
            // wakes calls waiting on other threads; they then return
            queue.terminate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    #[test]
    fn stop_waiter_cannot_clear_termination_before_synthesis_registers() {
        struct Delayed {
            entered: mpsc::Sender<()>,
            release: Mutex<mpsc::Receiver<()>>,
            outcome: Mutex<Option<io::ErrorKind>>,
        }
        impl Runner for Arc<Delayed> {
            fn process(&self, _: usize) {
                self.entered.send(()).unwrap();
                self.release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                let result = crate::synthesis_loop::run_with(|_| {
                    panic!("termination was cleared by the stop waiter")
                });
                *self.outcome.lock().unwrap() = Some(result.unwrap_err().kind());
            }
            fn delete(&self, _: usize) {}
            fn is_setting(&self, _: usize) -> bool {
                false
            }
            fn cancel_audio(&self) {}
        }
        let (entered, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let runner = Arc::new(Delayed {
            entered,
            release: Mutex::new(gate),
            outcome: Mutex::new(None),
        });
        let queue = Arc::new(Queue::new(Arc::clone(&runner)).unwrap());
        queue.add(&[1]).unwrap();
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        let stopping = Arc::clone(&queue);
        let stopper = std::thread::spawn(move || stopping.stop());
        // One bounded condition wait, no spin or repeated sleep. Stop need
        // not notify this observer; the fixed deadline also checks state.
        let (state, _) = queue
            .shared
            .changed
            .wait_timeout_while(queue.lock(), Duration::from_secs(2), |state| !state.stop)
            .unwrap();
        assert!(state.stop);
        drop(state);
        let terminating = Arc::clone(&queue);
        let terminator = std::thread::spawn(move || terminating.terminate());
        // Termination releases the stop waiter's condition before this
        // command reaches its first synthesis pass / wake registration.
        stopper.join().unwrap();
        release.send(()).unwrap();
        terminator.join().unwrap();
        assert_eq!(
            *runner.outcome.lock().unwrap(),
            Some(io::ErrorKind::Interrupted)
        );
    }

    #[test]
    fn stop_and_terminate_wake_pending_synthesis_on_the_commands_port() {
        struct Pending {
            entered: mpsc::Sender<()>,
            deleted: mpsc::Sender<usize>,
            outcomes: Mutex<Vec<(usize, Option<io::ErrorKind>)>>,
            watchdog_fired: Arc<AtomicBool>,
        }
        impl Runner for Arc<Pending> {
            fn process(&self, command: usize) {
                let entered = self.entered.clone();
                let fired = Arc::clone(&self.watchdog_fired);
                // A test-only failure deadline. Clearing the handle after
                // the run also breaks the timer's potential port cycle.
                let watchdog = Arc::new(Mutex::new(None::<Handle>));
                let timer = Arc::clone(&watchdog);
                let mut passes = 0;
                let result = crate::synthesis_loop::run_with(move |wake| {
                    passes += 1;
                    assert_eq!(passes, 1, "pending synthesis was replayed");
                    if command == 2 {
                        return crate::synthesis_loop::Step::Done;
                    }
                    let handle = wake.handle().unwrap();
                    *timer.lock().unwrap() = Some(handle.clone());
                    let timer = Arc::clone(&timer);
                    let fired = Arc::clone(&fired);
                    handle
                        .defer_for(
                            Duration::from_secs(10),
                            CompletionKind::Timer,
                            0,
                            move |_| {
                                fired.store(true, Ordering::Relaxed);
                                if let Some(handle) = timer.lock().unwrap().as_ref() {
                                    handle.stop().unwrap();
                                }
                            },
                        )
                        .unwrap();
                    let entered = entered.clone();
                    handle
                        .enqueue_work(move |_| entered.send(()).unwrap())
                        .unwrap();
                    crate::synthesis_loop::Step::Pending
                });
                watchdog.lock().unwrap().take();
                self.outcomes
                    .lock()
                    .unwrap()
                    .push((command, result.err().map(|e| e.kind())));
            }
            fn delete(&self, command: usize) {
                self.deleted.send(command).unwrap();
            }
            fn is_setting(&self, _: usize) -> bool {
                false
            }
            fn cancel_audio(&self) {}
        }
        for terminate in [false, true] {
            let (entered, waiting) = mpsc::channel();
            let (deleted, finished) = mpsc::channel();
            let runner = Arc::new(Pending {
                entered,
                deleted,
                outcomes: Mutex::new(Vec::new()),
                watchdog_fired: Arc::new(AtomicBool::new(false)),
            });
            let queue = Queue::new(Arc::clone(&runner)).unwrap();
            // Like the C API's text + terminated-message transaction, the
            // second command cannot be taken until speech finishes. Add
            // must still return while the first command remains Pending.
            queue.add(&[1, 3]).unwrap();
            // This is delivered as a completion after the pass entered
            // Pending, rather than a signal emitted inside the pass.
            waiting.recv_timeout(Duration::from_secs(2)).unwrap();
            if terminate {
                queue.terminate();
            } else {
                queue.stop();
            }
            assert!(!runner.watchdog_fired.load(Ordering::Relaxed));
            assert_eq!(
                *runner.outcomes.lock().unwrap(),
                [(1, Some(io::ErrorKind::Interrupted))]
            );
            assert_eq!(finished.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
            assert_eq!(finished.recv_timeout(Duration::from_secs(2)).unwrap(), 3);
            if !terminate {
                // Stop must reset the scope before admitting later speech.
                queue.add(&[2]).unwrap();
                assert_eq!(finished.recv_timeout(Duration::from_secs(2)).unwrap(), 2);
                queue.terminate();
                assert_eq!(
                    *runner.outcomes.lock().unwrap(),
                    [(1, Some(io::ErrorKind::Interrupted)), (2, None)]
                );
            }
        }
    }

    struct Log {
        events: Mutex<Vec<(char, usize)>>,
        gate: Mutex<Option<mpsc::Receiver<()>>>,
    }
    impl Runner for Arc<Log> {
        fn process(&self, command: usize) {
            self.events.lock().unwrap().push(('p', command));
            if command == 99 {
                // hold the worker until the test releases it
                if let Some(gate) = self.gate.lock().unwrap().take() {
                    gate.recv().unwrap();
                }
            }
        }
        fn delete(&self, command: usize) {
            self.events.lock().unwrap().push(('d', command));
        }
        fn is_setting(&self, command: usize) -> bool {
            command >= 1000
        }
        fn cancel_audio(&self) {
            self.events.lock().unwrap().push(('c', 0));
        }
    }

    fn log() -> Arc<Log> {
        Arc::new(Log {
            events: Mutex::new(Vec::new()),
            gate: Mutex::new(None),
        })
    }

    #[test]
    fn commands_run_in_order_on_the_worker() {
        let log = log();
        let queue = Queue::new(Arc::clone(&log)).unwrap();
        queue.add(&[1]).unwrap();
        queue.add(&[2, 3]).unwrap();
        assert_eq!(queue.add(&[]), Err(AddError::Invalid));
        assert_eq!(queue.add(&[0]), Err(AddError::Invalid));
        while queue.is_busy() {
            std::thread::yield_now();
        }
        assert_eq!(
            *log.events.lock().unwrap(),
            [('p', 1), ('d', 1), ('p', 2), ('d', 2), ('p', 3), ('d', 3)]
        );
        assert!(queue.is_command_enabled());
    }

    /// Holds the worker on command 99, queues `behind` without waiting,
    /// stops from another thread, then releases the worker.
    fn stop_while_running(behind: &[usize]) -> Vec<(char, usize)> {
        let log = log();
        let (release, gate) = mpsc::channel();
        *log.gate.lock().unwrap() = Some(gate);
        let queue = Arc::new(Queue::new(Arc::clone(&log)).unwrap());
        queue.add(&[99]).unwrap();
        queue.lock().queue.extend(behind);
        let stopper = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || queue.stop())
        };
        while queue.is_command_enabled() {
            std::thread::yield_now();
        }
        release.send(()).unwrap();
        stopper.join().unwrap();
        assert!(queue.is_command_enabled());
        let events = log.events.lock().unwrap().clone();
        events
    }

    #[test]
    fn stop_discards_the_queue_but_keeps_settings() {
        let events = stop_while_running(&[5, 1000, 6]);
        assert_eq!(
            events,
            [
                ('p', 99),
                ('d', 99),
                ('d', 5),
                ('p', 1000),
                ('d', 1000),
                ('d', 6)
            ]
        );
    }

    #[test]
    fn a_running_command_can_queue_another() {
        struct Reentrant {
            queue: Mutex<Option<Arc<Queue<Arc<Reentrant>>>>>,
            seen: Mutex<Vec<usize>>,
        }
        impl Runner for Arc<Reentrant> {
            fn process(&self, command: usize) {
                self.seen.lock().unwrap().push(command);
                if command == 1 {
                    let queue = self.queue.lock().unwrap().clone().unwrap();
                    queue.add(&[2]).unwrap(); // on the worker: must not wait
                }
            }
            fn delete(&self, _: usize) {}
            fn is_setting(&self, _: usize) -> bool {
                false
            }
            fn cancel_audio(&self) {}
        }
        let runner = Arc::new(Reentrant {
            queue: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
        });
        let queue = Arc::new(Queue::new(Arc::clone(&runner)).unwrap());
        *runner.queue.lock().unwrap() = Some(Arc::clone(&queue));
        queue.add(&[1]).unwrap();
        while queue.is_busy() {
            std::thread::yield_now();
        }
        assert_eq!(*runner.seen.lock().unwrap(), [1, 2]);
        queue.terminate();
        *runner.queue.lock().unwrap() = None;
    }

    #[test]
    fn full_queue_is_refused() {
        let log = log();
        let (release, gate) = mpsc::channel();
        *log.gate.lock().unwrap() = Some(gate);
        let queue = Queue::new(Arc::clone(&log)).unwrap();
        queue.add(&[99]).unwrap();
        queue.lock().queue.extend(1..=MAX_COMMANDS);
        assert_eq!(queue.add(&[7]), Err(AddError::Full));
        release.send(()).unwrap();
        drop(queue); // terminate: whatever was not run is deleted
        let events = log.events.lock().unwrap().clone();
        let deleted = events.iter().filter(|&&(kind, _)| kind == 'd').count();
        assert_eq!(deleted, 1 + MAX_COMMANDS);
    }
}
