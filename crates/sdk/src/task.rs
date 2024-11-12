use std::future::Future;

/// A Future that can be completed.
pub trait CompletableFuture<T>: Future<Output = T> {
    /// Complete with result
    fn complete(&self, result: T);
}
