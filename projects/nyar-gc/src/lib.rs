use std::alloc::{self, Layout};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ptr::NonNull;

const BLOCK_SIZE: usize = 1024 * 1024; // 1MB blocks
const CARD_SIZE: usize = 512;
const CARDS_PER_BLOCK: usize = BLOCK_SIZE / CARD_SIZE;

struct GcBlock {
    ptr: NonNull<u8>,
    cursor: Cell<usize>,
    /// Card table for this block. Each byte represents CARD_SIZE bytes.
    /// 0 = clean, 1 = dirty.
    card_table: UnsafeCell<[u8; CARDS_PER_BLOCK]>,
}

impl GcBlock {
    fn new() -> Self {
        let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
        unsafe {
            let ptr = alloc::alloc(layout);
            if ptr.is_null() {
                alloc::handle_alloc_error(layout);
            }
            Self {
                ptr: NonNull::new_unchecked(ptr),
                cursor: Cell::new(0),
                card_table: UnsafeCell::new([0; CARDS_PER_BLOCK]),
            }
        }
    }

    fn alloc(&self, layout: Layout) -> Option<*mut u8> {
        let cursor = self.cursor.get();
        let align_offset = (self.ptr.as_ptr() as usize + cursor) % layout.align();
        let padding = if align_offset == 0 { 0 } else { layout.align() - align_offset };
        let new_cursor = cursor + padding + layout.size();

        if new_cursor <= BLOCK_SIZE {
            self.cursor.set(new_cursor);
            unsafe { Some(self.ptr.as_ptr().add(cursor + padding)) }
        } else {
            None
        }
    }

    fn contains(&self, ptr: *const u8) -> bool {
        let addr = ptr as usize;
        let base = self.ptr.as_ptr() as usize;
        addr >= base && addr < base + BLOCK_SIZE
    }

    fn mark_dirty(&self, ptr: *const u8) {
        let offset = ptr as usize - self.ptr.as_ptr() as usize;
        let card_idx = offset / CARD_SIZE;
        unsafe {
            (*self.card_table.get())[card_idx] = 1;
        }
    }

    fn is_card_dirty(&self, card_idx: usize) -> bool {
        unsafe { (*self.card_table.get())[card_idx] == 1 }
    }

    fn clear_cards(&self) {
        unsafe {
            std::ptr::write_bytes(self.card_table.get() as *mut u8, 0, CARDS_PER_BLOCK);
        }
    }
}

impl Drop for GcBlock {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
        unsafe {
            alloc::dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

/// A cell that can be used within GC-managed objects to store GC pointers.
pub struct GcCell<T: Trace + 'static> {
    inner: UnsafeCell<T>,
}

impl<T: Trace + 'static> GcCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
        }
    }

    /// Set the value of the cell.
    ///
    /// # Safety
    /// This does not automatically trigger a write barrier. Use `NyarGc::write` for that.
    pub fn set(&self, value: T) {
        unsafe {
            *self.inner.get() = value;
        }
    }

    /// Get a reference to the value.
    pub unsafe fn get_ref(&self) -> &T {
        &*self.inner.get()
    }
}

impl<T: Trace + 'static> Trace for GcCell<T> {
    fn trace(&self, ctx: &mut MarkContext) {
        unsafe {
            (*self.inner.get()).trace(ctx);
        }
    }
}

/// A trait for types that can be traced by the garbage collector.
pub trait Trace {
    /// Trace all GC pointers contained within this object.
    fn trace(&self, ctx: &mut MarkContext<'_>);
}

/// Context used during the marking phase of GC.
pub struct MarkContext<'a> {
    pub(crate) gray_stack: &'a mut Vec<NonNull<GcHeader>>,
}

impl<'a> MarkContext<'a> {
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
    /// Link to the next object in the collector's list.
    pub next: Cell<Option<NonNull<GcHeader>>>,
    /// Function to drop and deallocate the object.
    pub drop_and_dealloc: unsafe fn(NonNull<GcHeader>),
    /// Function to trace the object.
    pub trace_object: unsafe fn(NonNull<GcHeader>, &mut MarkContext<'_>),
    /// Size of the allocation in bytes.
    pub size: usize,
}

impl GcHeader {
    /// Mark the object and trace its children if it wasn't already marked.
    pub unsafe fn mark(ptr: NonNull<GcHeader>, ctx: &mut MarkContext<'_>) {
        ctx.mark(ptr);
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

impl<T: Trace + 'static> Gc<T> {
    pub fn as_ptr(&self) -> *mut GcBox<T> {
        self.ptr.as_ptr()
    }
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
/// Current state of the garbage collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GcState {
    /// GC is not currently running.
    Idle,
    /// GC is currently marking objects.
    Marking,
    /// GC is currently sweeping unreachable objects.
    Sweeping,
}

pub struct NyarGc {
    /// Head of the linked list of young generation objects.
    young_head: Cell<Option<NonNull<GcHeader>>>,
    /// Head of the linked list of old generation objects.
    old_head: Cell<Option<NonNull<GcHeader>>>,
    /// Memory blocks managed by the GC.
    blocks: RefCell<Vec<GcBlock>>,
    /// Gray stack for tri-color marking.
    gray_stack: RefCell<Vec<NonNull<GcHeader>>>,
    /// Current state of the GC.
    state: Cell<GcState>,
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
            blocks: RefCell::new(vec![GcBlock::new()]),
            gray_stack: RefCell::new(Vec::new()),
            state: Cell::new(GcState::Idle),
            allocated_bytes: Cell::new(0),
            threshold: Cell::new(1024 * 1024), // 1MB default threshold
        }
    }

    /// Allocate a new value on the managed heap.
    pub fn alloc<T: Trace + 'static>(&self, value: T) -> Gc<T> {
        let layout = Layout::new::<GcBox<T>>();
        let ptr = {
            let mut blocks = self.blocks.borrow_mut();
            let mut ptr = blocks.last().unwrap().alloc(layout);
            if ptr.is_none() {
                blocks.push(GcBlock::new());
                ptr = blocks.last().unwrap().alloc(layout);
            }
            ptr.unwrap() as *mut GcBox<T>
        };

        unsafe {
            std::ptr::write(
                &mut (*ptr).header,
                GcHeader {
                    color: Cell::new(Color::White),
                    generation: Cell::new(0),
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
                self.collect_minor(|_| {});
            }

            Gc { ptr: gc_box }
        }
    }

    fn find_block(&self, ptr: *const u8) -> Option<usize> {
        let blocks = self.blocks.borrow();
        for (i, block) in blocks.iter().enumerate() {
            if block.contains(ptr) {
                return Some(i);
            }
        }
        None
    }

    /// Write a value to a cell within a GC-managed object, automatically triggering a write barrier.
    pub fn write<P: Trace + 'static, T: Trace + 'static>(&self, parent: Gc<P>, cell: &GcCell<T>, value: T) {
        cell.set(value);
        unsafe {
            let parent_header = &parent.ptr.as_ref().header;
            // Generational barrier: if parent is old, mark its card as dirty.
            if parent_header.generation.get() > 0 {
                if let Some(block_idx) = self.find_block(parent.ptr.as_ptr() as *const u8) {
                    self.blocks.borrow()[block_idx].mark_dirty(parent.ptr.as_ptr() as *const u8);
                }
            }
            // Incremental barrier: if GC is marking and parent is black, ensure invariant holds.
            // We use a "Yuasa-style" or "Dijkstra-style" barrier. Dijkstra style: turn child gray.
            if self.state.get() == GcState::Marking && parent_header.color.get() == Color::Black {
                let mut gray_stack = self.gray_stack.borrow_mut();
                let ctx = MarkContext { gray_stack: &mut *gray_stack };
                // Since we don't know the child's GC pointers here (value might be an Option<Gc<T>> etc),
                // the easiest way is to mark the parent as gray again so it gets re-scanned.
                parent_header.color.set(Color::Gray);
                ctx.gray_stack.push(NonNull::new_unchecked(parent_header as *const _ as *mut _));
            }
        }
    }

    /// Write barrier: should be called when an old object is modified to point to a young object.
    pub fn write_barrier<T: Trace + 'static, U: Trace + 'static>(&self, parent: Gc<T>, child: Gc<U>) {
        unsafe {
            let parent_header = &parent.ptr.as_ref().header;
            let child_header = &child.ptr.as_ref().header;

            // Generational barrier
            if parent_header.generation.get() > 0 && child_header.generation.get() == 0 {
                if let Some(block_idx) = self.find_block(parent.ptr.as_ptr() as *const u8) {
                    self.blocks.borrow()[block_idx].mark_dirty(parent.ptr.as_ptr() as *const u8);
                }
            }

            // Incremental barrier
            if self.state.get() == GcState::Marking && parent_header.color.get() == Color::Black {
                let mut gray_stack = self.gray_stack.borrow_mut();
                let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                // Dijkstra style: turn child gray
                ctx.mark(NonNull::new_unchecked(child_header as *const GcHeader as *mut GcHeader));
            }
        }
    }

    unsafe fn drop_and_dealloc<T: Trace + 'static>(header_ptr: NonNull<GcHeader>) {
        let ptr = header_ptr.cast::<GcBox<T>>();
        // Explicitly drop the data
        std::ptr::drop_in_place(&mut (*ptr.as_ptr()).data);
        // Memory is managed by GcBlock, so we don't deallocate individual boxes here.
        // In a compacting GC, this memory would be reclaimed during compaction.
    }

    unsafe fn trace_object<T: Trace + 'static>(header_ptr: NonNull<GcHeader>, ctx: &mut MarkContext<'_>) {
        let ptr = header_ptr.cast::<GcBox<T>>();
        (*ptr.as_ptr()).data.trace(ctx);
    }

    /// Run a collection cycle.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure.
    pub unsafe fn collect<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        self.collect_major(mark_roots);
    }

    /// Major collection: collect all generations.
    pub unsafe fn collect_major<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        self.state.set(GcState::Marking);
        let mut gray_stack = self.gray_stack.borrow_mut();
        let mut ctx = MarkContext { gray_stack: &mut *gray_stack };

        // 1. Mark roots
        mark_roots(&mut ctx);

        // 2. Process gray stack
        self.process_gray_stack(&mut ctx);

        // 3. Sweep everything
        self.state.set(GcState::Sweeping);
        self.sweep_full();

        // 4. Clear card tables
        for block in self.blocks.borrow().iter() {
            block.clear_cards();
        }

        // 5. Adjust threshold
        self.threshold.set(self.allocated_bytes.get() * 2);
        self.state.set(GcState::Idle);
    }

    /// Process the gray stack until it's empty.
    unsafe fn process_gray_stack(&self, ctx: &mut MarkContext<'_>) {
        while let Some(ptr) = ctx.gray_stack.pop() {
            let header = ptr.as_ref();
            // Object is being scanned, its children will be added to gray stack
            (header.trace_object)(ptr, ctx);
            // Scanning finished, object is now black
            header.color.set(Color::Black);
        }
    }

    /// Minor collection: only collect young generation.
    pub unsafe fn collect_minor<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        let mut gray_stack = self.gray_stack.borrow_mut();
        let mut ctx = MarkContext { gray_stack: &mut *gray_stack };

        // 1. Mark roots
        mark_roots(&mut ctx);

        // 2. Mark from dirty cards in old generation (old -> young)
        let mut curr = self.old_head.get();
        while let Some(header_ptr) = curr {
            let header = header_ptr.as_ref();
            if let Some(block_idx) = self.find_block(header_ptr.as_ptr() as *const u8) {
                let block = &self.blocks.borrow()[block_idx];
                let offset = header_ptr.as_ptr() as usize - block.ptr.as_ptr() as usize;
                let card_idx = offset / CARD_SIZE;
                if block.is_card_dirty(card_idx) {
                    (header.trace_object)(header_ptr, &mut ctx);
                }
            }
            curr = header.next.get();
        }

        // 3. Process gray stack
        self.process_gray_stack(&mut ctx);

        // 4. Sweep young generation and promote survivors
        self.sweep_young();

        // 5. Clear card tables for next cycle
        for block in self.blocks.borrow().iter() {
            block.clear_cards();
        }
    }

    /// Perform a small step of garbage collection.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure if GC is in Marking state.
    pub unsafe fn step<F>(&self, work_limit: usize, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        match self.state.get() {
            GcState::Idle => {
                if self.allocated_bytes.get() > self.threshold.get() {
                    self.state.set(GcState::Marking);
                    let mut gray_stack = self.gray_stack.borrow_mut();
                    let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                    mark_roots(&mut ctx);
                }
            }
            GcState::Marking => {
                let mut gray_stack = self.gray_stack.borrow_mut();
                let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                let mut work_done = 0;
                while work_done < work_limit {
                    if let Some(ptr) = ctx.gray_stack.pop() {
                        let header = ptr.as_ref();
                        (header.trace_object)(ptr, &mut ctx);
                        header.color.set(Color::Black);
                        work_done += 1;
                    } else {
                        // Marking finished
                        self.state.set(GcState::Sweeping);
                        break;
                    }
                }
            }
            GcState::Sweeping => {
                // Sweeping is currently STW in this implementation, but we could make it incremental.
                self.sweep_full();
                for block in self.blocks.borrow().iter() {
                    block.clear_cards();
                }
                self.threshold.set(self.allocated_bytes.get() * 2);
                self.state.set(GcState::Idle);
            }
        }
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
    fn trace(&self, ctx: &mut MarkContext) {
        unsafe {
            let header_ptr = NonNull::new_unchecked(&self.ptr.as_ref().header as *const _ as *mut _);
            GcHeader::mark(header_ptr, ctx);
        }
    }
}

// Implement Trace for common types
impl Trace for i64 { fn trace(&self, _ctx: &mut MarkContext) {} }
impl Trace for f64 { fn trace(&self, _ctx: &mut MarkContext) {} }
impl Trace for bool { fn trace(&self, _ctx: &mut MarkContext) {} }
impl Trace for String { fn trace(&self, _ctx: &mut MarkContext) {} }
impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, ctx: &mut MarkContext) {
        for item in self {
            item.trace(ctx);
        }
    }
}
impl<T: Trace> Trace for Option<T> {
    fn trace(&self, ctx: &mut MarkContext) {
        if let Some(inner) = self {
            inner.trace(ctx);
        }
    }
}
