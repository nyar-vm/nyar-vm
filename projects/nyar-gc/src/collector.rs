use crate::block::{GcBlock, GcBlockHeader, BLOCK_SIZE, GC_BLOCK_MAGIC};
use crate::object::{
    Gc, GcBox, GcCell, GcHeader, GcState, GcVTable, LargeObjectHeader, MarkContext, Trace,
};
use crate::ptr::SendPtr;
use crate::tlab::{Tlab, TLAB_SIZE};
use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

pub static VTABLE_REGISTRY: [AtomicPtr<GcVTable>; 65536] =
    [const { AtomicPtr::new(std::ptr::null_mut()) }; 65536];
pub static VTABLE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub const SIZE_CLASSES: [usize; 10] = [16, 32, 48, 64, 128, 256, 512, 1024, 2048, 4096];

pub struct FreeNode {
    pub next: *mut FreeNode,
}

pub struct SweepState {
    pub block_curr: AtomicPtr<GcBlockHeader>,
    pub block_cursor: AtomicUsize,
    pub large_curr: AtomicPtr<LargeObjectHeader>,
    pub large_prev: AtomicPtr<LargeObjectHeader>,
}

pub struct NyarGc {
    /// Head of the linked list of large objects.
    pub large_head: AtomicPtr<LargeObjectHeader>,
    /// Memory blocks managed by the GC (lock-free linked list).
    pub blocks_head: AtomicPtr<GcBlockHeader>,
    /// Mark stack for bitmapped marking.
    pub mark_stack: Mutex<Vec<SendPtr<GcHeader>>>,
    /// Condvar for parallel marking synchronization.
    pub mark_condvar: Condvar,
    /// Current state of the GC.
    pub state: AtomicU8,
    pub sweep_state: Mutex<SweepState>,
    /// Total number of bytes allocated.
    pub allocated_bytes: AtomicUsize,
    /// Threshold for the next collection cycle.
    pub threshold: AtomicUsize,
    /// Free lists for different size classes.
    pub free_lists: [AtomicUptr<FreeNode>; 10],
    /// Optional callback to run after each GC cycle.
    pub post_collect: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// Total number of collection cycles performed.
    pub total_collections: AtomicU64,
    /// Number of threads to use for parallel marking.
    pub marking_threads: usize,
}

#[repr(transparent)]
pub struct AtomicUptr<T>(pub AtomicPtr<T>);
impl<T> AtomicUptr<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self(AtomicPtr::new(ptr))
    }
    pub fn load(&self, order: Ordering) -> *mut T {
        self.0.load(order)
    }
    pub fn compare_exchange_weak(
        &self,
        old: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<*mut T, *mut T> {
        self.0.compare_exchange_weak(old, new, success, failure)
    }
    pub fn swap(&self, new: *mut T, order: Ordering) -> *mut T {
        self.0.swap(new, order)
    }
}

impl Drop for NyarGc {
    fn drop(&mut self) {
        let mut curr = self.blocks_head.load(Ordering::Acquire);
        while !curr.is_null() {
            unsafe {
                let next = (*curr).next.load(Ordering::Acquire);
                let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
                alloc::dealloc(curr as *mut u8, layout);
                curr = next;
            }
        }
    }
}

impl NyarGc {
    /// Coalesce adjacent free nodes in the free lists to reduce fragmentation.
    pub unsafe fn coalesce_free_lists(&self) {
        for i in 0..SIZE_CLASSES.len() {
            let free_head = &self.free_lists[i];
            let head = free_head.swap(std::ptr::null_mut(), Ordering::Acquire);
            if head.is_null() {
                continue;
            }

            // 1. Collect all nodes into a vector
            let mut nodes = Vec::new();
            let mut curr = head;
            while !curr.is_null() {
                nodes.push(curr);
                curr = (*curr).next;
            }

            // 2. Sort by address
            nodes.sort_unstable();

            // 3. Coalesce adjacent nodes
            let mut new_head: *mut FreeNode = std::ptr::null_mut();
            let mut last_node: *mut FreeNode = std::ptr::null_mut();

            let mut j = 0;
            while j < nodes.len() {
                let curr_node = nodes[j];
                let curr_addr = curr_node as usize;
                let curr_size = SIZE_CLASSES[i];

                let mut merged_size = curr_size;
                let mut k = j + 1;

                // Try to merge with subsequent nodes if they are physically adjacent
                while k < nodes.len() {
                    let next_node = nodes[k];
                    let next_addr = next_node as usize;
                    if curr_addr + merged_size == next_addr {
                        merged_size += SIZE_CLASSES[i];
                        k += 1;
                    } else {
                        break;
                    }
                }

                if merged_size > curr_size {
                    // Merged! Try to put it into a larger size class
                    if let Some(new_idx) = self.get_size_class(merged_size) {
                        if new_idx > i {
                            // Put into larger size class
                            let target_head = &self.free_lists[new_idx];
                            let mut old_target_head = target_head.load(Ordering::Acquire);
                            loop {
                                (*curr_node).next = old_target_head;
                                match target_head.compare_exchange_weak(
                                    old_target_head,
                                    curr_node,
                                    Ordering::Release,
                                    Ordering::Acquire,
                                ) {
                                    Ok(_) => break,
                                    Err(actual) => old_target_head = actual,
                                }
                            }
                            j = k;
                            continue;
                        }
                    }
                }

                // Not merged or couldn't move to larger class, keep in current list
                if last_node.is_null() {
                    new_head = curr_node;
                } else {
                    (*last_node).next = curr_node;
                }
                last_node = curr_node;
                (*last_node).next = std::ptr::null_mut();
                j = k;
            }

            // 4. Put back to free list
            if !new_head.is_null() {
                let mut old_head = free_head.load(Ordering::Acquire);
                loop {
                    (*last_node).next = old_head;
                    match free_head.compare_exchange_weak(
                        old_head,
                        new_head,
                        Ordering::Release,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(actual) => old_head = actual,
                    }
                }
            }
        }
    }

    pub fn set_post_collect<F>(&self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let mut post_collect = self.post_collect.lock().unwrap();
        *post_collect = Some(Box::new(f));
    }

    /// Internal helper to initialize a newly allocated GcBox.
    pub unsafe fn init_gc_box<T: Trace + 'static>(
        &self,
        ptr: *mut GcBox<T>,
        value: T,
        layout: Layout,
    ) -> Gc<T> {
        let state = self.state.load(Ordering::Acquire);
        let marking = state == GcState::Marking as u8;

        let (type_id, _) = Self::get_type_info::<T>();
        let size = layout.size();
        assert!(size <= 16380, "Object too large for GcBox, use alloc_large");
        let type_and_flags = (type_id as u32) | (((size >> 2) as u32) << 20); // Gen 0, not marked, not large, not dirty

        std::ptr::write(
            &mut (*ptr).header,
            GcHeader {
                type_and_flags: AtomicU32::new(type_and_flags),
            },
        );

        if marking {
            let base = (ptr as usize) & !(BLOCK_SIZE - 1);
            let block_header = base as *const GcBlockHeader;
            (*block_header).set_marked(&(*ptr).header);
        }

        std::ptr::write(&mut (*ptr).data, value);

        let gc_box = NonNull::new_unchecked(ptr);
        self.allocated_bytes
            .fetch_add(layout.size(), Ordering::SeqCst);

        Gc { ptr: gc_box }
    }

    pub fn get_type_info<T: Trace + 'static>() -> (u16, *const GcVTable) {
        let vtable_ptr = Self::get_vtable::<T>();

        // Linear search in the registry (usually few types)
        let count = VTABLE_COUNT.load(Ordering::Acquire);
        for i in 0..count {
            if VTABLE_REGISTRY[i].load(Ordering::Acquire) == vtable_ptr as *mut GcVTable {
                return (i as u16, vtable_ptr);
            }
        }

        // Not found, add it
        let id = VTABLE_COUNT.fetch_add(1, Ordering::SeqCst);
        if id >= 65536 {
            panic!("Too many types registered in GC");
        }
        VTABLE_REGISTRY[id].store(vtable_ptr as *mut GcVTable, Ordering::Release);
        (id as u16, vtable_ptr)
    }

    pub fn new() -> Self {
        // Ensure Type ID 0 is reserved for padding/empty slots
        {
            if VTABLE_COUNT.load(Ordering::Acquire) == 0 {
                static PADDING_VTABLE: GcVTable = GcVTable {
                    drop_and_dealloc: |_ptr| { /* Nothing to do */ },
                    trace_object: |_ptr, _ctx| { /* Nothing to do */ },
                };
                let id = VTABLE_COUNT.fetch_add(1, Ordering::SeqCst);
                VTABLE_REGISTRY[id].store(
                    &PADDING_VTABLE as *const GcVTable as *mut GcVTable,
                    Ordering::Release,
                );
            }
        }

        let first_block = GcBlock::new();
        let head = first_block.ptr.as_ptr() as *mut GcBlockHeader;
        std::mem::forget(first_block);
        Self {
            large_head: AtomicPtr::new(std::ptr::null_mut()),
            blocks_head: AtomicPtr::new(head),
            mark_stack: Mutex::new(Vec::new()),
            mark_condvar: Condvar::new(),
            state: AtomicU8::new(GcState::Idle as u8),
            sweep_state: Mutex::new(SweepState {
                block_curr: AtomicPtr::new(std::ptr::null_mut()),
                block_cursor: AtomicUsize::new(0),
                large_curr: AtomicPtr::new(std::ptr::null_mut()),
                large_prev: AtomicPtr::new(std::ptr::null_mut()),
            }),
            allocated_bytes: AtomicUsize::new(0),
            threshold: AtomicUsize::new(1024 * 1024), // 1MB default threshold
            free_lists: [
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
                AtomicUptr::new(std::ptr::null_mut()),
            ],
            post_collect: Mutex::new(None),
            total_collections: AtomicU64::new(0),
            marking_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        }
    }

    /// Get total allocated bytes.
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    /// Flush all thread-local mark buffers and allocation states.
    pub fn flush_thread_local(&self) {
        crate::tlab::THREAD_TLAB.with(|tlab_cell| {
            let tlab = unsafe { &mut *tlab_cell.get() };
            self.flush_mark_buffer(tlab);
        });
    }

    pub fn get_state(&self) -> GcState {
        match self.state.load(Ordering::Acquire) {
            0 => GcState::Idle,
            1 => GcState::Marking,
            2 => GcState::Sweeping,
            _ => unreachable!(),
        }
    }

    pub fn set_state(&self, state: GcState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Allocate a new value on the managed heap.
    #[inline(always)]
    pub fn alloc<T: Trace + 'static>(&self, value: T) -> Gc<T> {
        let layout = Layout::new::<GcBox<T>>();
        let size = layout.size();

        // 0. Large object allocation
        if size > BLOCK_SIZE / 4 {
            return unsafe { self.alloc_large(value, layout) };
        }

        // 1. Try to allocate from free list first
        if let Some(idx) = self.get_size_class(size) {
            let free_head = &self.free_lists[idx];
            let mut head = free_head.load(Ordering::Acquire);
            while !head.is_null() {
                let next = unsafe { (*head).next };
                match free_head.compare_exchange_weak(
                    head,
                    next,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        return unsafe { self.init_gc_box(head as *mut GcBox<T>, value, layout) };
                    }
                    Err(actual) => head = actual,
                }
            }
        }

        // 2. Try to allocate from TLAB
        let ptr = crate::tlab::THREAD_TLAB.with(|tlab_cell| {
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

    #[inline(always)]
    pub fn get_size_class(&self, size: usize) -> Option<usize> {
        for (i, &sc) in SIZE_CLASSES.iter().enumerate() {
            if size <= sc {
                return Some(i);
            }
        }
        None
    }

    /// Allocate a large object directly from the system allocator.
    pub unsafe fn alloc_large<T: Trace + 'static>(&self, value: T, layout: Layout) -> Gc<T> {
        let size = layout.size();
        let total_size =
            size + std::mem::size_of::<LargeObjectHeader>() + std::mem::size_of::<GcHeader>();
        // Alignment increased to 64 to support AVX-512 and high-performance FFI/GPU buffers
        let total_layout = Layout::from_size_align(total_size, 64).unwrap();

        let ptr = alloc::alloc(total_layout);
        if ptr.is_null() {
            alloc::handle_alloc_error(total_layout);
        }

        let large_header = ptr as *mut LargeObjectHeader;
        (*large_header).size = size as u32;

        let gc_header_ptr = ptr.add(std::mem::size_of::<LargeObjectHeader>()) as *mut GcHeader;
        let (type_id, _) = Self::get_type_info::<T>();

        // Flags: marked (bit 16), generation (bit 17), large (bit 19)
        let state = self.state.load(Ordering::Acquire);
        let marking = state == GcState::Marking as u8;

        let mut flags = (1 << 17) | (1 << 19);
        if marking {
            flags |= 1 << 16;
        }
        let type_and_flags = (type_id as u32) | flags;

        std::ptr::write(
            gc_header_ptr,
            GcHeader {
                type_and_flags: AtomicU32::new(type_and_flags),
            },
        );

        let data_ptr = gc_header_ptr.add(1) as *mut T;
        std::ptr::write(data_ptr, value);

        // Insert into large_head atomically
        let mut old_head = self.large_head.load(Ordering::Acquire);
        loop {
            (*large_header).next.store(old_head, Ordering::Release);
            match self.large_head.compare_exchange_weak(
                old_head,
                large_header,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => old_head = actual,
            }
        }

        self.allocated_bytes.fetch_add(size, Ordering::SeqCst);
        Gc {
            ptr: NonNull::new_unchecked(gc_header_ptr as *mut GcBox<T>),
        }
    }

    pub fn refill_tlab(&self, tlab: &mut Tlab, layout: Layout) -> *mut u8 {
        let tlab_layout = Layout::from_size_align(TLAB_SIZE, 16).unwrap();

        // If we are in sweeping state, try to find a block that needs sweeping
        if self.get_state() == GcState::Sweeping {
            let mut curr = self.blocks_head.load(Ordering::Acquire);
            while !curr.is_null() {
                unsafe {
                    if (*curr).state.load(Ordering::Acquire) == 1 {
                        self.sweep_block(curr);
                        // After sweeping, some free list entries might be available.
                        // However, the current thread is already in refill_tlab, 
                        // so we might as well continue to find a block or allocate new.
                    }
                    curr = (*curr).get_next();
                }
            }
        }

        // Try to find space in existing blocks
        let mut curr = self.blocks_head.load(Ordering::Acquire);
        while !curr.is_null() {
            unsafe {
                let block = GcBlock {
                    ptr: NonNull::new_unchecked(curr as *mut u8),
                };
                if let Some(ptr) = block.alloc(tlab_layout) {
                    // Forget the block so it's not deallocated
                    std::mem::forget(block);
                    tlab.start = ptr;
                    tlab.cursor = ptr;
                    tlab.end = ptr.add(TLAB_SIZE);
                    return tlab.alloc(layout).unwrap();
                }
                std::mem::forget(block);
                curr = (*curr).get_next();
            }
        }

        // No space in existing blocks, allocate a new block
        let new_block = GcBlock::new();
        let new_header = new_block.ptr.as_ptr() as *mut GcBlockHeader;
        let ptr = new_block.alloc(tlab_layout).unwrap();

        // Link the new block into the list
        let mut old_head = self.blocks_head.load(Ordering::Acquire);
        loop {
            unsafe {
                (*new_header).set_next(old_head);
                match self.blocks_head.compare_exchange_weak(
                    old_head,
                    new_header,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(actual) => old_head = actual,
                }
            }
        }

        // Forget the block so it's not deallocated
        std::mem::forget(new_block);

        tlab.start = ptr;
        tlab.cursor = ptr;
        tlab.end = unsafe { ptr.add(TLAB_SIZE) };
        tlab.alloc(layout).unwrap()
    }

    pub fn find_block(&self, ptr: *const u8) -> Option<*mut GcBlockHeader> {
        let base = (ptr as usize) & !(BLOCK_SIZE - 1);
        let header = base as *mut GcBlockHeader;
        unsafe {
            if (*header).magic == GC_BLOCK_MAGIC {
                Some(header)
            } else {
                None
            }
        }
    }

    /// Write a value to a cell within a GC-managed object, automatically triggering a write barrier.
    pub fn write<P: Trace + 'static, T: Trace + 'static>(
        &self,
        _parent: Gc<P>,
        cell: &GcCell<T>,
        value: T,
    ) {
        // Incremental barrier: mark the value being written (Dijkstra style)
        if self.get_state() == GcState::Marking {
            crate::tlab::THREAD_TLAB.with(|tlab_cell| {
                let tlab = unsafe { &mut *tlab_cell.get() };
                let mut mark_stack = Vec::new();
                let mut ctx = MarkContext {
                    mark_stack: &mut mark_stack,
                };
                value.trace(&mut ctx);

                for ptr in mark_stack {
                    self.local_mark(tlab, ptr.0);
                }
            });
        }
        cell.set(value);
    }

    /// Write barrier: should be called when an object is modified to point to another object.
    pub fn write_barrier<T: Trace + 'static, U: Trace + 'static>(
        &self,
        _parent: Gc<T>,
        child: Gc<U>,
    ) {
        self.write_barrier_ptr(child.ptr.cast());
    }

    /// Write barrier for a raw pointer to a GC object header.
    pub fn write_barrier_ptr(&self, child_header: NonNull<GcHeader>) {
        unsafe {
            // Incremental barrier: Dijkstra style
            if self.get_state() == GcState::Marking {
                crate::tlab::THREAD_TLAB.with(|tlab_cell| {
                    let tlab = &mut *tlab_cell.get();
                    self.local_mark(tlab, child_header);
                });
            }
        }
    }

    /// Mark an object using a thread-local buffer to reduce contention.
    pub fn local_mark(&self, tlab: &mut Tlab, ptr: NonNull<GcHeader>) {
        unsafe {
            let header = ptr.as_ref();
            let marked = if header.is_large() {
                if !header.is_marked() {
                    header.set_marked(true);
                    true
                } else {
                    false
                }
            } else {
                let base = (ptr.as_ptr() as usize) & !(BLOCK_SIZE - 1);
                let block_header = base as *const GcBlockHeader;
                (*block_header).set_marked(header)
            };

            if marked {
                tlab.mark_buffer.push(SendPtr(ptr));
                if tlab.mark_buffer.len() >= crate::tlab::MARK_BUFFER_SIZE {
                    self.flush_mark_buffer(tlab);
                }
            }
        }
    }

    /// Flush the thread-local mark buffer to the global mark stack.
    pub fn flush_mark_buffer(&self, tlab: &mut Tlab) {
        if !tlab.mark_buffer.is_empty() {
            let mut global_stack = self.mark_stack.lock().unwrap();
            global_stack.append(&mut tlab.mark_buffer);
        }
    }

    pub unsafe fn drop_and_dealloc<T: Trace + 'static>(header_ptr: NonNull<GcHeader>) {
        let header = header_ptr.as_ref();
        let ptr = header_ptr.cast::<GcBox<T>>();
        // Explicitly drop the data
        std::ptr::drop_in_place(&mut (*ptr.as_ptr()).data);

        if header.is_large() {
            let total_size = header.size()
                + std::mem::size_of::<LargeObjectHeader>()
                + std::mem::size_of::<GcHeader>();
            let layout = Layout::from_size_align(total_size, 16).unwrap();
            let raw_ptr =
                (header_ptr.as_ptr() as *mut u8).sub(std::mem::size_of::<LargeObjectHeader>());
            alloc::dealloc(raw_ptr, layout);
        }
        // Memory for non-large objects is managed by GcBlock, so we don't deallocate individual boxes here.
    }

    pub unsafe fn free_object(&self, header_ptr: NonNull<GcHeader>) {
        let header = header_ptr.as_ref();
        let size = header.size();

        // 1. Drop the data
        unsafe {
            ((*header.get_vtable()).drop_and_dealloc)(header_ptr);
        }

        // 2. Try to add to free list if it's a small object
        if let Some(idx) = self.get_size_class(size) {
            let free_head = &self.free_lists[idx];
            let node_ptr = header_ptr.as_ptr() as *mut FreeNode;
            let mut old_head = free_head.load(Ordering::Acquire);
            loop {
                (*node_ptr).next = old_head;
                match free_head.compare_exchange_weak(
                    old_head,
                    node_ptr,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(actual) => old_head = actual,
                }
            }
        }
    }

    pub unsafe fn trace_object<T: Trace + 'static>(
        header_ptr: NonNull<GcHeader>,
        ctx: &mut MarkContext<'_>,
    ) {
        let ptr = header_ptr.cast::<GcBox<T>>();
        (*ptr.as_ptr()).data.trace(ctx);
    }

    pub fn get_vtable<T: Trace + 'static>() -> *const GcVTable {
        struct VTableHolder<T>(T);
        impl<T: Trace + 'static> VTableHolder<T> {
            const VTABLE: GcVTable = GcVTable {
                drop_and_dealloc: NyarGc::drop_and_dealloc::<T>,
                trace_object: NyarGc::trace_object::<T>,
            };
        }
        &VTableHolder::<T>::VTABLE
    }

    /// Run a collection cycle with custom root tracing.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure.
    pub unsafe fn collect_with<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        self.collect_all(mark_roots);
    }

    /// Full collection: collect all objects using block-based scanning.
    pub fn parallel_mark(&self) {
        let num_threads = self.marking_threads;
        if num_threads <= 1 {
            let mut stack = self.mark_stack.lock().unwrap();
            let mut ctx = MarkContext {
                mark_stack: &mut *stack,
            };
            unsafe {
                self.process_mark_stack(&mut ctx);
            }
            return;
        }

        let active_workers = AtomicUsize::new(0);

        std::thread::scope(|s| {
            for _ in 0..num_threads {
                s.spawn(|| {
                    let mut local_stack = Vec::with_capacity(256);
                    loop {
                        // 1. Try to get work from global stack
                        {
                            let mut global_stack = self.mark_stack.lock().unwrap();
                            while global_stack.is_empty() {
                                if active_workers.load(Ordering::Acquire) == 0 {
                                    return; // No more work and no active workers
                                }
                                global_stack = self.mark_condvar.wait(global_stack).unwrap();
                                if global_stack.is_empty()
                                    && active_workers.load(Ordering::Acquire) == 0
                                {
                                    return;
                                }
                            }
                            // Take some work
                            let len = global_stack.len();
                            let take_count = ((len + 1) / 2).min(128);
                            let start = len - take_count;

                            #[cfg(target_arch = "x86_64")]
                            {
                                use std::arch::x86_64::_mm_prefetch;
                                for i in 0..take_count.min(8) {
                                    let p = global_stack[start + i].0.as_ptr();
                                    unsafe {
                                        _mm_prefetch(p as *const i8, std::arch::x86_64::_MM_HINT_T0);
                                    }
                                }
                            }

                            local_stack.extend(global_stack.drain(start..));
                        }

                        // 2. Process local stack
                        active_workers.fetch_add(1, Ordering::SeqCst);
                        unsafe {
                            while !local_stack.is_empty() {
                                let ptr = local_stack.pop().unwrap();

                                #[cfg(target_arch = "x86_64")]
                                if let Some(next) = local_stack.last() {
                                    use std::arch::x86_64::_mm_prefetch;
                                    _mm_prefetch(
                                        next.0.as_ptr() as *const i8,
                                        std::arch::x86_64::_MM_HINT_T0,
                                    );
                                }

                                let header = ptr.as_ref();
                                {
                                    let mut ctx = MarkContext {
                                        mark_stack: &mut local_stack,
                                    };
                                    ((*header.get_vtable()).trace_object)(ptr.0, &mut ctx);
                                }

                                // If local stack is too large, push some to global
                                if local_stack.len() > 512 {
                                    let mut global_stack = self.mark_stack.lock().unwrap();
                                    let len = local_stack.len();
                                    let push_count = len / 2;
                                    global_stack
                                        .extend(local_stack.drain(len - push_count..));
                                    self.mark_condvar.notify_all();
                                }
                            }
                        }
                        active_workers.fetch_sub(1, Ordering::SeqCst);
                        self.mark_condvar.notify_all();
                    }
                });
            }
        });
    }

    /// Trigger a full blocking garbage collection.
    /// This is intended to be called when the system is idle or requires a deep cleanup.
    pub fn full_gc(&self) {
        unsafe {
            // Request all threads to pause if they are in an async loop
            crate::runtime::GC_STOP_THE_WORLD.store(true, Ordering::Release);
            
            self.collect_all(|ctx| {
                // 1. Scan roots registered in the current thread
                crate::stack::scan_thread_roots(ctx);
                
                // TODO: In a multi-threaded VM, we would need to wait for other threads
                // to reach a safepoint/yield and then scan their roots.
            });
            
            crate::runtime::GC_STOP_THE_WORLD.store(false, Ordering::Release);
        }
    }

    pub unsafe fn collect_all<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        self.set_state(GcState::Marking);

        // Clear mark bitmaps and large object mark bits
        let mut block_curr = self.blocks_head.load(Ordering::Acquire);
        while !block_curr.is_null() {
            unsafe {
                (*block_curr).clear_mark_bitmap();
                (*block_curr).live_bytes.store(0, Ordering::Relaxed);
                block_curr = (*block_curr).get_next();
            }
        }
        let mut large_curr = self.large_head.load(Ordering::Acquire);
        while let Some(header_ptr) = NonNull::new(large_curr) {
            unsafe {
                header_ptr.as_ref().set_marked(false);
                large_curr = header_ptr.as_ref().get_next();
            }
        }

        let mut mark_stack = self.mark_stack.lock().unwrap();
        let mut ctx = MarkContext {
            mark_stack: &mut *mark_stack,
        };

        // 1. Mark roots
        mark_roots(&mut ctx);
        drop(mark_stack);

        // 2. Parallel marking
        self.parallel_mark();

        // 3. Flush all thread-local mark buffers before sweeping
        crate::tlab::THREAD_TLAB.with(|tlab_cell| {
            let tlab = unsafe { &mut *tlab_cell.get() };
            self.flush_mark_buffer(tlab);
        });
        self.parallel_mark();

        // 4. Start concurrent/lazy sweeping
        self.set_state(GcState::Sweeping);
        // Mark all blocks as needing sweep
        let mut block_curr = self.blocks_head.load(Ordering::Acquire);
        while !block_curr.is_null() {
            unsafe {
                (*block_curr).state.store(1, Ordering::Release);
                block_curr = (*block_curr).get_next();
            }
        }

        // Synchronously sweep large objects (they don't support lazy sweep yet)
        unsafe {
            self.sweep_all();
        }

        // 5. Adjust threshold (Adaptive based on live bytes)
        let live_bytes = self.allocated_bytes.load(Ordering::Relaxed);
        let next_threshold = (live_bytes * 2).max(live_bytes + 1024 * 1024).max(1024 * 1024);
        self.threshold.store(next_threshold, Ordering::Relaxed);

        self.reclaim_empty_blocks();
        self.coalesce_free_lists();
        self.total_collections.fetch_add(1, Ordering::SeqCst);
        self.set_state(GcState::Idle);
    }

    pub unsafe fn sweep_block(&self, block_ptr: *mut GcBlockHeader) {
        let block = &*block_ptr;
        // Check if another thread already started sweeping this block
        if block
            .state
            .compare_exchange(1, 2, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        for header_ptr in block.iter_objects() {
            let header = &*header_ptr;
            let size = header.size();

            let marked = block.is_marked(header);
            if marked {
                block.live_bytes.fetch_add(size, Ordering::Relaxed);
            } else {
                self.allocated_bytes.fetch_sub(size, Ordering::SeqCst);
                self.free_object(NonNull::new_unchecked(header_ptr));
            }
        }
        block.state.store(0, Ordering::Release);
    }

    pub unsafe fn sweep_all(&self) {
        // Sweep large objects (STW for now, they are few)
        let mut prev: *mut LargeObjectHeader = std::ptr::null_mut();
        let mut curr = self.large_head.load(Ordering::Acquire);

        while let Some(header_ptr) = NonNull::new(curr) {
            let header = header_ptr.as_ref();
            let next = header.get_next();

            // Prefetch the next large object header to reduce cache misses in the list traversal
            if !next.is_null() {
                #[cfg(target_arch = "x86_64")]
                unsafe {
                    use std::arch::x86_64::_mm_prefetch;
                    _mm_prefetch(next as *const i8, std::arch::x86_64::_MM_HINT_T0);
                }
            }

            if header.is_marked() {
                header.set_marked(false);
                prev = curr;
                curr = next;
            } else {
                if let Some(mut p) = NonNull::new(prev) {
                    p.as_mut().set_next(next);
                } else {
                    self.large_head.store(next, Ordering::Release);
                }

                self.allocated_bytes
                    .fetch_sub(header.size() as usize, Ordering::SeqCst);
                self.free_object(NonNull::new_unchecked(
                    header.get_gc_header() as *const GcHeader as *mut GcHeader,
                ));
                curr = next;
            }
        }
    }

    pub unsafe fn reclaim_empty_blocks(&self) {
        let mut prev: *mut GcBlockHeader = std::ptr::null_mut();
        let mut curr = self.blocks_head.load(Ordering::Acquire);

        while !curr.is_null() {
            let header = &*curr;
            let next = header.get_next();

            // Reclaim block if it's full (cursor == BLOCK_SIZE) and has no live objects
            if header.cursor.load(Ordering::Relaxed) >= BLOCK_SIZE
                && header.live_bytes.load(Ordering::Relaxed) == 0
            {
                // Don't reclaim if it's the only block
                if prev.is_null() && next.is_null() {
                    prev = curr;
                    curr = next;
                    continue;
                }

                if prev.is_null() {
                    self.blocks_head.store(next, Ordering::Release);
                } else {
                    (*prev).set_next(next);
                }
                let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
                alloc::dealloc(curr as *mut u8, layout);
                curr = next;
                continue;
            }
            prev = curr;
            curr = next;
        }
    }

    /// Process the mark stack until it's empty.
    pub unsafe fn process_mark_stack(&self, ctx: &mut MarkContext<'_>) {
        while let Some(ptr) = ctx.mark_stack.pop() {
            let header = ptr.as_ref();
            // Prefetch the object's data to improve cache locality
            #[cfg(target_arch = "x86_64")]
            {
                use std::arch::x86_64::_mm_prefetch;
                _mm_prefetch(ptr.as_ptr() as *const i8, std::arch::x86_64::_MM_HINT_T0);
            }
            // Object is being scanned, its children will be added to mark stack
            unsafe {
                ((*header.get_vtable()).trace_object)(ptr.0, ctx);
            }
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
        match self.get_state() {
            GcState::Idle => {
                self.set_state(GcState::Marking);
                let sweep = self.sweep_state.lock().unwrap();
                sweep
                    .block_curr
                    .store(self.blocks_head.load(Ordering::Acquire), Ordering::Release);
                sweep
                    .block_cursor
                    .store(std::mem::size_of::<GcBlockHeader>(), Ordering::Release);
                sweep
                    .large_curr
                    .store(self.large_head.load(Ordering::Acquire), Ordering::Release);
                sweep
                    .large_prev
                    .store(std::ptr::null_mut(), Ordering::Release);

                // Reset live bytes for all blocks before marking
                let mut block_curr = self.blocks_head.load(Ordering::Acquire);
                while !block_curr.is_null() {
                    unsafe {
                        (*block_curr).live_bytes.store(0, Ordering::Relaxed);
                        block_curr = (*block_curr).get_next();
                    }
                }

                // Initial marking from roots
                let mut mark_stack = self.mark_stack.lock().unwrap();
                let mut ctx = MarkContext {
                    mark_stack: &mut *mark_stack,
                };
                mark_roots(&mut ctx);
            }
            GcState::Marking => {
                let mut mark_stack = self.mark_stack.lock().unwrap();
                let mut ctx = MarkContext {
                    mark_stack: &mut *mark_stack,
                };
                let mut work_done = 0;
                while work_done < work_limit && !ctx.mark_stack.is_empty() {
                    self.process_mark_stack(&mut ctx);
                    work_done += 1;
                }
                if ctx.mark_stack.is_empty() {
                    self.set_state(GcState::Sweeping);
                    // Mark all blocks as needing sweep for lazy/concurrent sweepers
                    let mut block_curr = self.blocks_head.load(Ordering::Acquire);
                    while !block_curr.is_null() {
                        unsafe {
                            (*block_curr).state.store(1, Ordering::Release);
                            block_curr = (*block_curr).get_next();
                        }
                    }
                }
            }
            GcState::Sweeping => {
                let mut work_done = 0;
                let sweep = self.sweep_state.lock().unwrap();
                while work_done < work_limit {
                    // 1. Sweep blocks
                    let block_ptr = sweep.block_curr.load(Ordering::Acquire);
                    if !block_ptr.is_null() {
                        let block = &*block_ptr;
                        
                        // Try to take ownership of sweeping this block if we just started it
                        if sweep.block_cursor.load(Ordering::Acquire) == std::mem::size_of::<GcBlockHeader>() {
                            let _ = block.state.compare_exchange(1, 2, Ordering::SeqCst, Ordering::Relaxed);
                        }

                        let cursor = sweep.block_cursor.load(Ordering::Acquire);
                        let limit = block.cursor.load(Ordering::Acquire);

                        if cursor < limit {
                            let header_ptr = (block_ptr as *const u8).add(cursor) as *mut GcHeader;
                            let header = &*header_ptr;
                            let size = header.size();

                            // Validate size
                            if size < 4 || cursor + size > BLOCK_SIZE {
                                // Invalid size, skip to next block
                                sweep.block_curr.store(block.get_next(), Ordering::Release);
                                sweep
                                    .block_cursor
                                    .store(std::mem::size_of::<GcBlockHeader>(), Ordering::Release);
                                continue;
                            }

                            // Skip padding objects (Type ID 0)
                            if header.get_type_id() != 0 {
                                let marked = block.is_marked(header);
                                if marked {
                                    // Survived! Update live bytes
                                    block.live_bytes.fetch_add(size, Ordering::Relaxed);
                                } else {
                                    // Free
                                    self.allocated_bytes.fetch_sub(size, Ordering::SeqCst);
                                    self.free_object(NonNull::new_unchecked(header_ptr));
                                }
                                work_done += 1;
                            }
                            sweep.block_cursor.store(cursor + size, Ordering::Release);
                        } else {
                            // Finished this block, move to next
                            block.state.store(0, Ordering::Release);
                            sweep.block_curr.store(block.get_next(), Ordering::Release);
                            sweep
                                .block_cursor
                                .store(std::mem::size_of::<GcBlockHeader>(), Ordering::Release);
                        }
                    } else {
                        // 2. Sweep large objects
                        let large_curr_ptr = sweep.large_curr.load(Ordering::Acquire);
                        if let Some(header_ptr) = NonNull::new(large_curr_ptr) {
                            let header = header_ptr.as_ref();
                            let next = header.get_next();

                            if header.is_marked() {
                                header.set_marked(false);
                                sweep.large_prev.store(large_curr_ptr, Ordering::Release);
                                sweep.large_curr.store(next, Ordering::Release);
                            } else {
                                // Free
                                let large_prev_ptr = sweep.large_prev.load(Ordering::Acquire);
                                if let Some(mut p) = NonNull::new(large_prev_ptr) {
                                    p.as_mut().set_next(next);
                                } else {
                                    self.large_head.store(next, Ordering::Release);
                                }
                                self.allocated_bytes
                                    .fetch_sub(header.size() as usize, Ordering::SeqCst);
                                self.free_object(NonNull::new_unchecked(header.get_gc_header()
                                    as *const GcHeader
                                    as *mut GcHeader));
                                sweep.large_curr.store(next, Ordering::Release);
                            }
                            work_done += 1;
                        } else {
                            // Finished sweeping
                            self.threshold.store(
                                self.allocated_bytes.load(Ordering::Relaxed) * 2,
                                Ordering::Relaxed,
                            );
                            self.reclaim_empty_blocks();
                            self.coalesce_free_lists();
                            self.set_state(GcState::Idle);
                            break;
                        }
                    }
                }
            }
        }
    }
}
