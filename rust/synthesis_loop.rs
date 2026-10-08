//! The synthesis loop (`speech.c`'s `Synthesize`) as proactor work.
//!
//! Each step fills one output buffer, delivers it with its events, and
//! generates more. Each step is a work item on the calling thread's
//! loadngo proactor, and a step that is not the last posts the next. The
//! calling thread drives the proactor until the last step completes. That
//! thread is the caller for the synchronous API and the queue's worker for
//! the asynchronous one. Each thread has its own proactor, so no lock is
//! held while the owner's callbacks run. Where no proactor can be created,
//! the steps run in a plain loop.
// SPDX-License-Identifier: GPL-3.0-or-later
use loadngo_proactor::{new_platform_proactor, PlatformPort, Proactor, ProactorHandle};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

/// What a step reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Continue,
    Done,
}

thread_local! {
    static PROACTOR: RefCell<Option<Option<Proactor<PlatformPort>>>> = const { RefCell::new(None) };
}

/// Whether this thread's steps run on a proactor (creating it on first use).
pub fn on_proactor() -> bool {
    PROACTOR.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| new_platform_proactor().ok())
            .is_some()
    })
}

/// Runs `step` until it reports [`Step::Done`]: as proactor work items
/// driven on this thread, or in a plain loop without a proactor.
pub fn run(step: impl FnMut() -> Step + Send + 'static) {
    let step: Arc<Mutex<dyn FnMut() -> Step + Send>> = Arc::new(Mutex::new(step));
    let proactor = PROACTOR.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| new_platform_proactor().ok())
            .take()
    });
    let Some(proactor) = proactor else {
        while (step.lock().unwrap_or_else(|p| p.into_inner()))() == Step::Continue {}
        return;
    };
    let done = Arc::new(Mutex::new(false));
    let posted = post(&proactor.handle(), Arc::clone(&step), Arc::clone(&done));
    if posted {
        while !*done.lock().unwrap_or_else(|p| p.into_inner()) {
            if proactor.run_once().is_err() {
                break;
            }
        }
    }
    let finished = *done.lock().unwrap_or_else(|p| p.into_inner());
    // put this thread's proactor back; while it is out, a nested run (a
    // callback that synthesizes) steps in a plain loop
    PROACTOR.with(|cell| *cell.borrow_mut() = Some(Some(proactor)));
    if !finished {
        // the proactor refused the work: finish the remaining steps directly
        while (step.lock().unwrap_or_else(|p| p.into_inner()))() == Step::Continue {}
    }
}

type Shared = Arc<Mutex<dyn FnMut() -> Step + Send>>;

/// Posts one step; when it continues, it posts the next.
fn post(handle: &ProactorHandle<PlatformPort>, step: Shared, done: Arc<Mutex<bool>>) -> bool {
    let next = handle.clone();
    handle
        .enqueue_work(move |_| {
            let result = (step.lock().unwrap_or_else(|p| p.into_inner()))();
            if result == Step::Done || !post(&next, step, Arc::clone(&done)) {
                *done.lock().unwrap_or_else(|p| p.into_inner()) = result == Step::Done;
            }
        })
        .is_ok()
}

/// The C entry: runs `step(context)` until it returns nonzero.
#[cfg(feature = "c-abi")]
mod c_api {
    use super::{run, Step};
    use std::ffi::c_void;

    struct Context(*mut c_void);
    // SAFETY: the context is only used on the thread that called
    // `espeak_rs_synthesis_run`, which drives the proactor and so runs
    // every step itself; it outlives the call.
    unsafe impl Send for Context {}

    /// Whether this thread's synthesis steps run on a proactor.
    #[no_mangle]
    extern "C" fn espeak_rs_synthesis_on_proactor() -> i32 {
        i32::from(super::on_proactor())
    }

    /// Returns 1 when the steps ran as proactor work, 0 in a plain loop.
    #[no_mangle]
    extern "C" fn espeak_rs_synthesis_run(
        step: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
        context: *mut c_void,
    ) -> i32 {
        let Some(step) = step else {
            return 0;
        };
        let proactor = super::on_proactor();
        let context = Context(context);
        run(move || {
            let context = &context;
            // SAFETY: the owner's step over its own live context.
            if unsafe { step(context.0) } != 0 {
                Step::Done
            } else {
                Step::Continue
            }
        });
        i32::from(proactor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        });
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
                });
            }
            if outer == 3 {
                Step::Done
            } else {
                Step::Continue
            }
        });
        assert_eq!(*inner.lock().unwrap(), 3);
    }
}
