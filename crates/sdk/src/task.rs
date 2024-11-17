use futures_util::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::oneshot;
use tracing::debug;

/// A Future that can be completed.
#[derive(Default)]
pub struct CompletableTask {
    completed: bool,
    result: Option<String>,
    unblock_rx: Option<oneshot::Receiver<Option<String>>>,
}

impl CompletableTask {
    pub(crate) fn new() -> (Self, oneshot::Sender<Option<String>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                completed: false,
                result: None,
                unblock_rx: Some(rx),
            },
            tx,
        )
    }
    /// Complete with result
    pub(crate) fn complete(&mut self, result: String) {
        self.result = Some(result);
        self.completed = true;
    }
    /// Complete with result
    pub(crate) fn fail(&mut self, result: String) {}
}

impl Future for CompletableTask {
    type Output = String;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.completed {
            let result = self.result.take().unwrap();
            return Poll::Ready(result);
        }
        debug!("calling task");
        match self.unblock_rx.as_mut().unwrap().poll_unpin(cx) {
            Poll::Ready(result) => {
                debug!("Complete task ready");
                match result {
                    Ok(result) => {
                        let result = result.unwrap();
                        Poll::Ready(result)
                    }
                    Err(_) => Poll::Pending,
                }
            }
            Poll::Pending => {
                debug!("Complete task not ready");
                Poll::Pending
            }
        }
    }
}
