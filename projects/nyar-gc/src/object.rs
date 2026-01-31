use crate::block::{GcBlockHeader, BLOCK_SIZE};
use crate::collector::VTABLE_REGISTRY;
use crate::ptr::SendPtr;
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

/// A cell that can be used within GC-managed objects to store GC pointers.
pub struct GcCell<T: Trace + 'static> {
    pub inner: UnsafeCell<T>,
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
    /// Bit 19: large
    /// Bits 20-31: size_packed (size >> 2)
    pub(crate) type_and_flags: AtomicU32,
}

#[repr(C)]
pub struct LargeObjectHeader {
    pub next: AtomicPtr<LargeObjectHeader>,
    pub size: u32,
}

impl LargeObjectHeader {
    pub fn get_next(&self) -> *mut LargeObjectHeader {
        self.next.load(Ordering::Acquire)
    }
    pub fn set_next(&self, next: *mut LargeObjectHeader) {
        self.next.store(next, Ordering::Release)
    }
    pub fn get_gc_header(&self) -> &GcHeader {
        unsafe {
            let ptr = (self as *const LargeObjectHeader as *const u8)
                .add(std::mem::size_of::<LargeObjectHeader>())
                as *const GcHeader;
            &*ptr
        }
    }
    pub fn is_marked(&self) -> bool {
        self.get_gc_header().is_marked()
    }
    pub fn set_marked(&self, marked: bool) {
        self.get_gc_header().set_marked(marked)
    }
    pub fn size(&self) -> u32 {
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
    pub ptr: NonNull<GcBox<T>>,
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
