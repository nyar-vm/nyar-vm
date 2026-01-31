use std::alloc::Layout;
use std::cell::UnsafeCell;

pub const TLAB_SIZE: usize = 64 * 1024; // 64KB TLAB
pub const MARK_BUFFER_SIZE: usize = 128;

pub struct Tlab {
    pub start: *mut u8,
    pub cursor: *mut u8,
    pub end: *mut u8,
    pub mark_buffer: Vec<crate::ptr::SendPtr<crate::object::GcHeader>>,
}

impl Tlab {
    pub const fn new() -> Self {
        Self {
            start: std::ptr::null_mut(),
            cursor: std::ptr::null_mut(),
            end: std::ptr::null_mut(),
            mark_buffer: Vec::new(),
        }
    }

    #[inline(always)]
    pub fn alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        let cursor = self.cursor as usize;
        let align_mask = layout.align() - 1;
        let aligned_cursor = (cursor + align_mask) & !align_mask;
        let new_cursor = aligned_cursor + layout.size();

        if new_cursor <= self.end as usize {
            self.cursor = new_cursor as *mut u8;
            Some(aligned_cursor as *mut u8)
        } else {
            None
        }
    }
}

thread_local! {
    pub static THREAD_TLAB: UnsafeCell<Tlab> = UnsafeCell::new(Tlab::new());
}
