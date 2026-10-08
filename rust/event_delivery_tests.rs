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
            return Err(io::Error::other("injected refusal after queuing"));
        }
        Ok(())
    }
    fn poll(&self, _: Option<Duration>) -> io::Result<PollEvent> {
        let poll = self.polls.fetch_add(1, Ordering::Relaxed) + 1;
        if poll == self.fail_poll || poll > 32 {
            return Err(io::Error::other("injected poll failure / excessive passes"));
        }
        if let Some(envelope) = self.pending.lock().unwrap().pop_front() {
            return Ok(PollEvent::Completion(envelope));
        }
        if let Some(action) = self.actions.lock().unwrap().pop_front() {
            action();
        }
        self.pending
            .lock()
            .unwrap()
            .pop_front()
            .map(PollEvent::Completion)
            .ok_or_else(|| io::Error::other("pending wait lost completion"))
    }
    fn wake(&self) -> io::Result<()> {
        Ok(())
    }
}
fn host(actions: Vec<Action>, fail_post: usize, fail_poll: usize) -> Proactor<Port> {
    Proactor::new(Port {
        pending: Mutex::default(),
        actions: Mutex::new(actions.into()),
        posts: AtomicUsize::new(0),
        polls: AtomicUsize::new(0),
        fail_post,
        fail_poll,
    })
}
fn event(uid: u32) -> Declared {
    Declared {
        event: Event {
            kind: WORD,
            unique_identifier: uid,
            ..Event::default()
        },
        name: None,
    }
}
fn full() -> Arc<Delivery> {
    let delivery = Arc::new(Delivery::new(None).unwrap());
    for _ in 0..MAX_EVENTS {
        delivery.declare(event(1), Duration::from_secs(30)).unwrap();
    }
    delivery
}
fn pop_one(delivery: &Delivery) {
    let shared = Arc::downgrade(&delivery.shared);
    let (done, completed) = mpsc::channel();
    delivery
        .handle
        .enqueue_work(move |_| {
            let shared = shared.upgrade().unwrap();
            shared.lock().pending.front_mut().unwrap().0 = Instant::now();
            shared.deliver_due();
            done.send(()).unwrap();
        })
        .unwrap();
    completed.recv_timeout(Duration::from_secs(5)).unwrap();
}
fn no_waiters(delivery: &Delivery) {
    assert!(delivery.shared.lock().waiters.iter().all(Option::is_none));
}

#[test]
fn mandatory_completion_survives_command_cancellation_and_clear() {
    use crate::synthesis_loop::{with_cancellation, Cancellation};
    let delivery = full();
    let (done, completed) = mpsc::channel();
    delivery.set_callback(Some(Box::new(move |events| {
        if events[0].kind == MSG_TERMINATED {
            done.send(events[0].unique_identifier).unwrap();
        }
    })));
    let scope = Arc::new(Cancellation::default());
    let cancel = scope.clone();
    let other = delivery.clone();
    let port = host(
        vec![Box::new(move || {
            cancel.request();
            other.clear().unwrap();
        })],
        0,
        0,
    );
    let mut completion = event(9);
    completion.event.kind = MSG_TERMINATED;
    with_cancellation(scope, || {
        delivery.declare_on(&port, completion, Duration::from_secs(30))
    })
    .unwrap();
    no_waiters(&delivery);
    pop_one(&delivery);
    assert_eq!(completed.recv_timeout(Duration::from_secs(5)).unwrap(), 9);
}

#[test]
fn caller_unwind_fences_pending_admission_and_releases_owner() {
    let delivery = full();
    let port = host(vec![Box::new(|| panic!("injected caller unwind"))], 0, 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        delivery.declare_on(&port, event(9), Duration::ZERO)
    }));
    assert!(result.is_err());
    no_waiters(&delivery);
    let weak = Arc::downgrade(&delivery.shared);
    drop(delivery);
    assert!(weak.upgrade().is_none());
}

#[test]
fn capacity_wake_rechecks_new_admission_and_retains_the_owned_event() {
    let delivery = full();
    let first = delivery.clone();
    let second = delivery.clone();
    let host = host(
        vec![
            Box::new(move || {
                pop_one(&first);
                first.declare(event(2), Duration::from_secs(30)).unwrap();
            }),
            Box::new(move || pop_one(&second)),
        ],
        0,
        0,
    );
    let mut mark = event(3);
    mark.event.kind = MARK;
    mark.name = Some(CString::new("owned while pending").unwrap());
    delivery.declare_on(&host, mark, Duration::ZERO).unwrap();
    let queue = delivery.shared.lock();
    assert_eq!(queue.pending.len(), MAX_EVENTS);
    assert_eq!(
        queue.pending.back().unwrap().1.name.as_deref(),
        Some(c"owned while pending")
    );
    drop(queue);
    no_waiters(&delivery);
}

#[test]
fn clear_and_command_cancellation_interrupt_full_admission_and_permit_reuse() {
    use crate::synthesis_loop::{with_cancellation, Cancellation};
    for clearing in [false, true] {
        let delivery = full();
        let scope = Arc::new(Cancellation::default());
        let other = delivery.clone();
        let cancel = scope.clone();
        let port = host(
            vec![Box::new(move || {
                if clearing {
                    other.clear().unwrap();
                } else {
                    cancel.request();
                }
            })],
            0,
            0,
        );
        let result = with_cancellation(scope.clone(), || {
            delivery.declare_on(&port, event(9), Duration::ZERO)
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        no_waiters(&delivery);
        assert!(delivery
            .shared
            .lock()
            .pending
            .iter()
            .all(|(_, e)| e.event.unique_identifier != 9));
        scope.reset();
        let other = delivery.clone();
        let port = host(
            if clearing {
                vec![]
            } else {
                vec![Box::new(move || pop_one(&other)) as Action]
            },
            0,
            0,
        );
        with_cancellation(scope, || {
            delivery.declare_on(&port, event(10), Duration::from_secs(30))
        })
        .unwrap();
    }
}

#[test]
fn failed_caller_post_and_poll_release_slots_and_fence_stale_jobs() {
    for (fail_post, fail_poll) in [(1, 0), (0, 1)] {
        let delivery = full();
        let other = delivery.clone();
        let port = host(
            vec![Box::new(move || pop_one(&other))],
            fail_post,
            fail_poll,
        );
        assert!(delivery
            .declare_on(&port, event(7), Duration::ZERO)
            .is_err());
        no_waiters(&delivery);
        delivery
            .declare_on(&port, event(8), Duration::from_secs(30))
            .unwrap();
        no_waiters(&delivery);
        assert!(delivery
            .shared
            .lock()
            .pending
            .iter()
            .all(|(_, e)| e.event.unique_identifier != 7));
    }
}

#[test]
fn worker_exit_releases_capacity_and_accepted_clear_waits() {
    for clearing in [false, true] {
        let delivery = full();
        let (entered, blocked) = mpsc::channel();
        let (release, released) = mpsc::channel();
        delivery
            .handle
            .enqueue_work(move |_| {
                entered.send(()).unwrap();
                released.recv_timeout(Duration::from_secs(5)).unwrap();
                panic!("injected worker unwind");
            })
            .unwrap();
        blocked.recv_timeout(Duration::from_secs(5)).unwrap();
        let other = delivery.clone();
        let port = host(
            vec![Box::new(move || {
                release.send(()).unwrap();
                other.terminate();
            })],
            0,
            0,
        );
        if clearing {
            delivery.clear_on(&port).unwrap();
            assert_eq!(delivery.pending(), 0);
        } else {
            assert_eq!(
                delivery
                    .declare_on(&port, event(9), Duration::ZERO)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::Interrupted
            );
        }
        no_waiters(&delivery);
        assert_eq!(
            delivery.declare(event(10), Duration::ZERO),
            Err(Refused::Stopped)
        );
    }
}

#[test]
fn failed_clear_cannot_execute_against_reused_owner_state() {
    let delivery = full();
    let (entered, blocked) = mpsc::channel();
    let (release, released) = mpsc::channel();
    delivery
        .handle
        .enqueue_work(move |_| {
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
        })
        .unwrap();
    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
    let port = host(vec![], 0, 2);
    assert!(delivery.clear_on(&port).is_err());
    no_waiters(&delivery);
    release.send(()).unwrap();
    let (done, drained) = mpsc::channel();
    delivery
        .handle
        .enqueue_work(move |_| done.send(()).unwrap())
        .unwrap();
    drained.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(delivery.pending(), MAX_EVENTS);
    delivery.clear().unwrap();
    assert_eq!(delivery.pending(), 0);
    assert!(delivery.shared.lock().pending.capacity() >= MAX_EVENTS);
}

#[test]
fn all_sixteen_waiters_wake_and_the_seventeenth_is_refused() {
    let delivery = full();
    let (ready, waiting) = mpsc::channel();
    let mut calls = Vec::new();
    let mut releases = Vec::new();
    for _ in 0..MAX_WAITERS {
        let owner = delivery.clone();
        let ready = ready.clone();
        let (release, released) = mpsc::channel();
        releases.push(release);
        calls.push(std::thread::spawn(move || {
            let port = host(
                vec![Box::new(move || {
                    ready.send(()).unwrap();
                    released.recv_timeout(Duration::from_secs(5)).unwrap();
                })],
                0,
                0,
            );
            owner
                .declare_on(&port, event(4), Duration::ZERO)
                .unwrap_err()
                .kind()
        }));
    }
    for _ in 0..MAX_WAITERS {
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    assert_eq!(
        delivery
            .declare_on(&host(vec![], 0, 0), event(5), Duration::ZERO)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    // Cleanup retains a reservation even when every admission slot is full.
    delivery.clear().unwrap();
    assert_eq!(delivery.pending(), 0);
    for release in releases {
        release.send(()).unwrap();
    }
    for call in calls {
        assert_eq!(call.join().unwrap(), io::ErrorKind::Interrupted);
    }
    no_waiters(&delivery);
}

#[test]
fn callbacks_can_replace_clear_refuse_self_wait_and_terminate_without_cycles() {
    let delivery = full();
    let owner = Arc::downgrade(&delivery);
    let (done, completed) = mpsc::channel();
    delivery.set_callback(Some(Box::new(move |events| {
        assert_eq!(events[0].kind, SENTENCE);
        let owner = owner.upgrade().unwrap();
        owner.set_callback(Some(Box::new(|_| {
            panic!("callback invoked after self termination")
        })));
        owner.clear().unwrap();
        for _ in 0..MAX_EVENTS {
            owner.declare(event(6), Duration::from_secs(30)).unwrap();
        }
        assert_eq!(
            owner
                .declare_wait(event(7), Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        owner.terminate();
        done.send(()).unwrap();
    })));
    pop_one(&delivery);
    completed.recv_timeout(Duration::from_secs(5)).unwrap();
    delivery.terminate();
    let weak = Arc::downgrade(&delivery.shared);
    drop(delivery);
    assert!(weak.upgrade().is_none());
}
