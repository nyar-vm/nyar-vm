use std::alloc::{self, Layout};
use std::cell::{Cell, UnsafeCell};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, AtomicU8, Ordering};
use std::sync::{Mutex, Arc};

const BLOCK_SIZE: usize = 1024 * 1024; // 1MB blocks
const CARD_SIZE: usize = 512;
const CARDS_PER_BLOCK: usize = BLOCK_SIZE / CARD_SIZE;

struct GcBlock {
    ptr: NonNull<u8>,
    cursor: AtomicUsize,
    /// Card table for this block. Each byte represents CARD_SIZE bytes.
    /// 0 = clean, 1 = dirty.
    card_table: Box<[AtomicU8; CARDS_PER_BLOCK]>,
}

unsafe impl Send for GcBlock {}
unsafe impl Sync for GcBlock {}

impl GcBlock {
    fn new() -> Self {
        let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
        unsafe {
            let ptr = alloc::alloc(layout);
            if ptr.is_null() {
                alloc::handle_alloc_error(layout);
            }
            
            // Initialize card table with zeros
            let mut cards = Vec::with_capacity(CARDS_PER_BLOCK);
            for _ in 0..CARDS_PER_BLOCK {
                cards.push(AtomicU8::new(0));
            }
            let card_table = cards.into_boxed_slice();
            let card_table = Box::from_raw(Box::into_raw(card_table) as *mut [AtomicU8; CARDS_PER_BLOCK]);

            Self {
                ptr: NonNull::new_unchecked(ptr),
                cursor: AtomicUsize::new(0),
                card_table,
            }
        }
    }

    fn alloc(&self, layout: Layout) -> Option<*mut u8> {
        loop {
            let cursor = self.cursor.load(Ordering::Relaxed);
            let align_offset = (self.ptr.as_ptr() as usize + cursor) % layout.align();
            let padding = if align_offset == 0 { 0 } else { layout.align() - align_offset };
            let size = layout.size();
            let new_cursor = cursor + padding + size;

            if new_cursor <= BLOCK_SIZE {
                if self.cursor.compare_exchange_weak(cursor, new_cursor, Ordering::SeqCst, Ordering::Relaxed).is_ok() {
                    return unsafe { Some(self.ptr.as_ptr().add(cursor + padding)) };
                }
            } else {
                return None;
            }
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
        self.card_table[card_idx].store(1, Ordering::Release);
    }

    fn is_card_dirty(&self, card_idx: usize) -> bool {
        self.card_table[card_idx].load(Ordering::Acquire) == 1
    }

    fn clear_cards(&self) {
        for card in self.card_table.iter() {
            card.store(0, Ordering::Release);
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
    pub next: AtomicPtr<GcHeader>,
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
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GcState {
    /// Not yet visited.
    Idle = 0,
    /// GC is currently marking objects.
    Marking = 1,
    /// GC is currently sweeping unreachable objects.
    Sweeping = 2,
}

impl GcState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => GcState::Idle,
            1 => GcState::Marking,
            2 => GcState::Sweeping,
            _ => unreachable!(),
        }
    }
}

struct SweepState {
    young_curr: AtomicPtr<GcHeader>,
    young_prev: AtomicPtr<GcHeader>,
    old_curr: AtomicPtr<GcHeader>,
    old_prev: AtomicPtr<GcHeader>,
}

pub struct NyarGc {
    /// Head of the linked list of young generation objects.
    young_head: AtomicPtr<GcHeader>,
    /// Head of the linked list of old generation objects.
    old_head: AtomicPtr<GcHeader>,
    /// Memory blocks managed by the GC.
    blocks: Mutex<Vec<Arc<GcBlock>>>,
    /// Gray stack for tri-color marking.
    gray_stack: Mutex<Vec<NonNull<GcHeader>>>,
    /// Current state of the GC.
    state: AtomicU8,
    sweep_state: SweepState,
    /// Total number of bytes allocated.
    allocated_bytes: AtomicUsize,
    /// Threshold for the next collection cycle.
    threshold: AtomicUsize,
}

pub struct Tlab<'a> {
    gc: &'a NyarGc,
    block: Option<Arc<GcBlock>>,
}

impl<'a> Tlab<'a> {
    pub fn new(gc: &'a NyarGc) -> Self {
        Self { gc, block: None }
    }

    pub fn alloc<T: Trace + 'static>(&mut self, value: T) -> Gc<T> {
        let layout = Layout::new::<GcBox<T>>();
        
        // 1. Try to allocate from current block
        if let Some(ref block) = self.block {
            if let Some(ptr) = block.alloc(layout) {
                return unsafe { self.gc.init_gc_box(ptr as *mut GcBox<T>, value, layout) };
            }
        }

        // 2. Current block is full or missing, get a new block from GC
        self.new_block();
        
        let ptr = self.block.as_ref().unwrap().alloc(layout).expect("Allocation failed even in new block");
        unsafe { self.gc.init_gc_box(ptr as *mut GcBox<T>, value, layout) }
    }

    fn new_block(&mut self) {
        let block = Arc::new(GcBlock::new());
        self.block = Some(block.clone());
        let mut blocks = self.gc.blocks.lock().unwrap();
        blocks.push(block);
    }
}

impl NyarGc {
    /// Internal helper to initialize a newly allocated GcBox and link it to the young generation.
    unsafe fn init_gc_box<T: Trace + 'static>(&self, ptr: *mut GcBox<T>, value: T, layout: Layout) -> Gc<T> {
        let state = self.state.load(Ordering::Acquire);
        let color = if state != GcState::Idle as u8 {
            Color::Black
        } else {
            Color::White
        };
        
        // Insert into young_head atomically
        let mut old_head = self.young_head.load(Ordering::Acquire);
        loop {
            std::ptr::write(
                &mut (*ptr).header,
                GcHeader {
                    color: Cell::new(color),
                    generation: Cell::new(0),
                    next: AtomicPtr::new(old_head),
                    drop_and_dealloc: Self::drop_and_dealloc::<T>,
                    trace_object: Self::trace_object::<T>,
                    size: layout.size(),
                },
            );
            match self.young_head.compare_exchange_weak(
                old_head,
                &mut (*ptr).header as *mut _,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => old_head = actual,
            }
        }

        std::ptr::write(&mut (*ptr).data, value);

        let gc_box = NonNull::new_unchecked(ptr);
        let allocated = self.allocated_bytes.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();

        if allocated > self.threshold.load(Ordering::Relaxed) {
            self.collect_minor(|_| {});
        }

        Gc { ptr: gc_box }
    }
    pub fn new() -> Self {
        Self {
            young_head: AtomicPtr::new(std::ptr::null_mut()),
            old_head: AtomicPtr::new(std::ptr::null_mut()),
            blocks: Mutex::new(vec![Arc::new(GcBlock::new())]),
            gray_stack: Mutex::new(Vec::new()),
            state: AtomicU8::new(GcState::Idle as u8),
            sweep_state: SweepState {
                young_curr: AtomicPtr::new(std::ptr::null_mut()),
                young_prev: AtomicPtr::new(std::ptr::null_mut()),
                old_curr: AtomicPtr::new(std::ptr::null_mut()),
                old_prev: AtomicPtr::new(std::ptr::null_mut()),
            },
            allocated_bytes: AtomicUsize::new(0),
            threshold: AtomicUsize::new(1024 * 1024), // 1MB default threshold
        }
    }

    /// Allocate a new value on the managed heap.
    pub fn alloc<T: Trace + 'static>(&self, value: T) -> Gc<T> {
        let layout = Layout::new::<GcBox<T>>();
        let ptr = {
            let mut blocks = self.blocks.lock().unwrap();
            let mut ptr = blocks.last().unwrap().alloc(layout);
            if ptr.is_none() {
                blocks.push(Arc::new(GcBlock::new()));
                ptr = blocks.last().unwrap().alloc(layout);
            }
            ptr.unwrap() as *mut GcBox<T>
        };

        unsafe { self.init_gc_box(ptr, value, layout) }
    }

    fn find_block(&self, ptr: *const u8) -> Option<usize> {
        let blocks = self.blocks.lock().unwrap();
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
                    self.blocks.lock().unwrap()[block_idx].mark_dirty(parent.ptr.as_ptr() as *const u8);
                }
            }
            // Incremental barrier: if GC is marking and parent is black, ensure invariant holds.
            // We use a "Yuasa-style" or "Dijkstra-style" barrier. Dijkstra style: turn child gray.
            if self.state.load(Ordering::Acquire) == GcState::Marking as u8 && parent_header.color.get() == Color::Black {
                let mut gray_stack = self.gray_stack.lock().unwrap();
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
                    self.blocks.lock().unwrap()[block_idx].mark_dirty(parent.ptr.as_ptr() as *const u8);
                }
            }

            // Incremental barrier
            if self.state.load(Ordering::Acquire) == GcState::Marking as u8 && parent_header.color.get() == Color::Black {
                let mut gray_stack = self.gray_stack.lock().unwrap();
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
        self.state.store(GcState::Marking as u8, Ordering::Release);
        let mut gray_stack = self.gray_stack.lock().unwrap();
        let mut ctx = MarkContext { gray_stack: &mut *gray_stack };

        // 1. Mark roots
        mark_roots(&mut ctx);

        // 2. Process gray stack
        self.process_gray_stack(&mut ctx);

        // 3. Sweep everything
        self.state.store(GcState::Sweeping as u8, Ordering::Release);
        self.sweep_full();

        // 4. Clear card tables
        for block in self.blocks.lock().unwrap().iter() {
            block.clear_cards();
        }

        // 5. Adjust threshold
        self.threshold.store(self.allocated_bytes.load(Ordering::Relaxed) * 2, Ordering::Relaxed);
        self.state.store(GcState::Idle as u8, Ordering::Release);
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
        self.state.store(GcState::Marking as u8, Ordering::Release);
        let mut gray_stack = self.gray_stack.lock().unwrap();
        let mut ctx = MarkContext { gray_stack: &mut *gray_stack };

        // 1. Mark roots
        mark_roots(&mut ctx);

        // 2. Mark from dirty cards in old generation (old -> young)
        let mut curr = self.old_head.load(Ordering::Relaxed);
        while let Some(header_ptr) = NonNull::new(curr) {
            let header = header_ptr.as_ref();
            if let Some(block_idx) = self.find_block(header_ptr.as_ptr() as *const u8) {
                let blocks = self.blocks.lock().unwrap();
                let block = &blocks[block_idx];
                let offset = header_ptr.as_ptr() as usize - block.ptr.as_ptr() as usize;
                let card_idx = offset / CARD_SIZE;
                if block.is_card_dirty(card_idx) {
                    (header.trace_object)(header_ptr, &mut ctx);
                }
            }
            curr = header.next.load(Ordering::Relaxed);
        }

        // 3. Process gray stack
        self.process_gray_stack(&mut ctx);

        // 4. Sweep young generation and promote survivors
        self.state.store(GcState::Sweeping as u8, Ordering::Release);
        self.sweep_young();

        // 5. Clear card tables for next cycle
        for block in self.blocks.lock().unwrap().iter() {
            block.clear_cards();
        }
        self.state.store(GcState::Idle as u8, Ordering::Release);
    }

    /// Perform a small step of garbage collection.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure if GC is in Marking state.
    pub unsafe fn step<F>(&self, work_limit: usize, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        match GcState::from_u8(self.state.load(Ordering::Acquire)) {
            GcState::Idle => {
                if self.allocated_bytes.load(Ordering::Relaxed) > self.threshold.load(Ordering::Relaxed) {
                    self.state.store(GcState::Marking as u8, Ordering::Release);
                    let mut gray_stack = self.gray_stack.lock().unwrap();
                    let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                    mark_roots(&mut ctx);
                }
            }
            GcState::Marking => {
                let mut gray_stack = self.gray_stack.lock().unwrap();
                let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                let mut work_done = 0;
                while work_done < work_limit {
                    if let Some(ptr) = ctx.gray_stack.pop() {
                        let header = ptr.as_ref();
                        (header.trace_object)(ptr, &mut ctx);
                        header.color.set(Color::Black);
                        work_done += 1;
                    } else {
                        // Marking finished, start sweeping
                        self.state.store(GcState::Sweeping as u8, Ordering::Release);
                        self.sweep_state.young_curr.store(self.young_head.load(Ordering::Acquire), Ordering::Release);
                        self.sweep_state.young_prev.store(std::ptr::null_mut(), Ordering::Release);
                        self.sweep_state.old_curr.store(self.old_head.load(Ordering::Acquire), Ordering::Release);
                        self.sweep_state.old_prev.store(std::ptr::null_mut(), Ordering::Release);
                        break;
                    }
                }
            }
            GcState::Sweeping => {
                let mut work_done = 0;
                while work_done < work_limit {
                    // 1. Sweep young generation
                    let young_curr_ptr = self.sweep_state.young_curr.load(Ordering::Acquire);
                    if let Some(header_ptr) = NonNull::new(young_curr_ptr) {
                        let header = header_ptr.as_ref();
                        let next = NonNull::new(header.next.load(Ordering::Acquire));

                        if header.color.get() != Color::White {
                            // Object survived! Promote to old generation.
                            header.color.set(Color::White);
                            header.generation.set(1);

                            // Remove from young list
                            let young_prev_ptr = self.sweep_state.young_prev.load(Ordering::Acquire);
                            if let Some(mut p) = NonNull::new(young_prev_ptr) {
                                p.as_mut().next.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                            } else {
                                self.young_head.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                            }

                            // Add to old list
                            header.next.store(self.old_head.load(Ordering::Acquire), Ordering::Release);
                            self.old_head.store(header_ptr.as_ptr(), Ordering::Release);

                            // Since we removed it from the current list, prev doesn't change
                            self.sweep_state.young_curr.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                        } else {
                            // Object is unreachable, free it
                            let young_prev_ptr = self.sweep_state.young_prev.load(Ordering::Acquire);
                            if let Some(mut p) = NonNull::new(young_prev_ptr) {
                                p.as_mut().next.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                            } else {
                                self.young_head.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                            }

                            // Update allocated_bytes
                            self.allocated_bytes.fetch_sub(header.size, Ordering::SeqCst);
                            (header.drop_and_dealloc)(header_ptr);
                            self.sweep_state.young_curr.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                        }
                        work_done += 1;
                    }
                    // 2. Sweep old generation
                    else {
                        let old_curr_ptr = self.sweep_state.old_curr.load(Ordering::Acquire);
                        if let Some(header_ptr) = NonNull::new(old_curr_ptr) {
                            let header = header_ptr.as_ref();
                            let next = NonNull::new(header.next.load(Ordering::Acquire));

                            if header.color.get() != Color::White {
                                header.color.set(Color::White);
                                self.sweep_state.old_prev.store(header_ptr.as_ptr(), Ordering::Release);
                                self.sweep_state.old_curr.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                            } else {
                                let old_prev_ptr = self.sweep_state.old_prev.load(Ordering::Acquire);
                                if let Some(mut p) = NonNull::new(old_prev_ptr) {
                                    p.as_mut().next.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                                } else {
                                    self.old_head.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                                }

                                self.allocated_bytes.fetch_sub(header.size, Ordering::SeqCst);
                                (header.drop_and_dealloc)(header_ptr);
                                self.sweep_state.old_curr.store(next.map_or(std::ptr::null_mut(), |n| n.as_ptr()), Ordering::Release);
                            }
                            work_done += 1;
                        } else {
                            // Sweeping finished
                            for block in self.blocks.lock().unwrap().iter() {
                                block.clear_cards();
                            }
                            self.threshold.store(self.allocated_bytes.load(Ordering::Relaxed) * 2, Ordering::Relaxed);
                            self.state.store(GcState::Idle as u8, Ordering::Release);
                            break;
                        }
                    }
                }
            }
        }
    }

    unsafe fn sweep_young(&self) {
        let mut prev: Option<NonNull<GcHeader>> = None;
        let mut curr = NonNull::new(self.young_head.load(Ordering::Acquire));

        while let Some(header_ptr) = curr {
            let header = header_ptr.as_ref();
            let next = NonNull::new(header.next.load(Ordering::Acquire));

            if header.color.get() != Color::White {
                // Object survived! Promote to old generation.
                header.color.set(Color::White);
                header.generation.set(1);

                // Remove from young list
                if let Some(mut p) = prev {
                    p.as_mut().next.store(next.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()), Ordering::Release);
                } else {
                    self.young_head.store(next.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()), Ordering::Release);
                }

                // Add to old list
                let mut old_head = self.old_head.load(Ordering::Acquire);
                loop {
                    header.next.store(old_head, Ordering::Release);
                    match self.old_head.compare_exchange_weak(
                        old_head,
                        header_ptr.as_ptr(),
                        Ordering::Release,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(actual) => old_head = actual,
                    }
                }

                // Since we removed it from the current list, prev doesn't change
                curr = next;
            } else {
                // Object is unreachable, free it
                if let Some(mut p) = prev {
                    p.as_mut().next.store(next.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()), Ordering::Release);
                } else {
                    self.young_head.store(next.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()), Ordering::Release);
                }

                // Update allocated_bytes
                self.allocated_bytes.fetch_sub(header.size, Ordering::SeqCst);
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
        let mut curr = NonNull::new(self.old_head.load(Ordering::Acquire));

        while let Some(header_ptr) = curr {
            let header = header_ptr.as_ref();
            let next = NonNull::new(header.next.load(Ordering::Acquire));

            if header.color.get() != Color::White {
                header.color.set(Color::White);
                prev = Some(header_ptr);
                curr = next;
            } else {
                if let Some(mut p) = prev {
                    p.as_mut().next.store(next.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()), Ordering::Release);
                } else {
                    self.old_head.store(next.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut()), Ordering::Release);
                }

                // Update allocated_bytes
                self.allocated_bytes.fetch_sub(header.size, Ordering::SeqCst);
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
