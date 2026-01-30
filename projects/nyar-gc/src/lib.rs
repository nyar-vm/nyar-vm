use std::alloc::{self, Layout};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, Arc};

#[repr(transparent)]
pub struct SendPtr<T>(pub NonNull<T>);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

impl<T> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<T> Copy for SendPtr<T> {}
impl<T> std::ops::Deref for SendPtr<T> {
    type Target = NonNull<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


const BLOCK_SIZE: usize = 1024 * 1024; // 1MB blocks
const TLAB_SIZE: usize = 64 * 1024; // 64KB TLAB
const CARD_SIZE: usize = 512;
const CARDS_PER_BLOCK: usize = BLOCK_SIZE / CARD_SIZE;
const CARD_BITMAP_WORDS: usize = CARDS_PER_BLOCK / 64;

struct Tlab {
    start: *mut u8,
    cursor: *mut u8,
    end: *mut u8,
}

impl Tlab {
    const fn new() -> Self {
        Self {
            start: std::ptr::null_mut(),
            cursor: std::ptr::null_mut(),
            end: std::ptr::null_mut(),
        }
    }

    fn alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        let cursor = self.cursor as usize;
        let align_offset = cursor % layout.align();
        let padding = if align_offset == 0 { 0 } else { layout.align() - align_offset };
        let size = layout.size();
        let new_cursor = cursor + padding + size;

        if new_cursor <= self.end as usize {
            let ptr = (cursor + padding) as *mut u8;
            self.cursor = new_cursor as *mut u8;
            Some(ptr)
        } else {
            None
        }
    }
}

thread_local! {
    static THREAD_TLAB: UnsafeCell<Tlab> = UnsafeCell::new(Tlab::new());
}

struct GcBlockHeader {
    cursor: AtomicUsize,
    live_bytes: AtomicUsize,
    /// Card table for this block. Each bit represents CARD_SIZE bytes.
    /// 1 = dirty, 0 = clean.
    card_table: [AtomicU64; CARD_BITMAP_WORDS],
}

struct GcBlock {
    ptr: NonNull<u8>,
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
            
            let header = ptr as *mut GcBlockHeader;
            (*header).cursor.store(std::mem::size_of::<GcBlockHeader>(), Ordering::Relaxed);
            (*header).live_bytes.store(0, Ordering::Relaxed);
            for word in (*header).card_table.iter() {
                word.store(0, Ordering::Relaxed);
            }

            Self {
                ptr: NonNull::new_unchecked(ptr),
            }
        }
    }

    fn get_header(&self) -> &GcBlockHeader {
        unsafe { &*(self.ptr.as_ptr() as *const GcBlockHeader) }
    }

    fn alloc(&self, layout: Layout) -> Option<*mut u8> {
        let header = self.get_header();
        loop {
            let cursor = header.cursor.load(Ordering::Relaxed);
            let align_offset = (self.ptr.as_ptr() as usize + cursor) % layout.align();
            let padding = if align_offset == 0 { 0 } else { layout.align() - align_offset };
            let size = layout.size();
            let new_cursor = cursor + padding + size;

            if new_cursor <= BLOCK_SIZE {
                if header.cursor.compare_exchange_weak(cursor, new_cursor, Ordering::SeqCst, Ordering::Relaxed).is_ok() {
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

    fn mark_dirty(ptr: *const u8) {
        let base = (ptr as usize) & !(BLOCK_SIZE - 1);
        let header = base as *const GcBlockHeader;
        let offset = ptr as usize - base;
        let card_idx = offset / CARD_SIZE;
        let word_idx = card_idx / 64;
        let bit_idx = card_idx % 64;
        unsafe {
            (*header).card_table[word_idx].fetch_or(1 << bit_idx, Ordering::Release);
        }
    }

    fn clear_cards(&self) {
        let header = self.get_header();
        for word in header.card_table.iter() {
            word.store(0, Ordering::Release);
        }
    }

    fn add_live_bytes(&self, bytes: usize) {
        self.get_header().live_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    fn reset_live_bytes(&self) {
        self.get_header().live_bytes.store(0, Ordering::Relaxed);
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
    pub(crate) gray_stack: &'a mut Vec<SendPtr<GcHeader>>,
}

impl<'a> MarkContext<'a> {
    /// Mark a GC pointer as reachable.
    pub unsafe fn mark(&mut self, ptr: NonNull<GcHeader>) {
        let header = ptr.as_ref();
        if header.get_color() == Color::White {
            header.set_color(Color::Gray);
            self.gray_stack.push(SendPtr(ptr));
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
    pub color: AtomicU8,
    /// Which generation this object belongs to (0 for young, 1 for old).
    pub(crate) generation: AtomicU8,
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
    pub fn get_color(&self) -> Color {
        match self.color.load(Ordering::Acquire) {
            0 => Color::White,
            1 => Color::Gray,
            2 => Color::Black,
            _ => unreachable!(),
        }
    }

    pub fn set_color(&self, color: Color) {
        self.color.store(color as u8, Ordering::Release);
    }

    /// Mark the object and trace its children if it wasn't already marked.
    pub unsafe fn mark(ptr: NonNull<GcHeader>, ctx: &mut MarkContext<'_>) {
        let header = ptr.as_ref();
        if header.get_color() == Color::White {
            header.set_color(Color::Gray);
            ctx.gray_stack.push(SendPtr(ptr));
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

unsafe impl<T: Trace + 'static> Send for Gc<T> {}
unsafe impl<T: Trace + 'static> Sync for Gc<T> {}

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
    gray_stack: Mutex<Vec<SendPtr<GcHeader>>>,
    /// Current state of the GC.
    state: AtomicU8,
    sweep_state: Mutex<SweepState>,
    /// Total number of bytes allocated.
    allocated_bytes: AtomicUsize,
    /// Threshold for the next collection cycle.
    threshold: AtomicUsize,
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
                    color: AtomicU8::new(color as u8),
                    generation: AtomicU8::new(0),
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
            sweep_state: Mutex::new(SweepState {
                young_curr: AtomicPtr::new(std::ptr::null_mut()),
                young_prev: AtomicPtr::new(std::ptr::null_mut()),
                old_curr: AtomicPtr::new(std::ptr::null_mut()),
                old_prev: AtomicPtr::new(std::ptr::null_mut()),
            }),
            allocated_bytes: AtomicUsize::new(0),
            threshold: AtomicUsize::new(1024 * 1024), // 1MB default threshold
        }
    }

    fn get_state(&self) -> GcState {
        match self.state.load(Ordering::Acquire) {
            0 => GcState::Idle,
            1 => GcState::Marking,
            2 => GcState::Sweeping,
            _ => unreachable!(),
        }
    }

    fn set_state(&self, state: GcState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Allocate a new value on the managed heap.
    pub fn alloc<T: Trace + 'static>(&self, value: T) -> Gc<T> {
        let layout = Layout::new::<GcBox<T>>();

        // Try to allocate from TLAB first
        let ptr = THREAD_TLAB.with(|tlab_cell| {
            let tlab = unsafe { &mut *tlab_cell.get() };
            if let Some(ptr) = tlab.alloc(layout) {
                ptr
            } else {
                // TLAB exhausted, try to refill
                self.refill_tlab(tlab, layout)
            }
        });

        unsafe { self.init_gc_box(ptr as *mut GcBox<T>, value, layout) }
    }

    fn refill_tlab(&self, tlab: &mut Tlab, layout: Layout) -> *mut u8 {
        // Try to get a new chunk from existing blocks
        let tlab_layout = Layout::from_size_align(TLAB_SIZE, 8).unwrap();

        {
            let blocks = self.blocks.lock().unwrap();
            for block in blocks.iter() {
                if let Some(ptr) = block.alloc(tlab_layout) {
                    tlab.start = ptr;
                    tlab.cursor = ptr;
                    tlab.end = unsafe { ptr.add(TLAB_SIZE) };
                    return tlab.alloc(layout).unwrap();
                }
            }
        }

        // No space in existing blocks, allocate a new block
        let new_block = Arc::new(GcBlock::new());
        let ptr = new_block.alloc(tlab_layout).unwrap();
        {
            let mut blocks = self.blocks.lock().unwrap();
            blocks.push(new_block);
        }

        tlab.start = ptr;
        tlab.cursor = ptr;
        tlab.end = unsafe { ptr.add(TLAB_SIZE) };
        tlab.alloc(layout).unwrap()
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
            if parent_header.generation.load(Ordering::Acquire) > 0 {
                GcBlock::mark_dirty(parent.ptr.as_ptr() as *const u8);
            }
            // Incremental barrier: if GC is marking and parent is black, ensure invariant holds.
            if self.get_state() == GcState::Marking && parent_header.get_color() == Color::Black {
                let mut gray_stack = self.gray_stack.lock().unwrap();
                let ctx = MarkContext { gray_stack: &mut *gray_stack };
                // Steele style: turn parent gray
                parent_header.set_color(Color::Gray);
                ctx.gray_stack.push(SendPtr(NonNull::new_unchecked(parent_header as *const _ as *mut _)));
            }
        }
    }

    /// Write barrier: should be called when an old object is modified to point to a young object.
    pub fn write_barrier<T: Trace + 'static, U: Trace + 'static>(&self, parent: Gc<T>, child: Gc<U>) {
        unsafe {
            let parent_header = &parent.ptr.as_ref().header;
            let child_header = &child.ptr.as_ref().header;

            // Generational barrier
            if parent_header.generation.load(Ordering::Acquire) > 0 && child_header.generation.load(Ordering::Acquire) == 0 {
                GcBlock::mark_dirty(parent.ptr.as_ptr() as *const u8);
            }

            // Incremental barrier
            if self.get_state() == GcState::Marking && parent_header.get_color() == Color::Black {
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
        self.set_state(GcState::Marking);
        let mut gray_stack = self.gray_stack.lock().unwrap();
        let mut ctx = MarkContext { gray_stack: &mut *gray_stack };

        // 1. Mark roots
        mark_roots(&mut ctx);

        // 2. Process gray stack
        self.process_gray_stack(&mut ctx);

        // 3. Sweep everything
        self.set_state(GcState::Sweeping);
        self.sweep_full();

        // 4. Clear card tables
        for block in self.blocks.lock().unwrap().iter() {
            block.clear_cards();
        }

        // 5. Adjust threshold
        self.threshold.store(self.allocated_bytes.load(Ordering::Relaxed) * 2, Ordering::Relaxed);
        self.set_state(GcState::Idle);
    }

    /// Process the gray stack until it's empty.
    unsafe fn process_gray_stack(&self, ctx: &mut MarkContext<'_>) {
        while let Some(ptr) = ctx.gray_stack.pop() {
            let header = ptr.as_ref();
            // Object is being scanned, its children will be added to gray stack
            (header.trace_object)(ptr.0, ctx);
            // Scanning finished, object is now black
            header.set_color(Color::Black);
        }
    }

    /// Minor collection: only collect young generation.
    pub unsafe fn collect_minor<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        self.set_state(GcState::Marking);
        let mut gray_stack = self.gray_stack.lock().unwrap();
        let mut ctx = MarkContext { gray_stack: &mut *gray_stack };

        // 1. Mark roots
        mark_roots(&mut ctx);

        // 2. Mark from dirty cards in old generation (old -> young)
        let mut curr = self.old_head.load(Ordering::Relaxed);
        let mut last_card_idx = usize::MAX;
        let mut last_block_base = usize::MAX;
        let mut is_last_card_dirty = false;
        let mut is_last_block_dirty = true;

        while let Some(header_ptr) = NonNull::new(curr) {
            let header = header_ptr.as_ref();
            let addr = header_ptr.as_ptr() as usize;
            let base = addr & !(BLOCK_SIZE - 1);
            let offset = addr - base;
            let card_idx = offset / CARD_SIZE;

            if base != last_block_base {
                last_block_base = base;
                last_card_idx = card_idx;
                let block_header = base as *const GcBlockHeader;
                
                // Check if the whole block is clean
                unsafe {
                    is_last_block_dirty = false;
                    for word in (*block_header).card_table.iter() {
                        if word.load(Ordering::Acquire) != 0 {
                            is_last_block_dirty = true;
                            break;
                        }
                    }
                }

                if is_last_block_dirty {
                    let word_idx = card_idx / 64;
                    let bit_idx = card_idx % 64;
                    unsafe {
                        is_last_card_dirty = ((*block_header).card_table[word_idx].load(Ordering::Acquire) & (1 << bit_idx)) != 0;
                    }
                } else {
                    is_last_card_dirty = false;
                }
            } else if card_idx != last_card_idx {
                last_card_idx = card_idx;
                if is_last_block_dirty {
                    let block_header = base as *const GcBlockHeader;
                    let word_idx = card_idx / 64;
                    let bit_idx = card_idx % 64;
                    unsafe {
                        is_last_card_dirty = ((*block_header).card_table[word_idx].load(Ordering::Acquire) & (1 << bit_idx)) != 0;
                    }
                } else {
                    is_last_card_dirty = false;
                }
            }

            if is_last_card_dirty {
                unsafe { (header.trace_object)(header_ptr, &mut ctx); }
            }
            curr = header.next.load(Ordering::Relaxed);
        }

        // 3. Process gray stack
        self.process_gray_stack(&mut ctx);

        // 4. Sweep young generation and promote survivors
        self.set_state(GcState::Sweeping);
        self.sweep_young();

        // 5. Clear card tables for next cycle
        for block in self.blocks.lock().unwrap().iter() {
            block.clear_cards();
        }
        self.set_state(GcState::Idle);
    }

    /// Perform a small step of garbage collection.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure if GC is in Marking state.
    pub unsafe fn step<F>(&self, work_limit: usize, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        match self.get_state() {
            GcState::Idle => {
                self.set_state(GcState::Marking);
                let sweep = self.sweep_state.lock().unwrap();
                sweep.young_curr.store(self.young_head.load(Ordering::Acquire), Ordering::Release);
                sweep.young_prev.store(std::ptr::null_mut(), Ordering::Release);
                sweep.old_curr.store(self.old_head.load(Ordering::Acquire), Ordering::Release);
                sweep.old_prev.store(std::ptr::null_mut(), Ordering::Release);

                // Reset live bytes for all blocks before marking
                {
                    let blocks = self.blocks.lock().unwrap();
                    for block in blocks.iter() {
                        block.reset_live_bytes();
                    }
                }

                // Initial marking from roots
                let mut gray_stack = self.gray_stack.lock().unwrap();
                let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                mark_roots(&mut ctx);
            }
            GcState::Marking => {
                let mut gray_stack = self.gray_stack.lock().unwrap();
                let mut ctx = MarkContext { gray_stack: &mut *gray_stack };
                let mut work_done = 0;
                while work_done < work_limit && !ctx.gray_stack.is_empty() {
                    self.process_gray_stack(&mut ctx);
                    work_done += 1;
                }
                if ctx.gray_stack.is_empty() {
                    self.set_state(GcState::Sweeping);
                }
            }
            GcState::Sweeping => {
                let mut work_done = 0;
                let sweep = self.sweep_state.lock().unwrap();
                while work_done < work_limit {
                    // 1. Sweep young generation
                    let young_curr_ptr = sweep.young_curr.load(Ordering::Acquire);
                    if let Some(header_ptr) = NonNull::new(young_curr_ptr) {
                        let header = header_ptr.as_ref();
                        let next = header.next.load(Ordering::Acquire);

                        if header.get_color() != Color::White {
                            // Survive and promote
                            header.set_color(Color::White);
                            header.generation.store(1, Ordering::Release);

                            // Update live bytes
                            let addr = header_ptr.as_ptr() as usize;
                            let base = addr & !(BLOCK_SIZE - 1);
                            let block_header = base as *const GcBlockHeader;
                            unsafe {
                                (*block_header).live_bytes.fetch_add(header.size, Ordering::Relaxed);
                            }

                            // Remove from young
                            let young_prev_ptr = sweep.young_prev.load(Ordering::Acquire);
                            if let Some(mut p) = NonNull::new(young_prev_ptr) {
                                p.as_mut().next.store(next, Ordering::Release);
                            } else {
                                self.young_head.store(next, Ordering::Release);
                            }

                            // Add to old
                            let mut old_head = self.old_head.load(Ordering::Acquire);
                            loop {
                                header.next.store(old_head, Ordering::Release);
                                match self.old_head.compare_exchange_weak(
                                     old_head,
                                    young_curr_ptr,
                                    Ordering::Release,
                                    Ordering::Acquire,
                                ) {
                                    Ok(_) => break,
                                    Err(actual) => old_head = actual,
                                }
                            }
                            sweep.young_curr.store(next, Ordering::Release);
                        } else {
                            // Free
                            let young_prev_ptr = sweep.young_prev.load(Ordering::Acquire);
                            if let Some(mut p) = NonNull::new(young_prev_ptr) {
                                p.as_mut().next.store(next, Ordering::Release);
                            } else {
                                self.young_head.store(next, Ordering::Release);
                            }
                            self.allocated_bytes.fetch_sub(header.size, Ordering::SeqCst);
                            (header.drop_and_dealloc)(header_ptr);
                            sweep.young_curr.store(next, Ordering::Release);
                        }
                        work_done += 1;
                    } else {
                        // 2. Sweep old generation
                        let old_curr_ptr = sweep.old_curr.load(Ordering::Acquire);
                        if let Some(header_ptr) = NonNull::new(old_curr_ptr) {
                            let header = header_ptr.as_ref();
                            let next = header.next.load(Ordering::Acquire);

                            if header.get_color() != Color::White {
                                header.set_color(Color::White);

                                // Update live bytes
                                let addr = header_ptr.as_ptr() as usize;
                                let base = addr & !(BLOCK_SIZE - 1);
                                let block_header = base as *const GcBlockHeader;
                                unsafe {
                                    (*block_header).live_bytes.fetch_add(header.size, Ordering::Relaxed);
                                }

                                sweep.old_prev.store(old_curr_ptr, Ordering::Release);
                                sweep.old_curr.store(next, Ordering::Release);
                            } else {
                                let old_prev_ptr = sweep.old_prev.load(Ordering::Acquire);
                                if let Some(mut p) = NonNull::new(old_prev_ptr) {
                                    p.as_mut().next.store(next, Ordering::Release);
                                } else {
                                    self.old_head.store(next, Ordering::Release);
                                }
                                self.allocated_bytes.fetch_sub(header.size, Ordering::SeqCst);
                                (header.drop_and_dealloc)(header_ptr);
                                sweep.old_curr.store(next, Ordering::Release);
                            }
                            work_done += 1;
                        } else {
                            // Finished sweeping
                            self.reclaim_empty_blocks();
                            self.threshold.store(self.allocated_bytes.load(Ordering::Relaxed) * 2, Ordering::Relaxed);
                            self.set_state(GcState::Idle);
                            break;
                        }
                    }
                }
            }
        }
    }

    fn reclaim_empty_blocks(&self) {
        let mut blocks = self.blocks.lock().unwrap();
        if blocks.len() <= 1 {
            return;
        }

        // Keep at least one block, and don't reclaim the last block if it's still being used
        let mut i = 0;
        while i < blocks.len() {
            let block = &blocks[i];
            let header = block.get_header();
            // If block is empty and not the last one (potentially active)
            if header.live_bytes.load(Ordering::Relaxed) == 0 && i < blocks.len() - 1 {
                blocks.remove(i);
            } else {
                i += 1;
            }
        }
    }

    unsafe fn sweep_young(&self) {
        // Reset live bytes for all blocks before sweeping
        {
            let blocks = self.blocks.lock().unwrap();
            for block in blocks.iter() {
                block.reset_live_bytes();
            }
        }

        let mut curr = self.young_head.load(Ordering::Acquire);

        while let Some(header_ptr) = NonNull::new(curr) {
            let header = header_ptr.as_ref();
            let next = header.next.load(Ordering::Acquire);

            if header.get_color() != Color::White {
                // Object survived! Promote to old generation.
                header.set_color(Color::White);
                header.generation.store(1, Ordering::Release);
                
                // Update live bytes in the block it belongs to
                if let Some(block_idx) = self.find_block(header_ptr.as_ptr() as *const u8) {
                    let blocks = self.blocks.lock().unwrap();
                    blocks[block_idx].add_live_bytes(header.size);
                }

                // Remove from young list (always head since we promote everything)
                self.young_head.store(next, Ordering::Release);

                // Add to old list
                let mut old_head = self.old_head.load(Ordering::Acquire);
                loop {
                    header.next.store(old_head, Ordering::Release);
                    match self.old_head.compare_exchange_weak(
                        old_head,
                        curr,
                        Ordering::Release,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(actual) => old_head = actual,
                    }
                }
                curr = next;
            } else {
                // Object is unreachable, free it
                self.young_head.store(next, Ordering::Release);

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

        let mut prev: *mut GcHeader = std::ptr::null_mut();
        let mut curr = self.old_head.load(Ordering::Acquire);

        while let Some(header_ptr) = NonNull::new(curr) {
            let header = header_ptr.as_ref();
            let next = header.next.load(Ordering::Acquire);

            if header.get_color() != Color::White {
                header.set_color(Color::White);
                
                // Update live bytes in the block it belongs to
                if let Some(block_idx) = self.find_block(header_ptr.as_ptr() as *const u8) {
                    let blocks = self.blocks.lock().unwrap();
                    blocks[block_idx].add_live_bytes(header.size);
                }

                prev = curr;
                curr = next;
            } else {
                if let Some(mut p) = NonNull::new(prev) {
                    p.as_mut().next.store(next, Ordering::Release);
                } else {
                    self.old_head.store(next, Ordering::Release);
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
