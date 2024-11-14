use futures_util::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::oneshot;
use tracing::debug;

/// A Future that can be completed.
#[derive(Default)]
pub struct CompletableTask {
    result: Option<Vec<u8>>,
    unblock_rx: Option<oneshot::Receiver<String>>,
}

impl CompletableTask {
    pub(crate) fn new() -> (Self, oneshot::Sender<String>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                result: None,
                unblock_rx: Some(rx),
            },
            tx,
        )
    }
    /// Complete with result
    fn complete(&mut self, result: Vec<u8>) {
        self.result = Some(result);
    }
    /// Complete with result
    fn fail(&mut self, result: Vec<u8>) {}
}

impl Future for CompletableTask {
    type Output = Vec<u8>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        debug!("calling task");
        match self.unblock_rx.as_mut().unwrap().poll_unpin(cx) {
            Poll::Ready(result) => {
                debug!("Complete task ready");
                let result = result.unwrap();
                Poll::Ready(serde_json::to_vec(&"done").unwrap())
                //Poll::Ready(result.into_bytes())
                //Poll::Ready(self.result.take().unwrap())
            }
            Poll::Pending => {
                debug!("Complete task not ready");
                Poll::Pending
            }
        }
    }
}
