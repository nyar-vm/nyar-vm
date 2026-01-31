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

    pub fn alloc(&mut self, layout: Layout) -> Option<*mut u8> {
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
    pub static THREAD_TLAB: UnsafeCell<Tlab> = UnsafeCell::new(Tlab::new());
}
