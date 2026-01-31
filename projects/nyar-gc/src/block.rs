use crate::object::GcHeader;
use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub const BLOCK_SIZE: usize = 1024 * 1024; // 1MB blocks
pub const MARK_BITMAP_WORDS: usize = (BLOCK_SIZE / 16) / 64;
pub const GC_BLOCK_MAGIC: u64 = 0x4E5941524743424C; // "NYARGCBL"

pub struct GcBlockHeader {
    pub magic: u64,
    pub cursor: AtomicUsize,
    pub live_bytes: AtomicUsize,
    pub next: AtomicPtr<GcBlockHeader>,
    /// Mark bitmap for this block. Each bit represents 16 bytes.
    /// 1 = marked, 0 = unmarked.
    pub mark_bitmap: [AtomicU64; MARK_BITMAP_WORDS],
}

impl GcBlockHeader {
    pub fn get_next(&self) -> *mut GcBlockHeader {
        self.next.load(Ordering::Acquire)
    }
    pub fn set_next(&self, next: *mut GcBlockHeader) {
        self.next.store(next, Ordering::Release)
    }

    pub fn is_marked(&self, ptr: *const GcHeader) -> bool {
        let offset = ptr as usize - (self as *const _ as usize);
        let bit_idx = offset / 16;
        let word_idx = bit_idx / 64;
        let bit_in_word = bit_idx % 64;
        (self.mark_bitmap[word_idx].load(Ordering::Acquire) & (1 << bit_in_word)) != 0
    }

    pub fn set_marked(&self, ptr: *const GcHeader) -> bool {
        let offset = ptr as usize - (self as *const _ as usize);
        let bit_idx = offset / 16;
        let word_idx = bit_idx / 64;
        let bit_in_word = bit_idx % 64;
        let mask = 1 << bit_in_word;
        let old = self.mark_bitmap[word_idx].fetch_or(mask, Ordering::SeqCst);
        (old & mask) == 0
    }

    pub fn clear_mark_bitmap(&self) {
        for word in self.mark_bitmap.iter() {
            word.store(0, Ordering::Release);
        }
    }

    /// Iterate over all objects in this block.
    ///
    /// # Safety
    /// The block must not be modified during iteration.
    pub unsafe fn iter_objects(&self) -> GcBlockIterator<'_> {
        GcBlockIterator {
            block: self,
            cursor: std::mem::size_of::<GcBlockHeader>(),
            limit: self.cursor.load(Ordering::Acquire),
        }
    }
}

pub struct GcBlockIterator<'a> {
    pub block: &'a GcBlockHeader,
    pub cursor: usize,
    pub limit: usize,
}

impl<'a> Iterator for GcBlockIterator<'a> {
    type Item = *mut GcHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.limit {
            return None;
        }

        unsafe {
            let ptr =
                (self.block as *const GcBlockHeader as *const u8).add(self.cursor) as *mut GcHeader;
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

pub struct GcBlock {
    pub ptr: NonNull<u8>,
}

unsafe impl Send for GcBlock {}
unsafe impl Sync for GcBlock {}

impl GcBlock {
    pub fn new() -> Self {
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
            for word in (*header).mark_bitmap.iter() {
                word.store(0, Ordering::Relaxed);
            }

            Self {
                ptr: NonNull::new_unchecked(ptr),
            }
        }
    }

    pub fn get_header(&self) -> &GcBlockHeader {
        unsafe { &*(self.ptr.as_ptr() as *const GcBlockHeader) }
    }

    pub fn alloc(&self, layout: Layout) -> Option<*mut u8> {
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
}

impl Drop for GcBlock {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
        unsafe {
            alloc::dealloc(self.ptr.as_ptr(), layout);
        }
    }
}
