use std::{
    future::Future,
    panic::{catch_unwind, UnwindSafe},
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Debug)]
pub enum RunError {
    Panicked,
    RunnerDropped,
}

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
    pub fn run(mut self) {
        let res = catch_unwind(self.f);
        let to_store = res.map_err(|_| RunError::Panicked);
        let _ = self.sndr.send(to_store);
    }
}

pub fn closure_future<Output, F: FnOnce() -> Output>(
    f: F,
) -> (ClosureFuture<Output>, ClosureFutureWorker<Output, F>) {
    let (sndr, rcvr) = async_oneshot::oneshot();
    (ClosureFuture { rcvr }, ClosureFutureWorker { sndr, f })
}
