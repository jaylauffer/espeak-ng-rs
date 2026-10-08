//! The asynchronous API's command queue (`fifo.c`) on a loadngo proactor.
//!
//! Commands are opaque to Rust and are run by the owner's callback. One
//! worker thread runs the proactor loop. Adding commands posts a drain job
//! to it, and the job runs them in order. After the queue empties, a
//! proactor timer stands in for C's inactivity wait, which was three timed
//! condition waits of 50 ms. Stop and terminate are a flag plus the
//! proactor's `stop`; no thread sleeps or polls.
// SPDX-License-Identifier: GPL-3.0-or-later
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
    /// Commands pushed and taken, so an adder can wait for its own.
    pushed: u64,
    taken: u64,
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
}

type Handle = ProactorHandle<loadngo_proactor::PlatformPort>;

pub struct Queue<R: Runner> {
    shared: Arc<Shared<R>>,
    handle: Handle,
    worker: Option<JoinHandle<io::Result<()>>>,
}

impl<R: Runner> Queue<R> {
    /// Starts the worker thread and its proactor.
    pub fn new(runner: R) -> io::Result<Self> {
        let proactor = new_platform_proactor()?;
        let handle = proactor.handle();
        let worker = std::thread::Builder::new()
            .name("espeak-fifo".into())
            .spawn(move || proactor.run_until_stopped())?;
        Ok(Self {
            shared: Arc::new(Shared {
                runner,
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
            }),
            handle,
            worker: Some(worker),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.shared.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `fifo_add_command`/`fifo_add_commands`: queues the commands (both or
    /// neither) and waits until the worker has taken the last of them, as C
    /// waited for its command to be running.
    pub fn add(&self, commands: &[usize]) -> Result<(), AddError> {
        if commands.is_empty() || commands.contains(&0) {
            return Err(AddError::Invalid);
        }
        let mut state = self.lock();
        if state.queue.len() + commands.len() > MAX_COMMANDS {
            return Err(AddError::Full);
        }
        state.queue.extend(commands);
        state.pushed += commands.len() as u64;
        let target = state.pushed;
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
        while state.taken < target && !state.terminate {
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
        while !state.acknowledged && !state.terminate {
            state = self
                .shared
                .changed
                .wait(state)
                .unwrap_or_else(|p| p.into_inner());
        }
        state.stop = false;
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

impl<R: Runner> Drop for Queue<R> {
    /// `fifo_terminate`: stops the worker and deletes what is left unrun.
    fn drop(&mut self) {
        {
            let mut state = self.lock();
            state.terminate = true;
            self.shared.changed.notify_all();
        }
        let _ = self.handle.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let rest: Vec<usize> = self.lock().queue.drain(..).collect();
        for command in rest {
            self.shared.runner.delete(command);
        }
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
        state.taken += 1;
        shared.changed.notify_all();
        let stopping = state.stop;
        drop(state);
        if !stopping {
            shared.runner.process(command);
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
        state.taken += rest.len() as u64;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

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
