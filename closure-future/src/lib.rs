use std::mem::ManuallyDrop;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::Mutex;
use std::task::Waker;
use std::{
    any::Any,
    cell::UnsafeCell,
    future::Future,
    panic::{catch_unwind, UnwindSafe},
    pin::Pin,
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Poll},
};

#[derive(Debug)]
pub enum RunError {
    Panicked(Box<dyn Any + Send + 'static>),
    RunnerDropped,
}

// Invariants:
//  1. If runner_finished = true, then rsult is initialized
struct SharedState<Output> {
    runner_finished: AtomicBool,
    result: UnsafeCell<Option<Result<Output, RunError>>>,
    waker: Mutex<Option<Waker>>,
}

pub struct ClosureFuture<Output> {
    state: Arc<SharedState<Output>>,
}

unsafe impl<Output> Send for ClosureFuture<Output> {}

impl<Output> Future for ClosureFuture<Output> {
    type Output = Result<Output, RunError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state.runner_finished.load(Acquire) {
            let res = unsafe { &mut *self.state.result.get() }
                .take()
                .expect("This future has already been polled to completion.");
            Poll::Ready(res)
        } else {
            let new_waker = cx.waker();
            let mut old_waker = self
                .state
                .waker
                .lock()
                .expect("Couldn't lock waker storage when polling.");
            *old_waker = Some(new_waker.clone());
            Poll::Pending
        }
    }
}

pub struct ClosureFutureWorker<Output, F: FnOnce() -> Output> {
    state: Arc<SharedState<F::Output>>,
    f: F,
}

impl<Output, F> ClosureFutureWorker<Output, F>
where
    F: FnOnce() -> Output,
{
    fn deconstruct_bypassing_drop(mut self) -> (Arc<SharedState<F::Output>>, F) {
        unsafe {
            let res = (
                ManuallyDrop::take(std::mem::transmute(&mut self.state)),
                ManuallyDrop::take(std::mem::transmute(&mut self.f)),
            );
            std::mem::forget(self);
            res
        }
    }
}

unsafe impl<Output, F> Send for ClosureFutureWorker<Output, F> where F: FnOnce() -> Output {}

impl<Output, F> ClosureFutureWorker<Output, F>
where
    F: FnOnce() -> Output,
    //TODO: shall we rather wrap in AssertUnwindSafe below?
    F: UnwindSafe,
{
    pub fn run(self) {
        // We bypass the drop because otherwise the borrow-chacker
        // is upset that we move f out of self, and we should
        // finish this function anyway
        let (state, f) = self.deconstruct_bypassing_drop();
        let res = catch_unwind(f);
        let to_store = res.map_err(|e| RunError::Panicked(e));
        // No other code can have marked the code as finished before
        // so we must be the only one accessing the storage cell
        unsafe { *state.result.get() = Some(to_store) };
        state.runner_finished.store(true, Release);
        if let Some(waker) = state
            .waker
            .lock()
            .expect("Couldn't lock waker to wake it.")
            .take()
        {
            waker.wake();
        };
    }
}

impl<Output, F> Drop for ClosureFutureWorker<Output, F>
where
    F: FnOnce() -> Output,
{
    fn drop(&mut self) {
        // We are being dropped without having finished running the closure.
        if !self.state.runner_finished.load(Acquire) {
            let to_store = Err(RunError::RunnerDropped);
            // No other code have marked the code as finished before
            // so we must be the only one accessing the storage cell
            unsafe { *self.state.result.get() = Some(to_store) };
            self.state.runner_finished.store(true, Release)
        }
    }
}

pub fn wrap_as_future<Output, F: FnOnce() -> Output>(
    f: F,
) -> (ClosureFuture<Output>, ClosureFutureWorker<Output, F>) {
    let state = Arc::new(SharedState {
        runner_finished: AtomicBool::new(false),
        result: UnsafeCell::new(None),
        waker: Mutex::new(None),
    });
    (
        ClosureFuture {
            state: state.clone(),
        },
        ClosureFutureWorker { state, f },
    )
}
