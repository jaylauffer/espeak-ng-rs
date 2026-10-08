//! Synthesis steps on one caller-owned loadngo completion port.
//!
//! Continue means useful local work remains. Pending leaves no queued step;
//! the host resumes it with Wake after an I/O completion. Wake/cancel coalesce
//! without a thread, timer or polling scheduler. Nested synchronous calls use
//! the same thread's port. A failed/stopped port never replays callbacks in a
//! plain loop, and queued jobs cannot access a legacy context after return.
// SPDX-License-Identifier: GPL-3.0-or-later
use loadngo_proactor::{
    new_platform_proactor, CompletionPort, PlatformPort, Proactor, ProactorHandle,
};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Another bounded pass can make progress without waiting for I/O.
    Continue,
    /// No local work remains; retain this pass until the host wakes it.
    Pending,
    Done,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Ready,
    Queued(u64),
    Running,
    Waiting,
    Done,
    Cancelled,
    Failed,
}
struct Gate {
    phase: Phase,
    active: bool,
    notified: bool,
    ticket: u64,
    error: Option<io::Error>,
}
type Callback<P> = Box<dyn FnMut(&Wake<P>) -> Step + Send>;
struct Control<P: CompletionPort> {
    handle: ProactorHandle<P>,
    gate: Mutex<Gate>,
    callback: Mutex<Option<Callback<P>>>,
}

/// A wake/cancel capability for one runner lifetime. It holds no callback,
/// buffer or port alive. Old capabilities cannot wake a subsequent run.
pub struct Wake<P: CompletionPort> {
    control: Weak<Control<P>>,
}
impl<P: CompletionPort> Clone for Wake<P> {
    fn clone(&self) -> Self {
        Self {
            control: self.control.clone(),
        }
    }
}
impl<P: CompletionPort> Wake<P> {
    /// Obtain the running caller's port to submit bounded I/O. The I/O owner
    /// must retain/cancel/drain its operations; ending this runner fences step
    /// callbacks, not unrelated I/O callbacks or their resources.
    pub fn handle(&self) -> io::Result<ProactorHandle<P>> {
        let control = self.control.upgrade().ok_or_else(closed)?;
        let gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
        if !gate.active || matches!(gate.phase, Phase::Done | Phase::Cancelled | Phase::Failed) {
            return Err(closed());
        }
        Ok(control.handle.clone())
    }
    /// Publish a host completion. A wake during the pass is retained even if
    /// the pass has not returned Pending yet. Multiple wakes coalesce.
    pub fn wake(&self) -> io::Result<bool> {
        let Some(control) = self.control.upgrade() else {
            return Ok(false);
        };
        let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
        if !gate.active {
            return Ok(false);
        }
        match gate.phase {
            Phase::Running => {
                let fresh = !gate.notified;
                gate.notified = true;
                Ok(fresh)
            }
            Phase::Waiting => {
                let ticket = gate.ticket;
                drop(gate);
                queue(&control, Phase::Waiting, ticket)
            }
            _ => Ok(false),
        }
    }
    /// Fence further passes and wake the owner to return Interrupted. This
    /// does not stop the shared port or cancel the host's outstanding I/O.
    pub fn cancel(&self) -> io::Result<bool> {
        let Some(control) = self.control.upgrade() else {
            return Ok(false);
        };
        let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
        if !gate.active || matches!(gate.phase, Phase::Done | Phase::Cancelled | Phase::Failed) {
            return Ok(false);
        }
        gate.phase = Phase::Cancelled;
        drop(gate);
        // There is no step in a Waiting phase; this wake-only completion
        // releases the owner's blocking poll without capturing this port.
        control.handle.enqueue_work(|_| {})?;
        Ok(true)
    }
}
fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "synthesis runner has ended")
}

/// An accepted callback runs only for its ticket and live runner. Work jobs
/// retain weak control references, so a queued job cannot keep its own port
/// alive or invoke a callback after its synchronous owner returns.
fn queue<P: CompletionPort>(
    control: &Arc<Control<P>>,
    observed_phase: Phase,
    observed_ticket: u64,
) -> io::Result<bool> {
    let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
    // A publisher may lose the gate between observing Waiting and posting.
    // Its stale observation cannot overwrite a running pass or a later wait.
    if !gate.active || gate.phase != observed_phase || gate.ticket != observed_ticket {
        return Ok(false);
    }
    let Some(ticket) = gate.ticket.checked_add(1) else {
        gate.phase = Phase::Failed;
        gate.error = Some(io::Error::other("synthesis work ticket overflow"));
        return Err(io::Error::other("synthesis work ticket overflow"));
    };
    gate.ticket = ticket;
    gate.phase = Phase::Queued(ticket);
    gate.notified = false;
    drop(gate);
    let weak = Arc::downgrade(control);
    if let Err(error) = control.handle.enqueue_work(move |_| {
        if let Some(control) = weak.upgrade() {
            dispatch(&control, ticket);
        }
    }) {
        let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
        if gate.active && gate.phase == Phase::Queued(ticket) {
            gate.phase = Phase::Failed;
            gate.error = Some(io::Error::new(error.kind(), error.to_string()));
        }
        return Err(error);
    }
    Ok(true)
}
fn dispatch<P: CompletionPort>(control: &Arc<Control<P>>, ticket: u64) {
    let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
    if !gate.active || gate.phase != Phase::Queued(ticket) {
        return;
    }
    gate.phase = Phase::Running;
    drop(gate);
    let wake = Wake {
        control: Arc::downgrade(control),
    };
    let result = {
        let mut callback = control.callback.lock().unwrap_or_else(|p| p.into_inner());
        let Some(callback) = callback.as_mut() else {
            return;
        };
        callback(&wake)
    };
    let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
    if !gate.active || gate.phase != Phase::Running {
        return;
    }
    match result {
        Step::Done => gate.phase = Phase::Done,
        Step::Pending if !gate.notified => gate.phase = Phase::Waiting,
        Step::Pending | Step::Continue => {
            drop(gate);
            let _ = queue(control, Phase::Running, ticket); // stores a refusal for the owner
        }
    }
}
struct Fence<P: CompletionPort>(Arc<Control<P>>);
impl<P: CompletionPort> Drop for Fence<P> {
    fn drop(&mut self) {
        self.0.gate.lock().unwrap_or_else(|p| p.into_inner()).active = false;
        // Drop the captured legacy context before its caller can release it.
        self.0
            .callback
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
    }
}

/// Run bounded synthesis passes on the supplied port until Done, cancellation
/// or a port error. Pending blocks on actual host completions, without issuing
/// another pass. The caller drives this port; hosts must wake after publishing
/// completion state and must own/drain I/O that can outlive this call.
pub fn run_on<P: CompletionPort>(
    host: &Proactor<P>,
    callback: impl FnMut(&Wake<P>) -> Step + Send + 'static,
) -> io::Result<()> {
    if !host.handle().is_running() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "synthesis proactor stopped",
        ));
    }
    let control = Arc::new(Control {
        handle: host.handle(),
        gate: Mutex::new(Gate {
            phase: Phase::Ready,
            active: true,
            notified: false,
            ticket: 0,
            error: None,
        }),
        callback: Mutex::new(Some(Box::new(callback))),
    });
    let _fence = Fence(Arc::clone(&control));
    queue(&control, Phase::Ready, 0)?;
    loop {
        {
            let mut gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
            match gate.phase {
                Phase::Done => return Ok(()),
                Phase::Cancelled => {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "synthesis cancelled",
                    ))
                }
                Phase::Failed => {
                    return Err(gate
                        .error
                        .take()
                        .unwrap_or_else(|| io::Error::other("synthesis work refused")))
                }
                _ => {}
            }
        }
        let report = host.run_once()?;
        if report.stopped {
            let gate = control.gate.lock().unwrap_or_else(|p| p.into_inner());
            if gate.phase != Phase::Done {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "synthesis proactor stopped",
                ));
            }
        }
    }
}
thread_local! {
    static PROACTOR: RefCell<Option<Rc<Proactor<PlatformPort>>>> = const { RefCell::new(None) };
    static CANCELLATION: RefCell<Option<Arc<Cancellation>>> = const { RefCell::new(None) };
}

/// One async command queue's cancellation scope. The queue serializes reset
/// with stop acknowledgement and new commands. Only the innermost runner is
/// registered; unwinding that runner restores and cancels its outer runner.
#[derive(Default)]
pub(crate) struct Cancellation(Mutex<CancelState>);
#[derive(Default)]
struct CancelState {
    requested: bool,
    current: Option<Wake<PlatformPort>>,
}
impl Cancellation {
    pub(crate) fn request(&self) {
        let wake = {
            let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
            state.requested = true;
            state.current.clone()
        };
        if let Some(wake) = wake {
            let _ = wake.cancel();
        }
    }
    pub(crate) fn reset(&self) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).requested = false;
    }
    fn bind(self: &Arc<Self>, wake: Wake<PlatformPort>) -> Binding {
        let (previous, requested) = {
            let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
            (state.current.replace(wake.clone()), state.requested)
        };
        if requested {
            let _ = wake.cancel();
        }
        Binding {
            scope: Arc::clone(self),
            previous,
            requested,
        }
    }
}
struct Binding {
    scope: Arc<Cancellation>,
    previous: Option<Wake<PlatformPort>>,
    requested: bool,
}
impl Drop for Binding {
    fn drop(&mut self) {
        let previous = {
            let mut state = self.scope.0.lock().unwrap_or_else(|p| p.into_inner());
            state.current = self.previous.take();
            state.requested.then(|| state.current.clone()).flatten()
        };
        if let Some(wake) = previous {
            let _ = wake.cancel();
        }
    }
}
/// Bind the queue's cancellation scope on its worker without exposing a
/// callback/context to the stopping thread or locking across owner code.
pub(crate) fn with_cancellation<T>(scope: Arc<Cancellation>, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<Arc<Cancellation>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CANCELLATION.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CANCELLATION.with(|slot| slot.replace(Some(scope))));
    run()
}
fn host() -> io::Result<Rc<Proactor<PlatformPort>>> {
    PROACTOR.with(|cell| {
        let mut slot = cell.borrow_mut();
        if let Some(host) = slot.as_ref() {
            return Ok(Rc::clone(host));
        }
        let host = Rc::new(new_platform_proactor()?);
        *slot = Some(Rc::clone(&host));
        Ok(host)
    })
}
pub fn on_proactor() -> bool {
    host().is_ok_and(|h| h.handle().is_running())
}
/// Run on the thread's cached port, also used by nested synchronous runs.
pub fn run_with(
    mut callback: impl FnMut(&Wake<PlatformPort>) -> Step + Send + 'static,
) -> io::Result<()> {
    let host = host()?;
    let cancellation = CANCELLATION.with(|slot| slot.borrow().clone());
    let mut binding = None;
    run_on(&host, move |wake| {
        if binding.is_none() {
            if let Some(scope) = cancellation.as_ref() {
                let registered = scope.bind(wake.clone());
                let requested = registered.requested;
                binding = Some(registered);
                if requested {
                    return Step::Done; // cancel has already fenced this pass
                }
            }
        }
        callback(wake)
    })
}
/// Legacy locally-ready steps, using the same runner and lifetime fence.
pub fn run(mut step: impl FnMut() -> Step + Send + 'static) -> io::Result<()> {
    run_with(move |_| step())
}

#[cfg(feature = "c-abi")]
mod c_api {
    use super::{run, Step};
    use std::ffi::c_void;
    struct Context(*mut c_void);
    // SAFETY: this thread's cached port runs every callback on the calling
    // thread; the fence removes the callback before this call returns.
    unsafe impl Send for Context {}
    #[no_mangle]
    extern "C" fn espeak_rs_synthesis_on_proactor() -> i32 {
        i32::from(super::on_proactor())
    }
    /// 1 completed on the proactor, -1 refused/failed/stopped; no direct replay.
    #[no_mangle]
    extern "C" fn espeak_rs_synthesis_run(
        step: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
        context: *mut c_void,
    ) -> i32 {
        let Some(step) = step else {
            return -1;
        };
        let context = Context(context);
        let result = run(move || {
            let context = &context;
            // SAFETY: live serialized owner context, fenced before return.
            if unsafe { step(context.0) } != 0 {
                Step::Done
            } else {
                Step::Continue
            }
        });
        if result.is_ok() {
            1
        } else {
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loadngo_proactor::{CompletionEnvelope, PollEvent};
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct FaultPort {
        queue: Arc<Mutex<VecDeque<CompletionEnvelope>>>,
        posts: AtomicUsize,
        polls: AtomicUsize,
        fail_post: usize,
        fail_poll: usize,
    }
    impl CompletionPort for FaultPort {
        fn post(&self, envelope: CompletionEnvelope) -> io::Result<()> {
            self.queue.lock().unwrap().push_back(envelope);
            if self.posts.fetch_add(1, Ordering::Relaxed) + 1 == self.fail_post {
                return Err(io::Error::other("injected post refusal after queueing"));
            }
            Ok(())
        }
        fn poll(&self, _: Option<Duration>) -> io::Result<PollEvent> {
            if self.polls.fetch_add(1, Ordering::Relaxed) + 1 == self.fail_poll {
                return Err(io::Error::other("injected poll error"));
            }
            self.queue
                .lock()
                .unwrap()
                .pop_front()
                .map(PollEvent::Completion)
                .ok_or_else(|| io::Error::other("no completion: runner lost its wake"))
        }
        fn wake(&self) -> io::Result<()> {
            Ok(())
        }
    }
    fn fault_host(fail_post: usize, fail_poll: usize) -> Proactor<FaultPort> {
        Proactor::new(FaultPort {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            posts: AtomicUsize::new(0),
            polls: AtomicUsize::new(0),
            fail_post,
            fail_poll,
        })
    }
    struct DropContext(Arc<AtomicUsize>);
    impl Drop for DropContext {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn requested_scope_fences_first_pass_and_restores_after_return() {
        let scope = Arc::new(Cancellation::default());
        scope.request();
        let result = with_cancellation(Arc::clone(&scope), || {
            run_with(|_| panic!("already cancelled callback"))
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        scope.reset();
        with_cancellation(scope, || run_with(|_| Step::Done)).unwrap();
        run_with(|_| Step::Done).unwrap();
    }

    #[test]
    fn cancellation_of_a_nested_run_also_fences_its_outer_pass() {
        let scope = Arc::new(Cancellation::default());
        let request = Arc::clone(&scope);
        let result = with_cancellation(scope, || {
            run_with(move |_| {
                let request = Arc::clone(&request);
                let inner = run_with(move |wake| {
                    let request = Arc::clone(&request);
                    wake.handle()
                        .unwrap()
                        .enqueue_work(move |_| request.request())
                        .unwrap();
                    Step::Pending
                });
                assert_eq!(inner.unwrap_err().kind(), io::ErrorKind::Interrupted);
                // Dropping the inner binding restored and cancelled this outer
                // runner; returning Done must not conceal the cancellation.
                Step::Done
            })
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        run_with(|_| Step::Done).unwrap();
    }

    #[test]
    fn pending_waits_for_completion_and_coalesces_early_and_duplicate_wakes() {
        let host = fault_host(0, 0);
        let log = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::clone(&log);
        let saved = Arc::new(Mutex::new(None));
        let token = Arc::clone(&saved);
        let mut pass = 0;
        run_on(&host, move |wake| {
            pass += 1;
            calls.lock().unwrap().push(pass);
            match pass {
                1 => {
                    *token.lock().unwrap() = Some(wake.clone());
                    assert!(wake.wake().unwrap());
                    assert!(!wake.wake().unwrap());
                    Step::Pending
                }
                2 => {
                    let resume = wake.clone();
                    let calls = Arc::clone(&calls);
                    wake.handle()
                        .unwrap()
                        .enqueue_work(move |_| {
                            calls.lock().unwrap().push(99);
                            assert!(resume.wake().unwrap());
                            assert!(!resume.wake().unwrap());
                        })
                        .unwrap();
                    Step::Pending
                }
                3 => Step::Done,
                _ => panic!("pending pass was replayed"),
            }
        })
        .unwrap();
        assert_eq!(*log.lock().unwrap(), [1, 2, 99, 3]);
        let old = saved.lock().unwrap().take().unwrap();
        assert!(!old.wake().unwrap());
        assert!(!old.cancel().unwrap());
        assert_eq!(
            old.handle().err().expect("ended runner").kind(),
            io::ErrorKind::NotConnected
        );
        run_on(&host, move |_| {
            assert!(!old.wake().unwrap());
            Step::Done
        })
        .unwrap();
    }

    #[test]
    fn delayed_waiting_wake_cannot_replace_a_running_pass_or_a_later_wait() {
        let host = fault_host(0, 0);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        let mut observed_ticket = 0;
        run_on(&host, move |wake| {
            let pass = count.fetch_add(1, Ordering::Relaxed) + 1;
            let control = wake.control.upgrade().unwrap();
            match pass {
                1 => {
                    observed_ticket = control.gate.lock().unwrap().ticket;
                    let resume = wake.clone();
                    wake.handle()
                        .unwrap()
                        .enqueue_work(move |_| {
                            assert!(resume.wake().unwrap());
                        })
                        .unwrap();
                    Step::Pending
                }
                2 => {
                    // A second publisher observed the first wait, then lost
                    // the CPU until the first publisher's resumed pass ran.
                    assert!(!queue(&control, Phase::Waiting, observed_ticket).unwrap());
                    let resume = wake.clone();
                    let old_ticket = observed_ticket;
                    wake.handle()
                        .unwrap()
                        .enqueue_work(move |_| {
                            let control = resume.control.upgrade().unwrap();
                            // The same delayed observation remains stale even
                            // when this second pass enters another Waiting phase.
                            assert!(!queue(&control, Phase::Waiting, old_ticket).unwrap());
                            assert!(resume.wake().unwrap());
                        })
                        .unwrap();
                    Step::Pending
                }
                3 => Step::Done,
                _ => panic!("stale wake replayed a synthesis pass"),
            }
        })
        .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn cancellation_wakes_a_pending_run_without_stopping_its_shared_port() {
        let host = fault_host(0, 0);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        let error = run_on(&host, move |wake| {
            assert_eq!(count.fetch_add(1, Ordering::Relaxed), 0);
            let cancel = wake.clone();
            wake.handle()
                .unwrap()
                .enqueue_work(move |_| {
                    assert!(cancel.cancel().unwrap());
                    assert!(!cancel.cancel().unwrap());
                })
                .unwrap();
            Step::Pending
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(host.handle().is_running());
        run_on(&host, |_| Step::Done).unwrap();
    }

    #[test]
    fn refused_and_failed_jobs_drop_context_and_never_replay_queued_callbacks() {
        for (post, poll, expected) in [(1, 0, 0), (2, 0, 1), (0, 1, 0), (0, 2, 1)] {
            let host = fault_host(post, poll);
            let dropped = Arc::new(AtomicUsize::new(0));
            let context = DropContext(Arc::clone(&dropped));
            let calls = Arc::new(AtomicUsize::new(0));
            let count = Arc::clone(&calls);
            let result = run_on(&host, move |_| {
                assert_eq!(context.0.load(Ordering::Relaxed), 0);
                count.fetch_add(1, Ordering::Relaxed);
                Step::Continue
            });
            assert!(result.is_err());
            assert_eq!(calls.load(Ordering::Relaxed), expected);
            assert_eq!(dropped.load(Ordering::Relaxed), 1);
            // Each fault left a queued envelope. A fresh run dispatches that
            // stale job first; its old legacy context must never be touched.
            run_on(&host, |_| Step::Done).unwrap();
            assert_eq!(calls.load(Ordering::Relaxed), expected);
            assert_eq!(dropped.load(Ordering::Relaxed), 1);
        }
    }

    #[test]
    fn stopped_port_returns_without_spinning_or_replaying_the_step() {
        let host = fault_host(0, 0);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        let error = run_on(&host, move |wake| {
            assert_eq!(count.fetch_add(1, Ordering::Relaxed), 0);
            wake.handle().unwrap().stop().unwrap();
            Step::Continue
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            run_on(&host, |_| panic!("stopped port"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn unwinding_a_step_fences_context_and_outstanding_wakes() {
        let host = fault_host(0, 0);
        let dropped = Arc::new(AtomicUsize::new(0));
        let context = DropContext(Arc::clone(&dropped));
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_on(&host, move |wake| {
                assert_eq!(context.0.load(Ordering::Relaxed), 0);
                let pending = wake.clone();
                wake.handle()
                    .unwrap()
                    .enqueue_work(move |_| {
                        assert!(!pending.wake().unwrap());
                    })
                    .unwrap();
                panic!("injected callback unwind");
            })
        }));
        assert!(caught.is_err());
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        run_on(&host, |_| Step::Done).unwrap();
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn nested_pending_runs_share_the_callers_port() {
        let first = host().unwrap();
        let address = Rc::as_ptr(&first) as usize;
        run_with(move |_| {
            assert_eq!(Rc::as_ptr(&host().unwrap()) as usize, address);
            let mut pass = 0;
            run_with(move |wake| {
                assert_eq!(Rc::as_ptr(&host().unwrap()) as usize, address);
                pass += 1;
                if pass == 1 {
                    let resume = wake.clone();
                    wake.handle()
                        .unwrap()
                        .enqueue_work(move |_| {
                            resume.wake().unwrap();
                        })
                        .unwrap();
                    Step::Pending
                } else {
                    assert_eq!(pass, 2);
                    Step::Done
                }
            })
            .unwrap();
            Step::Done
        })
        .unwrap();
    }

    #[test]
    fn steps_run_in_order_until_done() {
        assert!(on_proactor());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let mut count = 0;
        run(move || {
            count += 1;
            log.lock()
                .unwrap()
                .push((count, std::thread::current().id()));
            if count == 5 {
                Step::Done
            } else {
                Step::Continue
            }
        })
        .unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(
            seen.iter().map(|&(n, _)| n).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        // every step ran on the calling thread
        assert!(seen
            .iter()
            .all(|&(_, thread)| thread == std::thread::current().id()));
    }

    #[test]
    fn a_step_can_run_a_nested_loop() {
        let inner = Arc::new(Mutex::new(0));
        let count = Arc::clone(&inner);
        let mut outer = 0;
        run(move || {
            outer += 1;
            if outer == 2 {
                let count = Arc::clone(&count);
                run(move || {
                    let mut count = count.lock().unwrap();
                    *count += 1;
                    if *count == 3 {
                        Step::Done
                    } else {
                        Step::Continue
                    }
                })
                .unwrap();
            }
            if outer == 3 {
                Step::Done
            } else {
                Step::Continue
            }
        })
        .unwrap();
        assert_eq!(*inner.lock().unwrap(), 3);
    }
}
