use crate::object::{MarkContext, Trace};
use std::cell::RefCell;

thread_local! {
    /// A thread-local registry of GC roots on the stack.
    /// This is used for fast root scanning without full stack walking.
    pub static THREAD_ROOTS: RefCell<Vec<*const dyn Trace>> = RefCell::new(Vec::with_capacity(64));
}

/// A guard that registers a pointer as a root for the duration of its lifetime.
pub struct StackRootGuard<T: Trace + 'static> {
    ptr: *const T,
}

impl<T: Trace + 'static> StackRootGuard<T> {
    pub fn new(ptr: *const T) -> Self {
        THREAD_ROOTS.with(|roots| {
            roots.borrow_mut().push(ptr as *const dyn Trace);
        });
        Self {
            ptr,
        }
    }
}

impl<T: Trace + 'static> Drop for StackRootGuard<T> {
    fn drop(&mut self) {
        THREAD_ROOTS.with(|roots| {
            let mut roots = roots.borrow_mut();
            if let Some(pos) = roots.iter().rposition(|&p| p == self.ptr as *const dyn Trace) {
                roots.swap_remove(pos);
            }
        });
    }
}

/// Scan all roots registered in the current thread.
pub unsafe fn scan_thread_roots(ctx: &mut MarkContext) {
    THREAD_ROOTS.with(|roots| {
        for root in roots.borrow().iter() {
            (**root).trace(ctx);
        }
    });
}
