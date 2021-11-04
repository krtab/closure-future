#![doc = include_str!("../README.md")]
#![cfg_attr(feature = "nightly-docs", feature(doc_cfg))]

use std::{
    future::Future,
    panic::{catch_unwind, UnwindSafe},
    pin::Pin,
    task::{Context, Poll},
};

/// The error type
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RunError {
    /// The wrapped closure panicked
    Panicked,
    /// The runner was dropped before it could complete
    RunnerDropped,
}

#[must_use = "The future must be polled for the output to be observed"]
/// A Future whose output will be the output of the closure
pub struct ClosureFuture<Output> {
    rcvr: async_oneshot::Receiver<Result<Output, RunError>>,
}

impl<Output> Future for ClosureFuture<Output> {
    type Output = Result<Output, RunError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.rcvr).poll(cx).map(|res| match res {
            Ok(res) => res,
            Err(async_oneshot::Closed { .. }) => Err(RunError::RunnerDropped),
        })
    }
}

#[must_use = "The worker must be run for the future to make any progress."]
/// A worker to ractually run the closure, using [`self.run()`](`Self::run`)
pub struct ClosureFutureWorker<Output, F: FnOnce() -> Output> {
    sndr: async_oneshot::Sender<Result<Output, RunError>>,
    f: F,
}

impl<Output, F> ClosureFutureWorker<Output, F>
where
    F: FnOnce() -> Output,
    //TODO: shall we rather wrap in AssertUnwindSafe below?
    F: UnwindSafe,
{
    /// Runs the closure
    pub fn run(mut self) {
        let res = catch_unwind(self.f);
        let to_store = res.map_err(|_| RunError::Panicked);
        let _ = self.sndr.send(to_store);
    }
}

/// Wraps a closure to be run as a Future
pub fn closure_future<Output, F: FnOnce() -> Output>(
    f: F,
) -> (ClosureFuture<Output>, ClosureFutureWorker<Output, F>) {
    let (sndr, rcvr) = async_oneshot::oneshot();
    (ClosureFuture { rcvr }, ClosureFutureWorker { sndr, f })
}

#[cfg(feature = "rayon")]
#[cfg_attr(feature = "nightly-docs", doc(cfg(feature = "rayon")))]
/// An helper function to spawn a closure on the rayon global threadpool.
/// Requires feature "rayon" to be activated.
pub fn spawn_rayon<Output: Send + Sync + 'static, F: FnOnce() -> Output + UnwindSafe + Send + 'static>(
    f: F,
) -> ClosureFuture<Output> {
    let (future, worker) = closure_future(f);
    rayon::spawn(|| worker.run());
    future
}