use crate::object::{MarkContext, Trace};
use std::cell::RefCell;
use std::marker::PhantomData;

thread_local! {
    /// A thread-local registry of GC roots on the stack.
    /// This is used for fast root scanning without full stack walking.
    pub static THREAD_ROOTS: RefCell<Vec<*const dyn Trace>> = RefCell::new(Vec::with_capacity(64));
}

/// A guard that registers a pointer as a root for the duration of its lifetime.
pub struct StackRootGuard<'a, T: Trace + 'static> {
    ptr: *const T,
    _marker: PhantomData<&'a T>,
}

impl<'a, T: Trace + 'static> StackRootGuard<'a, T> {
    pub fn new(value: &'a T) -> Self {
        let ptr = value as *const T;
        THREAD_ROOTS.with(|roots| {
            roots.borrow_mut().push(ptr as *const dyn Trace);
        });
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Create a root guard from a raw pointer.
    ///
    /// # Safety
    /// The pointer must remain valid for the duration of the guard's lifetime.
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        THREAD_ROOTS.with(|roots| {
            roots.borrow_mut().push(ptr as *const dyn Trace);
        });
        Self {
            ptr,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: Trace + 'static> Drop for StackRootGuard<'a, T> {
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
