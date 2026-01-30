use std::alloc::{self, Layout};
use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

/// A trait for types that can be traced by the garbage collector.
pub trait Trace {
    /// Trace all GC pointers contained within this object.
    fn trace(&self, ctx: &mut MarkContext);
}

/// Context used during the marking phase of GC.
pub struct MarkContext {
    pub(crate) gray_stack: Vec<NonNull<GcHeader>>,
}

impl MarkContext {
    /// Mark a GC pointer as reachable.
    pub unsafe fn mark(&mut self, ptr: NonNull<GcHeader>) {
        let header = ptr.as_ref();
        if header.color.get() == Color::White {
            header.color.set(Color::Gray);
            self.gray_stack.push(ptr);
        }
    }
}

/// Color of an object for tri-color marking.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// Not yet visited.
    White = 0,
    /// Visited, but children not yet visited.
    Gray = 1,
    /// Visited and children visited.
    Black = 2,
}

/// Metadata stored at the beginning of every GC-managed allocation.
pub struct GcHeader {
    /// Color of this object for tri-color marking.
    pub color: Cell<Color>,
    /// Which generation this object belongs to (0 for young, 1 for old).
    pub(crate) generation: Cell<u8>,
    /// Whether this object is in the remembered set (old object pointing to young).
    pub(crate) dirty: Cell<bool>,
    /// Link to the next object in the collector's list.
    pub next: Cell<Option<NonNull<GcHeader>>>,
    /// Function to drop and deallocate the object.
    pub drop_and_dealloc: unsafe fn(NonNull<GcHeader>),
    /// Function to trace the object.
    pub trace_object: unsafe fn(NonNull<GcHeader>),
    /// Size of the allocation in bytes.
    pub size: usize,
}

impl GcHeader {
    /// Mark the object and trace its children if it wasn't already marked.
    pub unsafe fn mark_and_trace(ptr: NonNull<GcHeader>) {
        let header = ptr.as_ref();
        if header.color.get() == Color::White {
            header.color.set(Color::Gray);
            // In a full tri-color implementation, this would push to a gray stack.
            // For now, we'll keep it simple and trace immediately to keep compatibility
            // with the current single-threaded Stop-the-World model, but use the color.
            (header.trace_object)(ptr);
            header.color.set(Color::Black);
        }
    }
}

#[repr(C)]
pub struct GcBox<T: Trace + 'static> {
    pub header: GcHeader,
    pub data: T,
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
    /// Head of the linked list of young generation objects.
    young_head: Cell<Option<NonNull<GcHeader>>>,
    /// Head of the linked list of old generation objects.
    old_head: Cell<Option<NonNull<GcHeader>>>,
    /// Objects in the old generation that may point to young generation.
    remembered_set: RefCell<Vec<NonNull<GcHeader>>>,
    /// Total number of bytes allocated.
    allocated_bytes: Cell<usize>,
    /// Threshold for the next collection cycle.
    threshold: Cell<usize>,
}

impl NyarGc {
    /// Create a new garbage collector.
    pub fn new() -> Self {
        Self {
            young_head: Cell::new(None),
            old_head: Cell::new(None),
            remembered_set: RefCell::new(Vec::new()),
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
                    color: Cell::new(Color::White),
                    generation: Cell::new(0), // New objects are always in young generation
                    dirty: Cell::new(false),
                    next: Cell::new(self.young_head.get()),
                    drop_and_dealloc: Self::drop_and_dealloc::<T>,
                    trace_object: Self::trace_object::<T>,
                    size: layout.size(),
                },
            );
            std::ptr::write(&mut (*ptr).data, value);

            let gc_box = NonNull::new_unchecked(ptr);
            self.young_head.set(Some(NonNull::new_unchecked(&mut (*ptr).header)));
            self.allocated_bytes.set(self.allocated_bytes.get() + layout.size());

            if self.allocated_bytes.get() > self.threshold.get() {
                // Trigger minor collection first
                self.collect_minor(|| {});
            }

            Gc { ptr: gc_box }
        }
    }

    /// Write barrier: should be called when an old object is modified to point to a young object.
    pub fn write_barrier<T: Trace + 'static, U: Trace + 'static>(&self, parent: Gc<T>, child: Gc<U>) {
        unsafe {
            let parent_header = &parent.ptr.as_ref().header;
            let child_header = &child.ptr.as_ref().header;

            // If parent is old and child is young, add parent to remembered set
            if parent_header.generation.get() > 0 && child_header.generation.get() == 0 {
                if !parent_header.dirty.get() {
                    parent_header.dirty.set(true);
                    self.remembered_set.borrow_mut().push(NonNull::new_unchecked(parent_header as *const _ as *mut _));
                }
            }
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

    unsafe fn trace_object<T: Trace + 'static>(header_ptr: NonNull<GcHeader>) {
        let ptr = header_ptr.cast::<GcBox<T>>();
        (*ptr.as_ptr()).data.trace();
    }

    /// Run a collection cycle.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure.
    pub unsafe fn collect<F>(&self, mark_roots: F)
    where
        F: FnOnce(),
    {
        // For simplicity, a full collect is a major collection
        self.collect_major(mark_roots);
    }

    /// Minor collection: only collect young generation.
    pub unsafe fn collect_minor<F>(&self, mark_roots: F)
    where
        F: FnOnce(),
    {
        // 1. Mark roots
        mark_roots();

        // 2. Mark from remembered set (old -> young)
        for &header_ptr in self.remembered_set.borrow().iter() {
            let header = header_ptr.as_ref();
            // Trace the object to mark its young children
            (header.trace_object)(header_ptr);
        }

        // 3. Sweep young generation and promote survivors
        self.sweep_young();

        // 4. Reset remembered set for next cycle
        // Only keep objects that are still dirty (though in minor GC we usually clear them
        // and let the write barrier re-add them if they still point to young)
        let mut remembered = self.remembered_set.borrow_mut();
        for &header_ptr in remembered.iter() {
            header_ptr.as_ref().dirty.set(false);
        }
        remembered.clear();
    }

    /// Major collection: collect all generations.
    pub unsafe fn collect_major<F>(&self, mark_roots: F)
    where
        F: FnOnce(),
    {
        // 1. Mark roots
        mark_roots();

        // 2. Sweep everything
        self.sweep_full();

        // 3. Clear remembered set
        let mut remembered = self.remembered_set.borrow_mut();
        for &header_ptr in remembered.iter() {
            header_ptr.as_ref().dirty.set(false);
        }
        remembered.clear();

        // 4. Adjust threshold
        self.threshold.set(self.allocated_bytes.get() * 2);
    }

    unsafe fn sweep_young(&self) {
        let prev: Option<NonNull<GcHeader>> = None;
        let mut curr = self.young_head.get();

        while let Some(header_ptr) = curr {
            let header = header_ptr.as_ref();
            let next = header.next.get();

            if header.color.get() != Color::White {
                // Object survived! Promote to old generation.
                header.color.set(Color::White);
                header.generation.set(1);

                // Remove from young list
                if let Some(mut p) = prev {
                    p.as_mut().next.set(next);
                } else {
                    self.young_head.set(next);
                }

                // Add to old list
                header.next.set(self.old_head.get());
                self.old_head.set(Some(header_ptr));

                // Since we removed it from the current list, prev doesn't change
                curr = next;
            } else {
                // Object is unreachable, free it
                if let Some(mut p) = prev {
                    p.as_mut().next.set(next);
                } else {
                    self.young_head.set(next);
                }

                // Update allocated_bytes
                self.allocated_bytes.set(self.allocated_bytes.get() - header.size);
                (header.drop_and_dealloc)(header_ptr);
                curr = next;
            }
        }
    }

    unsafe fn sweep_full(&self) {
        // Sweep young
        self.sweep_young();

        // Sweep old
        let mut prev: Option<NonNull<GcHeader>> = None;
        let mut curr = self.old_head.get();

        while let Some(header_ptr) = curr {
            let header = header_ptr.as_ref();
            let next = header.next.get();

            if header.color.get() != Color::White {
                header.color.set(Color::White);
                prev = Some(header_ptr);
                curr = next;
            } else {
                if let Some(mut p) = prev {
                    p.as_mut().next.set(next);
                } else {
                    self.old_head.set(next);
                }

                // Update allocated_bytes
                self.allocated_bytes.set(self.allocated_bytes.get() - header.size);
                (header.drop_and_dealloc)(header_ptr);
                curr = next;
            }
        }
    }
}

impl<T: Trace + 'static> Trace for Gc<T> {
    fn trace(&self) {
        unsafe {
            let header_ptr = NonNull::new_unchecked(&self.ptr.as_ref().header as *const _ as *mut _);
            GcHeader::mark_and_trace(header_ptr);
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
