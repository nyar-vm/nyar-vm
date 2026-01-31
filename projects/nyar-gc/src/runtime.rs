use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A wrapper for futures that ensures GC state is handled correctly during task switches.
pub struct GcRuntimeFuture<'a, F: Future> {
    gc: &'a crate::NyarGc,
    inner: F,
}

impl<'a, F: Future> GcRuntimeFuture<'a, F> {
    pub fn new(gc: &'a crate::NyarGc, future: F) -> Self {
        Self { gc, inner: future }
    }
}

impl<'a, F: Future> Future for GcRuntimeFuture<'a, F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Before polling, we could register this task's stack as a root.
        // After polling, if it returns Pending, we flush the thread-local buffers
        // to ensure that any objects marked during the task's execution are visible to GC.
        
        let gc = self.gc;
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.inner) };
        let result = inner.poll(cx);
        
        if result.is_pending() {
            // Task is being suspended, flush local state to ensure GC visibility
            // This is crucial for incremental/concurrent GC.
            gc.flush_thread_local();
        }
        
        result
    }
}

/// Helper trait to integrate GC with async runtimes.
pub trait GcRuntime {
    /// Wrap a future to make it GC-aware.
    fn wrap_gc<F>(&self, future: F) -> GcRuntimeFuture<'_, F>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
}

impl GcRuntime for crate::NyarGc {
    fn wrap_gc<F>(&self, future: F) -> GcRuntimeFuture<'_, F>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        GcRuntimeFuture::new(self, future)
    }
}
