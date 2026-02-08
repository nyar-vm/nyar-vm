use crate::collector::NyarGc;
use crate::object::{Gc, Trace};
use std::ops::Deref;
use std::sync::Arc;

/// A root guard that keeps a GC object alive even if no other GC pointers point to it.
/// This is useful for FFI or keeping objects alive on the Rust stack.
pub struct Root<T: Trace + 'static> {
    inner: Gc<T>,
    gc: Arc<NyarGc>,
}

impl<T: Trace + 'static> Root<T> {
    pub fn new(gc: Arc<NyarGc>, obj: Gc<T>) -> Self {
        unsafe {
            gc.register_global_root(&obj as *const dyn Trace);
        }
        Self { inner: obj, gc }
    }

    pub fn as_gc(&self) -> Gc<T> {
        self.inner
    }
}

impl<T: Trace + 'static> Drop for Root<T> {
    fn drop(&mut self) {
        unsafe {
            self.gc.unregister_global_root(&self.inner as *const dyn Trace);
        }
    }
}

impl<T: Trace + 'static> Deref for Root<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A handle for a persistent root that is not a GC-managed object itself, 
/// but contains GC-managed objects and implements Trace.
pub struct PersistentRoot<T: Trace + 'static> {
    gc: Arc<NyarGc>,
    ptr: *const T,
}

impl<T: Trace + 'static> PersistentRoot<T> {
    /// # Safety
    /// The pointer must remain valid and stable (not moved) until the PersistentRoot is dropped.
    pub unsafe fn new(gc: Arc<NyarGc>, ptr: *const T) -> Self {
        gc.register_global_root(ptr as *const dyn Trace);
        Self { gc, ptr }
    }
}

impl<T: Trace + 'static> Drop for PersistentRoot<T> {
    fn drop(&mut self) {
        unsafe {
            self.gc.unregister_global_root(self.ptr as *const dyn Trace);
        }
    }
}
