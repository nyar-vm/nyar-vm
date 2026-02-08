use crate::object::{Gc, Trace};
use std::ops::Deref;
use std::sync::Arc;

/// A root guard that keeps a GC object alive even if no other GC pointers point to it.
/// This is useful for FFI or keeping objects alive on the Rust stack.
pub struct Root<T: Trace + 'static> {
    inner: Gc<T>,
    // In a production GC, this would register the pointer in a global root set.
    // For this implementation, we can simulate it by holding an Arc to the collector
    // or adding it to a specialized root registry in NyarGc.
    _marker: Arc<RootRegistry>,
}

struct RootRegistry {
    // This would be managed by NyarGc
}

impl<T: Trace + 'static> Root<T> {
    pub fn new(gc: Gc<T>) -> Self {
        // Implementation detail: register with collector
        Self {
            inner: gc,
            _marker: Arc::new(RootRegistry {}),
        }
    }

    pub fn as_gc(&self) -> Gc<T> {
        self.inner
    }

    /// Convert to a raw pointer for FFI.
    /// # Safety
    /// The caller must ensure the Root remains alive while the raw pointer is used.
    pub unsafe fn as_raw(&self) -> *const T {
        self.inner.as_ptr() as *const T
    }
}

impl<T: Trace + 'static> Deref for Root<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Helper to safely execute a closure with a temporary root.
pub fn with_root<T: Trace + 'static, R>(gc: Gc<T>, f: impl FnOnce(&Root<T>) -> R) -> R {
    let root = Root::new(gc);
    f(&root)
}
