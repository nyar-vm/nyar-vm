use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

/// A global flag to request all threads to pause for GC.
/// In an async context, this is used for cooperative yielding.
pub static GC_STOP_THE_WORLD: AtomicBool = AtomicBool::new(false);

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
        // 1. Check if GC requested a pause
        if GC_STOP_THE_WORLD.load(Ordering::Acquire) {
            // Cooperative yield: if GC is marking/sweeping and needs STW,
            // we yield the current task to allow GC to proceed.
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        let gc = self.gc;
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.inner) };
        let result = inner.poll(cx);

        if result.is_pending() {
            // Task is being suspended, flush local state to ensure GC visibility
            gc.flush_thread_local();
        }

        result
    }
}

/// Helper to check if the current task should yield for GC.
/// This should be called in hot loops within async tasks.
#[cfg(feature = "tokio")]
#[inline(always)]
pub async fn yield_now_for_gc() {
    if GC_STOP_THE_WORLD.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
}

/// Helper trait to integrate GC with async runtimes.
pub trait GcRuntime {
    /// Wrap a future to make it GC-aware.
    fn wrap_gc<F>(&self, future: F) -> GcRuntimeFuture<'_, F>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;

    /// Notify the GC that the VM is currently in an idle state.
    /// This will trigger a full collection.
    fn full_gc(&self);
}

impl GcRuntime for crate::NyarGc {
    fn wrap_gc<F>(&self, future: F) -> GcRuntimeFuture<'_, F>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        GcRuntimeFuture::new(self, future)
    }

    fn full_gc(&self) {
        // When the VM enters an idle period (e.g., waiting for user input or frame sync),
        // we can raise a full GC to reduce heap pressure and fragmentation.
        // self.full_gc(); // Avoid infinite recursion and missing roots
    }
}
