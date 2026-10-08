// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use loadngo_proactor::{CompletionEnvelope, PollEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

type Action = Box<dyn FnOnce() + Send>;
struct Port {
    pending: Mutex<VecDeque<CompletionEnvelope>>,
    actions: Mutex<VecDeque<Action>>,
    posts: AtomicUsize,
    polls: AtomicUsize,
    fail_post: usize,
    fail_poll: usize,
}
impl CompletionPort for Port {
    fn post(&self, envelope: CompletionEnvelope) -> io::Result<()> {
        self.pending.lock().unwrap().push_back(envelope);
        if self.posts.fetch_add(1, Ordering::Relaxed) + 1 == self.fail_post {
            return Err(io::Error::other("injected post failure after queuing"));
        }
        Ok(())
    }
    fn poll(&self, _: Option<Duration>) -> io::Result<PollEvent> {
        let poll = self.polls.fetch_add(1, Ordering::Relaxed) + 1;
        if poll == self.fail_poll || poll > 16 {
            return Err(io::Error::other(
                "injected poll failure / excessive synthesis passes",
            ));
        }
        if let Some(envelope) = self.pending.lock().unwrap().pop_front() {
            return Ok(PollEvent::Completion(envelope));
        }
        let action = self.actions.lock().unwrap().pop_front();
        if let Some(action) = action {
            action();
        }
        self.pending
            .lock()
            .unwrap()
            .pop_front()
            .map(PollEvent::Completion)
            .ok_or_else(|| io::Error::other("pending wait lost its idle completion"))
    }
    fn wake(&self) -> io::Result<()> {
        Ok(())
    }
}
fn host(actions: Vec<Action>, fail_post: usize, fail_poll: usize) -> Proactor<Port> {
    Proactor::new(Port {
        pending: Mutex::new(VecDeque::new()),
        actions: Mutex::new(actions.into()),
        posts: AtomicUsize::new(0),
        polls: AtomicUsize::new(0),
        fail_post,
        fail_poll,
    })
}
struct Gated {
    gates: Mutex<VecDeque<mpsc::Receiver<()>>>,
    events: Mutex<Vec<(char, usize)>>,
    panic: bool,
}
impl Runner for Arc<Gated> {
    fn process(&self, command: usize) {
        self.events.lock().unwrap().push(('p', command));
        let gate = self.gates.lock().unwrap().pop_front().unwrap();
        gate.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(!self.panic, "injected command unwind");
    }
    fn delete(&self, command: usize) {
        self.events.lock().unwrap().push(('d', command));
    }
    fn is_setting(&self, _: usize) -> bool {
        false
    }
    fn cancel_audio(&self) {}
}
type TestQueue = Queue<Arc<Gated>>;
fn blocked(panic: bool) -> (Arc<TestQueue>, Arc<Gated>, mpsc::Sender<()>) {
    let (release, gate) = mpsc::channel();
    let runner = Arc::new(Gated {
        gates: Mutex::new(VecDeque::from([gate])),
        events: Mutex::new(Vec::new()),
        panic,
    });
    let queue = Arc::new(Queue::new(Arc::clone(&runner)).unwrap());
    queue.add(&[99]).unwrap();
    (queue, runner, release)
}
fn after_drain(queue: &TestQueue) -> mpsc::Receiver<()> {
    let (done, completed) = mpsc::channel();
    queue
        .handle
        .enqueue_work(move |_| done.send(()).unwrap())
        .unwrap();
    completed
}

#[test]
fn idle_completion_rechecks_commands_admitted_before_the_caller_resumes() {
    let (queue, runner, first) = blocked(false);
    let first_idle = after_drain(&queue);
    let (second, gate) = mpsc::channel();
    let again = Arc::clone(&queue);
    let first_runner = Arc::clone(&runner);
    let second_idle_queue = Arc::clone(&queue);
    // Two controlled Pending waits: the first idle notification is queued,
    // then another command starts before that caller processes it.
    let (marker, marker_receiver) = mpsc::channel::<mpsc::Receiver<()>>();
    let host = host(
        vec![
            Box::new(move || {
                first.send(()).unwrap();
                first_idle.recv_timeout(Duration::from_secs(2)).unwrap();
                first_runner.gates.lock().unwrap().push_back(gate);
                again.add(&[100]).unwrap();
                marker.send(after_drain(&second_idle_queue)).unwrap();
            }),
            Box::new(move || {
                second.send(()).unwrap();
                marker_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap();
            }),
        ],
        0,
        0,
    );
    queue.synchronize_on(&host).unwrap();
    assert_eq!(
        *runner.events.lock().unwrap(),
        [('p', 99), ('d', 99), ('p', 100), ('d', 100)]
    );
    assert!(queue.lock().waiters.iter().all(Option::is_none));
    // Idle has no need to initialize or post to the supplied failing port.
    queue.synchronize_on(&self::host(vec![], 1, 1)).unwrap();
}

#[test]
fn failed_posts_and_polls_release_slots_and_fence_old_wait_callbacks() {
    for (post, poll) in [(1, 0), (0, 2)] {
        let (queue, _, release) = blocked(false);
        let idle = after_drain(&queue);
        let host = host(
            vec![Box::new(move || {
                release.send(()).unwrap();
                idle.recv_timeout(Duration::from_secs(2)).unwrap();
            })],
            post,
            poll,
        );
        assert!(queue.synchronize_on(&host).is_err());
        assert!(queue.lock().waiters.iter().all(Option::is_none));
        // The reused port and slot may see a queued envelope from the failed
        // run first; that callback cannot register its old reservation again.
        queue.synchronize_on(&host).unwrap();
        assert!(queue.lock().waiters.iter().all(Option::is_none));
    }
}

#[test]
fn stop_releases_idle_waiters_after_the_running_command_and_queue_are_discarded() {
    let (queue, runner, release) = blocked(false);
    queue.lock().queue.push_back(100);
    let stopping = Arc::clone(&queue);
    let host = host(
        vec![Box::new(move || {
            let requester = Arc::clone(&stopping);
            let thread = std::thread::spawn(move || requester.stop());
            let (state, _) = stopping
                .shared
                .changed
                .wait_timeout_while(stopping.lock(), Duration::from_secs(2), |state| !state.stop)
                .unwrap();
            assert!(state.stop);
            drop(state);
            release.send(()).unwrap();
            thread.join().unwrap();
        })],
        0,
        0,
    );
    queue.synchronize_on(&host).unwrap();
    assert_eq!(
        *runner.events.lock().unwrap(),
        [('p', 99), ('d', 99), ('d', 100)]
    );
}

#[test]
fn worker_stop_and_command_unwind_interrupt_waiters_and_delete_commands_once() {
    for mode in 0..3 {
        let panic = mode == 1;
        let (queue, runner, release) = blocked(panic);
        queue.lock().queue.push_back(100);
        let dying = Arc::clone(&queue);
        let host = host(
            vec![Box::new(move || {
                if mode == 2 {
                    let requester = Arc::clone(&dying);
                    let terminator = std::thread::spawn(move || requester.terminate());
                    let (state, _) = dying
                        .shared
                        .changed
                        .wait_timeout_while(dying.lock(), Duration::from_secs(2), |state| {
                            !state.terminate
                        })
                        .unwrap();
                    assert!(state.terminate);
                    drop(state);
                    release.send(()).unwrap();
                    terminator.join().unwrap();
                    return;
                }
                if !panic {
                    dying.handle.stop().unwrap();
                }
                release.send(()).unwrap();
                let worker = dying.worker.lock().unwrap().take().unwrap();
                let result = worker.join();
                assert_eq!(result.is_err(), panic);
            })],
            0,
            0,
        );
        assert_eq!(
            queue.synchronize_on(&host).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        assert!(queue.lock().waiters.iter().all(Option::is_none));
        assert_eq!(queue.add(&[101, 102]), Err(AddError::Stopped));
        assert!(!queue.lock().queue.contains(&101));
        queue.terminate();
        assert_eq!(
            *runner.events.lock().unwrap(),
            [('p', 99), ('d', 99), ('d', 100)]
        );
    }
}

#[test]
fn concurrent_waiters_are_bounded_and_all_receive_one_idle_completion() {
    let (queue, _, release) = blocked(false);
    let idle = after_drain(&queue);
    let (entered, waiting) = mpsc::channel();
    let mut owners = Vec::new();
    let mut resume = Vec::new();
    for _ in 0..MAX_SYNC_WAITERS {
        let (send, gate) = mpsc::channel();
        resume.push(send);
        let entered = entered.clone();
        let queue = Arc::clone(&queue);
        owners.push(std::thread::spawn(move || {
            let host = host(
                vec![Box::new(move || {
                    entered.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(10)).unwrap();
                })],
                0,
                0,
            );
            queue.synchronize_on(&host).unwrap();
        }));
    }
    for _ in 0..MAX_SYNC_WAITERS {
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    assert_eq!(
        queue
            .synchronize_on(&host(vec![], 0, 0))
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    release.send(()).unwrap();
    idle.recv_timeout(Duration::from_secs(2)).unwrap();
    for send in resume {
        send.send(()).unwrap();
    }
    for owner in owners {
        owner.join().unwrap();
    }
    assert!(queue.lock().waiters.iter().all(Option::is_none));
}

#[test]
fn queued_jobs_and_inactivity_notices_do_not_retain_their_own_port_or_owner() {
    let (queue, _, release) = blocked(false);
    let weak = Arc::downgrade(&queue.shared);
    release.send(()).unwrap();
    queue.synchronize().unwrap();
    queue.terminate();
    drop(queue);
    assert!(weak.upgrade().is_none());
}

#[test]
fn stop_includes_commands_a_kept_setting_queues_before_it_can_publish_idle() {
    struct Settings {
        queue: Mutex<Option<Arc<Queue<Arc<Settings>>>>>,
        events: Mutex<Vec<(char, usize)>>,
        gate: Mutex<mpsc::Receiver<()>>,
        panic: bool,
    }
    impl Runner for Arc<Settings> {
        fn process(&self, command: usize) {
            self.events.lock().unwrap().push(('p', command));
            if command == 99 {
                self.gate
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            } else if command == 1000 {
                assert!(!self.panic, "injected kept-setting unwind");
                self.queue
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .add(&[7, 1001])
                    .unwrap();
            }
        }
        fn delete(&self, command: usize) {
            self.events.lock().unwrap().push(('d', command));
        }
        fn is_setting(&self, command: usize) -> bool {
            command >= 1000
        }
        fn cancel_audio(&self) {}
    }
    for panic in [false, true] {
        let (release, gate) = mpsc::channel();
        let runner = Arc::new(Settings {
            queue: Mutex::new(None),
            events: Mutex::new(Vec::new()),
            gate: Mutex::new(gate),
            panic,
        });
        let queue = Arc::new(Queue::new(Arc::clone(&runner)).unwrap());
        *runner.queue.lock().unwrap() = Some(Arc::clone(&queue));
        queue.add(&[99, 1000, 1002]).unwrap();
        let stopping = Arc::clone(&queue);
        let thread = std::thread::spawn(move || stopping.stop());
        let (state, _) = queue
            .shared
            .changed
            .wait_timeout_while(queue.lock(), Duration::from_secs(2), |state| !state.stop)
            .unwrap();
        assert!(state.stop);
        drop(state);
        release.send(()).unwrap();
        thread.join().unwrap();
        let result = queue.synchronize();
        if panic {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        } else {
            result.unwrap();
        }
        let empty = queue.lock().queue.is_empty();
        runner.queue.lock().unwrap().take();
        queue.terminate();
        let events = runner.events.lock().unwrap().clone();
        if panic {
            assert!(!empty, "unstarted commands were lost on unwind");
            assert_eq!(
                events,
                [('p', 99), ('d', 99), ('p', 1000), ('d', 1000), ('d', 1002)]
            );
            continue;
        }
        assert!(empty, "idle with stranded commands");
        assert_eq!(
            events,
            [
                ('p', 99),
                ('d', 99),
                ('p', 1000),
                ('d', 1000),
                ('p', 1002),
                ('d', 1002),
                ('d', 7),
                ('p', 1001),
                ('d', 1001)
            ]
        );
    }
}
