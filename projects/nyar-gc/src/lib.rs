use std::alloc::{self, Layout};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{
    AtomicI32, AtomicPtr, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering,
};
use std::sync::Mutex;

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
const MARK_BITMAP_WORDS: usize = (BLOCK_SIZE / 16) / 64;

const SIZE_CLASSES: [usize; 7] = [64, 128, 256, 512, 1024, 2048, 4096];

static VTABLE_REGISTRY: Mutex<Vec<SendPtr<GcVTable>>> = Mutex::new(Vec::new());

struct FreeNode {
    next: *mut FreeNode,
}

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
        let padding = if align_offset == 0 {
            0
        } else {
            layout.align() - align_offset
        };
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

const GC_BLOCK_MAGIC: u64 = 0x4E5941524743424C; // "NYARGCBL"

struct GcBlockHeader {
    magic: u64,
    cursor: AtomicUsize,
    live_bytes: AtomicUsize,
    next: AtomicPtr<GcBlockHeader>,
    /// Card table for this block. Each bit represents CARD_SIZE bytes.
    /// 1 = dirty, 0 = clean.
    card_table: [AtomicU64; CARD_BITMAP_WORDS],
    /// Mark bitmap for this block. Each bit represents 16 bytes.
    /// 1 = marked, 0 = unmarked.
    mark_bitmap: [AtomicU64; MARK_BITMAP_WORDS],
}

impl GcBlockHeader {
    fn get_next(&self) -> *mut GcBlockHeader {
        self.next.load(Ordering::Acquire)
    }
    fn set_next(&self, next: *mut GcBlockHeader) {
        self.next.store(next, Ordering::Release)
    }

    fn is_marked(&self, ptr: *const GcHeader) -> bool {
        let offset = ptr as usize - (self as *const _ as usize);
        let bit_idx = offset / 16;
        let word_idx = bit_idx / 64;
        let bit_in_word = bit_idx % 64;
        (self.mark_bitmap[word_idx].load(Ordering::Acquire) & (1 << bit_in_word)) != 0
    }

    fn set_marked(&self, ptr: *const GcHeader) -> bool {
        let offset = ptr as usize - (self as *const _ as usize);
        let bit_idx = offset / 16;
        let word_idx = bit_idx / 64;
        let bit_in_word = bit_idx % 64;
        let mask = 1 << bit_in_word;
        let old = self.mark_bitmap[word_idx].fetch_or(mask, Ordering::SeqCst);
        (old & mask) == 0
    }

    fn clear_mark_bitmap(&self) {
        for word in self.mark_bitmap.iter() {
            word.store(0, Ordering::Release);
        }
    }

    /// Iterate over all objects in this block.
    ///
    /// # Safety
    /// The block must not be modified during iteration.
    unsafe fn iter_objects(&self) -> GcBlockIterator {
        GcBlockIterator {
            block: self,
            cursor: std::mem::size_of::<GcBlockHeader>(),
            limit: self.cursor.load(Ordering::Acquire),
        }
    }
}

struct GcBlockIterator<'a> {
    block: &'a GcBlockHeader,
    cursor: usize,
    limit: usize,
}

impl<'a> Iterator for GcBlockIterator<'a> {
    type Item = *mut GcHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.limit {
            return None;
        }

        unsafe {
            let ptr = (self.block as *const GcBlockHeader as *const u8).add(self.cursor) as *mut GcHeader;
            let header = &*ptr;
            let size = header.size();
            
            // Validate size to avoid infinite loop or out of bounds
            if size < 4 || self.cursor + size > BLOCK_SIZE {
                return None;
            }

            self.cursor += size;
            
            // Skip padding objects (Type ID 0)
            if header.get_type_id() == 0 {
                return self.next();
            }

            Some(ptr)
        }
    }
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
            (*header).magic = GC_BLOCK_MAGIC;
            (*header)
                .cursor
                .store(std::mem::size_of::<GcBlockHeader>(), Ordering::Relaxed);
            (*header).live_bytes.store(0, Ordering::Relaxed);
            (*header)
                .next
                .store(std::ptr::null_mut(), Ordering::Relaxed);
            for word in (*header).card_table.iter() {
                word.store(0, Ordering::Relaxed);
            }
            for word in (*header).mark_bitmap.iter() {
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
            let addr = self.ptr.as_ptr() as usize + cursor;
            let align_offset = addr % layout.align();
            let padding = if align_offset == 0 {
                0
            } else {
                layout.align() - align_offset
            };
            let size = layout.size();
            let new_cursor = cursor + padding + size;

            if new_cursor <= BLOCK_SIZE {
                if header
                    .cursor
                    .compare_exchange_weak(cursor, new_cursor, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    if padding > 0 {
                        // Fill padding gap with a padding object header
                        unsafe {
                            let padding_ptr = self.ptr.as_ptr().add(cursor) as *mut GcHeader;
                            let padding_size = padding;
                            // Type ID 0 (Padding), size_packed = padding_size >> 2
                            let type_and_flags = ((padding_size >> 2) as u32) << 20;
                            std::ptr::write(
                                padding_ptr,
                                GcHeader {
                                    type_and_flags: AtomicU32::new(type_and_flags),
                                },
                            );
                        }
                    }
                    return unsafe { Some(self.ptr.as_ptr().add(cursor + padding)) };
                }
            } else {
                return None;
            }
        }
    }

    fn mark_dirty(ptr: *const u8) {
        let base = (ptr as usize) & !(BLOCK_SIZE - 1);
        let header = base as *const GcBlockHeader;
        let offset = ptr as usize - base;
        let card_idx = offset / CARD_SIZE;
        let word_idx = card_idx / 64;
        let bit_idx = card_idx % 64;
        unsafe {
            let mask = 1 << bit_idx;
            // Optimization: check before atomic OR to avoid cache line bouncing
            if ((*header).card_table[word_idx].load(Ordering::Relaxed) & mask) == 0 {
                (*header).card_table[word_idx].fetch_or(mask, Ordering::Release);
            }
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
    pub(crate) mark_stack: &'a mut Vec<SendPtr<GcHeader>>,
}

impl<'a> MarkContext<'a> {
    /// Mark a GC pointer as reachable.
    pub unsafe fn mark(&mut self, ptr: NonNull<GcHeader>) {
        let header = ptr.as_ref();
        if header.is_large() {
            if !header.is_marked() {
                header.set_marked(true);
                self.mark_stack.push(SendPtr(ptr));
            }
        } else {
            let base = (ptr.as_ptr() as usize) & !(BLOCK_SIZE - 1);
            let block_header = base as *const GcBlockHeader;
            if (*block_header).set_marked(header) {
                self.mark_stack.push(SendPtr(ptr));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Gray = 1,
    Black = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GcState {
    /// Not yet visited.
    Idle = 0,
    /// GC is currently marking objects.
    Marking = 1,
    /// GC is currently sweeping unreachable objects.
    Sweeping = 2,
}

#[repr(transparent)]
pub struct GcHeader {
    /// Type ID for VTable lookup (low 16 bits) and Packed flags/size (high 16 bits).
    /// Bits 0-15: Type ID
    /// Bit 16: marked
    /// Bit 17: generation
    /// Bit 18: dirty
    /// Bit 19: large
    /// Bits 20-31: size_packed (size >> 2)
    pub(crate) type_and_flags: AtomicU32,
}

#[repr(C)]
struct LargeObjectHeader {
    next: AtomicPtr<LargeObjectHeader>,
    size: u32,
}

impl LargeObjectHeader {
    fn get_next(&self) -> *mut LargeObjectHeader {
        self.next.load(Ordering::Acquire)
    }
    fn set_next(&self, next: *mut LargeObjectHeader) {
        self.next.store(next, Ordering::Release)
    }
    fn get_gc_header(&self) -> &GcHeader {
        unsafe {
            let ptr = (self as *const LargeObjectHeader as *const u8)
                .add(std::mem::size_of::<LargeObjectHeader>())
                as *const GcHeader;
            &*ptr
        }
    }
    fn is_marked(&self) -> bool {
        self.get_gc_header().is_marked()
    }
    fn set_marked(&self, marked: bool) {
        self.get_gc_header().set_marked(marked)
    }
    fn size(&self) -> u32 {
        self.size
    }
}

/// VTable containing function pointers for GC operations.
pub struct GcVTable {
    /// Function to drop and deallocate the object.
    pub drop_and_dealloc: unsafe fn(NonNull<GcHeader>),
    /// Function to trace the object.
    pub trace_object: unsafe fn(NonNull<GcHeader>, &mut MarkContext<'_>),
}

impl GcHeader {
    pub fn size(&self) -> usize {
        let val = self.type_and_flags.load(Ordering::Acquire);
        if (val & (1 << 19)) != 0 {
            // Large object: size is stored in the LargeObjectHeader before the GcBox
            unsafe {
                let header_ptr = (self as *const GcHeader as *const u8)
                    .sub(std::mem::size_of::<LargeObjectHeader>())
                    as *const LargeObjectHeader;
                (*header_ptr).size as usize
            }
        } else {
            ((val >> 20) as usize) << 2
        }
    }

    pub fn is_marked(&self) -> bool {
        (self.type_and_flags.load(Ordering::Acquire) & (1 << 16)) != 0
    }
    pub fn set_marked(&self, marked: bool) {
        if marked {
            self.type_and_flags.fetch_or(1 << 16, Ordering::Release);
        } else {
            self.type_and_flags.fetch_and(!(1 << 16), Ordering::Release);
        }
    }
    pub fn is_dirty(&self) -> bool {
        (self.type_and_flags.load(Ordering::Acquire) & (1 << 18)) != 0
    }
    pub fn set_dirty(&self, dirty: bool) {
        if dirty {
            self.type_and_flags.fetch_or(1 << 18, Ordering::Release);
        } else {
            self.type_and_flags.fetch_and(!(1 << 18), Ordering::Release);
        }
    }
    pub fn is_large(&self) -> bool {
        (self.type_and_flags.load(Ordering::Acquire) & (1 << 19)) != 0
    }
    pub fn set_large(&self, large: bool) {
        if large {
            self.type_and_flags.fetch_or(1 << 19, Ordering::Release);
        } else {
            self.type_and_flags.fetch_and(!(1 << 19), Ordering::Release);
        }
    }
    pub fn get_generation(&self) -> u8 {
        ((self.type_and_flags.load(Ordering::Acquire) >> 17) & 0x01) as u8
    }
    pub fn set_generation(&self, gen: u8) {
        if gen != 0 {
            self.type_and_flags.fetch_or(1 << 17, Ordering::Release);
        } else {
            self.type_and_flags.fetch_and(!(1 << 17), Ordering::Release);
        }
    }
    pub fn get_type_id(&self) -> u16 {
        (self.type_and_flags.load(Ordering::Acquire) & 0xFFFF) as u16
    }
    pub fn set_type_id(&self, type_id: u16) {
        let mut old = self.type_and_flags.load(Ordering::Acquire);
        loop {
            let new = (old & !0xFFFF) | (type_id as u32);
            match self.type_and_flags.compare_exchange_weak(
                old,
                new,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => old = actual,
            }
        }
    }

    pub unsafe fn get_vtable(&self) -> *const GcVTable {
        let registry = VTABLE_REGISTRY.lock().unwrap();
        registry[self.get_type_id() as usize].as_ptr()
    }

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
struct SweepState {
    block_curr: AtomicPtr<GcBlockHeader>,
    block_cursor: AtomicUsize,
    large_curr: AtomicPtr<LargeObjectHeader>,
    large_prev: AtomicPtr<LargeObjectHeader>,
}

pub struct NyarGc {
    /// Head of the linked list of large objects.
    large_head: AtomicPtr<LargeObjectHeader>,
    /// Memory blocks managed by the GC (lock-free linked list).
    blocks_head: AtomicPtr<GcBlockHeader>,
    /// Mark stack for bitmapped marking.
    mark_stack: Mutex<Vec<SendPtr<GcHeader>>>,
    /// Current state of the GC.
    state: AtomicU8,
    sweep_state: Mutex<SweepState>,
    /// Total number of bytes allocated.
    allocated_bytes: AtomicUsize,
    /// Threshold for the next collection cycle.
    threshold: AtomicUsize,
    /// Free lists for different size classes.
    free_lists: [AtomicUptr<FreeNode>; 7],
    /// Optional callback to run after each GC cycle.
    post_collect: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// Total number of collection cycles performed.
    pub total_collections: AtomicU64,
}

#[repr(transparent)]
struct AtomicUptr<T>(AtomicPtr<T>);
impl<T> AtomicUptr<T> {
    fn new(ptr: *mut T) -> Self {
        Self(AtomicPtr::new(ptr))
    }
    fn load(&self, order: Ordering) -> *mut T {
        self.0.load(order)
    }
    fn compare_exchange_weak(
        &self,
        old: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<*mut T, *mut T> {
        self.0.compare_exchange_weak(old, new, success, failure)
    }
    fn swap(&self, new: *mut T, order: Ordering) -> *mut T {
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
    unsafe fn init_gc_box<T: Trace + 'static>(
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
                next_offset: AtomicI32::new(0),
            },
        );

        if marking {
            let base = (ptr as usize) & !(BLOCK_SIZE - 1);
            let block_header = base as *const GcBlockHeader;
            (*block_header).set_marked(&(*ptr).header);
        }

        std::ptr::write(&mut (*ptr).data, value);

        let gc_box = NonNull::new_unchecked(ptr);
        self.allocated_bytes.fetch_add(layout.size(), Ordering::SeqCst);

        Gc { ptr: gc_box }
    }

    fn get_type_info<T: Trace + 'static>() -> (u16, *const GcVTable) {
        let mut registry = VTABLE_REGISTRY.lock().unwrap();
        let vtable_ptr = Self::get_vtable::<T>();

        if let Some(pos) = registry
            .iter()
            .position(|&p| p.as_ptr() as *const GcVTable == vtable_ptr)
        {
            (pos as u16, vtable_ptr)
        } else {
            let id = registry.len() as u16;
            registry.push(SendPtr(NonNull::new(vtable_ptr as *mut GcVTable).unwrap()));
            (id, vtable_ptr)
        }
    }
    pub fn new() -> Self {
        // Ensure Type ID 0 is reserved for padding/empty slots
        {
            let mut registry = VTABLE_REGISTRY.lock().unwrap();
            if registry.is_empty() {
                static PADDING_VTABLE: GcVTable = GcVTable {
                    drop_and_dealloc: |ptr| { /* Nothing to do */ },
                    trace_object: |ptr, ctx| { /* Nothing to do */ },
                };
                registry.push(SendPtr(NonNull::from(&PADDING_VTABLE)));
            }
        }

        let first_block = GcBlock::new();
        let head = first_block.ptr.as_ptr() as *mut GcBlockHeader;
        std::mem::forget(first_block);
        Self {
            large_head: AtomicPtr::new(std::ptr::null_mut()),
            blocks_head: AtomicPtr::new(head),
            mark_stack: Mutex::new(Vec::new()),
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
            ],
            post_collect: Mutex::new(None),
            total_collections: AtomicU64::new(0),
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

    fn get_size_class(&self, size: usize) -> Option<usize> {
        for (i, &sc) in SIZE_CLASSES.iter().enumerate() {
            if size <= sc {
                return Some(i);
            }
        }
        None
    }

    /// Allocate a large object directly from the system allocator.
    unsafe fn alloc_large<T: Trace + 'static>(&self, value: T, layout: Layout) -> Gc<T> {
        let size = layout.size();
        let total_size =
            size + std::mem::size_of::<LargeObjectHeader>() + std::mem::size_of::<GcHeader>();
        let total_layout = Layout::from_size_align(total_size, 16).unwrap();

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

    fn refill_tlab(&self, tlab: &mut Tlab, layout: Layout) -> *mut u8 {
        // Try to get a new chunk from existing blocks
        let tlab_layout = Layout::from_size_align(TLAB_SIZE, 8).unwrap();

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

    fn find_block(&self, ptr: *const u8) -> Option<*mut GcBlockHeader> {
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
        unsafe {
            // Incremental barrier: mark the value being written (Dijkstra style)
            if self.get_state() == GcState::Marking {
                let mut mark_stack = self.mark_stack.lock().unwrap();
                let mut ctx = MarkContext {
                    mark_stack: &mut *mark_stack,
                };
                value.trace(&mut ctx);
            }
        }
        cell.set(value);
    }

    /// Write barrier: should be called when an object is modified to point to another object.
    pub fn write_barrier<T: Trace + 'static, U: Trace + 'static>(
        &self,
        _parent: Gc<T>,
        child: Gc<U>,
    ) {
        unsafe {
            let child_header = &child.ptr.as_ref().header;

            // Incremental barrier: Dijkstra style
            if self.get_state() == GcState::Marking {
                let mut mark_stack = self.mark_stack.lock().unwrap();
                let mut ctx = MarkContext {
                    mark_stack: &mut *mark_stack,
                };
                ctx.mark(NonNull::new_unchecked(
                    child_header as *const GcHeader as *mut GcHeader,
                ));
            }
        }
    }

    unsafe fn drop_and_dealloc<T: Trace + 'static>(header_ptr: NonNull<GcHeader>) {
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

    unsafe fn free_object(&self, header_ptr: NonNull<GcHeader>) {
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

    unsafe fn trace_object<T: Trace + 'static>(
        header_ptr: NonNull<GcHeader>,
        ctx: &mut MarkContext<'_>,
    ) {
        let ptr = header_ptr.cast::<GcBox<T>>();
        (*ptr.as_ptr()).data.trace(ctx);
    }

    fn get_vtable<T: Trace + 'static>() -> *const GcVTable {
        struct VTableHolder<T>(T);
        impl<T: Trace + 'static> VTableHolder<T> {
            const VTABLE: GcVTable = GcVTable {
                drop_and_dealloc: NyarGc::drop_and_dealloc::<T>,
                trace_object: NyarGc::trace_object::<T>,
            };
        }
        &VTableHolder::<T>::VTABLE
    }

    /// Run a collection cycle.
    ///
    /// # Safety
    /// The caller must ensure that all root pointers are traced via the provided closure.
    pub unsafe fn collect<F>(&self, mark_roots: F)
    where
        F: FnOnce(&mut MarkContext<'_>),
    {
        self.collect_all(mark_roots);
    }

    /// Full collection: collect all objects using block-based scanning.
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

        // 2. Process mark stack
        self.process_mark_stack(&mut ctx);

        // 3. Sweep everything
        self.set_state(GcState::Sweeping);
        self.sweep_all();

        // 4. Clear card tables
        let mut block_curr = self.blocks_head.load(Ordering::Acquire);
        while !block_curr.is_null() {
            unsafe {
                for word in (*block_curr).card_table.iter() {
                    word.store(0, Ordering::Release);
                }
                block_curr = (*block_curr).get_next();
            }
        }

        // 5. Adjust threshold
        self.threshold.store(
            self.allocated_bytes.load(Ordering::Relaxed) * 2,
            Ordering::Relaxed,
        );
        self.reclaim_empty_blocks();
        self.coalesce_free_lists();
        self.total_collections.fetch_add(1, Ordering::SeqCst);
        self.set_state(GcState::Idle);
    }

    unsafe fn sweep_all(&self) {
        // Sweep blocks
        let mut block_ptr = self.blocks_head.load(Ordering::Acquire);
        while !block_ptr.is_null() {
            let block = &*block_ptr;
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
            block_ptr = block.get_next();
        }

        // Sweep large objects
        let mut prev: *mut LargeObjectHeader = std::ptr::null_mut();
        let mut curr = self.large_head.load(Ordering::Acquire);

        while let Some(header_ptr) = NonNull::new(curr) {
            let header = header_ptr.as_ref();
            let next = header.get_next();

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

    unsafe fn reclaim_empty_blocks(&self) {
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
    unsafe fn process_mark_stack(&self, ctx: &mut MarkContext<'_>) {
        while let Some(ptr) = ctx.mark_stack.pop() {
            let header = ptr.as_ref();
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
                                self.free_object(NonNull::new_unchecked(
                                    header.get_gc_header() as *const GcHeader as *mut GcHeader,
                                ));
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

impl<T: Trace + 'static> Trace for Gc<T> {
    fn trace(&self, ctx: &mut MarkContext) {
        unsafe {
            let header_ptr =
                NonNull::new_unchecked(&self.ptr.as_ref().header as *const _ as *mut _);
            GcHeader::mark(header_ptr, ctx);
        }
    }
}

// Implement Trace for common types
impl Trace for i64 {
    fn trace(&self, _ctx: &mut MarkContext) {}
}
impl Trace for f64 {
    fn trace(&self, _ctx: &mut MarkContext) {}
}
impl Trace for bool {
    fn trace(&self, _ctx: &mut MarkContext) {}
}
impl Trace for String {
    fn trace(&self, _ctx: &mut MarkContext) {}
}
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
