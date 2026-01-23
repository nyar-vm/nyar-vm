use std::alloc::{self, Layout};
use std::cell::Cell;
use std::ptr::NonNull;

/// A trait for types that can be traced by the garbage collector.
pub trait Trace {
    /// Trace all GC pointers contained within this object.
    fn trace(&self);
}

/// Metadata stored at the beginning of every GC-managed allocation.
pub struct GcHeader {
    /// Whether this object has been marked during the current collection cycle.
    pub(crate) marked: Cell<bool>,
    /// Link to the next object in the collector's list.
    pub(crate) next: Cell<Option<NonNull<GcHeader>>>,
    /// Function to drop and deallocate the object.
    pub(crate) drop_and_dealloc: unsafe fn(NonNull<GcHeader>),
}

#[repr(C)]
struct GcBox<T: Trace + 'static> {
    header: GcHeader,
    data: T,
}

/// A garbage-collected pointer to a value of type `T`.
pub struct Gc<T: Trace + 'static> {
    ptr: NonNull<GcBox<T>>,
}

impl<T: Trace + 'static> Copy for Gc<T> {}
impl<T: Trace + 'static> Clone for Gc<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Trace + 'static> std::ops::Deref for Gc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &self.ptr.as_ref().data }
    }
}

/// The garbage collector itself.
pub struct NyarGc {
    /// Head of the linked list of all allocated objects.
    head: Cell<Option<NonNull<GcHeader>>>,
    /// Total number of bytes allocated.
    allocated_bytes: Cell<usize>,
    /// Threshold for the next collection cycle.
    threshold: Cell<usize>,
}

impl NyarGc {
    /// Create a new garbage collector.
    pub fn new() -> Self {
        Self {
            head: Cell::new(None),
            allocated_bytes: Cell::new(0),
            threshold: Cell::new(1024 * 1024), // 1MB default threshold
        }
    }

    /// Allocate a new value on the managed heap.
    pub fn alloc<T: Trace + 'static>(&self, value: T) -> Gc<T> {
        let layout = Layout::new::<GcBox<T>>();
        unsafe {
            let ptr = alloc::alloc(layout) as *mut GcBox<T>;
            if ptr.is_null() {
                alloc::handle_alloc_error(layout);
            }

            std::ptr::write(
                &mut (*ptr).header,
                GcHeader {
                    marked: Cell::new(false),
                    next: Cell::new(self.head.get()),
                    drop_and_dealloc: Self::drop_and_dealloc::<T>,
                },
            );
            std::ptr::write(&mut (*ptr).data, value);

            let gc_box = NonNull::new_unchecked(ptr);
            self.head.set(Some(NonNull::new_unchecked(&mut (*ptr).header)));
            self.allocated_bytes.set(self.allocated_bytes.get() + layout.size());

            Gc { ptr: gc_box }
        }
    }

    unsafe fn drop_and_dealloc<T: Trace + 'static>(header_ptr: NonNull<GcHeader>) {
        let ptr = header_ptr.cast::<GcBox<T>>();
        let layout = Layout::new::<GcBox<T>>();
        // Explicitly drop the data
        std::ptr::drop_in_place(&mut (*ptr.as_ptr()).data);
        // Deallocate the memory
        alloc::dealloc(ptr.as_ptr() as *mut u8, layout);
    }

    /// Run a collection cycle.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure.
    pub unsafe fn collect<F>(&self, mark_roots: F)
    where
        F: FnOnce(),
    {
        // 1. Marking phase
        mark_roots();

        // 2. Sweeping phase
        self.sweep();
    }

    unsafe fn sweep(&self) {
        let mut prev: Option<NonNull<GcHeader>> = None;
        let mut curr = self.head.get();

        while let Some(header_ptr) = curr {
            let header = header_ptr.as_ref();
            let next = header.next.get();

            if header.marked.get() {
                // Object is reachable, unmark for next cycle
                header.marked.set(false);
                prev = Some(header_ptr);
                curr = next;
            } else {
                // Object is unreachable, free it
                if let Some(mut p) = prev {
                    p.as_mut().next.set(next);
                } else {
                    self.head.set(next);
                }

                (header.drop_and_dealloc)(header_ptr);
                curr = next;
            }
        }
    }
}

impl<T: Trace + 'static> Trace for Gc<T> {
    fn trace(&self) {
        unsafe {
            let header = &self.ptr.as_ref().header;
            if !header.marked.get() {
                header.marked.set(true);
                self.ptr.as_ref().data.trace();
            }
        }
    }
}

// Implement Trace for common types
impl Trace for i64 { fn trace(&self) {} }
impl Trace for f64 { fn trace(&self) {} }
impl Trace for bool { fn trace(&self) {} }
impl Trace for String { fn trace(&self) {} }
impl<T: Trace> Trace for Vec<T> {
    fn trace(&self) {
        for item in self {
            item.trace();
        }
    }
}
impl<T: Trace> Trace for Option<T> {
    fn trace(&self) {
        if let Some(inner) = self {
            inner.trace();
        }
    }
}
