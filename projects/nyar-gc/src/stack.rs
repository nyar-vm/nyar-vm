use crate::object::{MarkContext, Trace};
use crate::ptr::SendPtr;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use dashmap::DashMap;
use std::thread::ThreadId;

/// A structure to hold thread-local roots that can be accessed from other threads.
pub struct ThreadRoots {
    pub roots: Vec<SendPtr<dyn Trace>>,
}

static GLOBAL_THREAD_ROOTS: Lazy<DashMap<ThreadId, Arc<Mutex<ThreadRoots>>>> = Lazy::new(DashMap::new);

thread_local! {
    /// A thread-local registry of GC roots on the stack.
    /// This is used for fast root scanning without full stack walking.
    pub static THREAD_ROOTS: Arc<Mutex<ThreadRoots>> = {
        let roots = Arc::new(Mutex::new(ThreadRoots { roots: Vec::with_capacity(64) }));
        GLOBAL_THREAD_ROOTS.insert(std::thread::current().id(), roots.clone());
        roots
    };
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
            let send_ptr = unsafe { SendPtr(NonNull::new_unchecked(ptr as *mut T as *mut dyn Trace)) };
            roots.lock().unwrap().roots.push(send_ptr);
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
            let send_ptr = SendPtr(NonNull::new_unchecked(ptr as *mut T as *mut dyn Trace));
            roots.lock().unwrap().roots.push(send_ptr);
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
            let mut roots = roots.lock().unwrap();
            let target = self.ptr as *const dyn Trace;
            if let Some(pos) = roots.roots.iter().rposition(|p| std::ptr::addr_eq(p.as_ptr() as *const dyn Trace, target)) {
                roots.roots.swap_remove(pos);
            }
        });
    }
}

/// Scan all roots registered in all threads.
pub unsafe fn scan_all_thread_roots(ctx: &mut MarkContext) {
    for entry in GLOBAL_THREAD_ROOTS.iter() {
        let roots = entry.value().lock().unwrap();
        for root in roots.roots.iter() {
            root.as_ref().trace(ctx);
        }
    }
}

/// Scan all roots registered in the current thread.
pub unsafe fn scan_thread_roots(ctx: &mut MarkContext) {
    THREAD_ROOTS.with(|roots| {
        let roots = roots.lock().unwrap();
        for root in roots.roots.iter() {
            root.as_ref().trace(ctx);
        }
    });
}
