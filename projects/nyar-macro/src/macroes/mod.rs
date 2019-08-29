#[macro_export]
macro_rules! sync_value_type {
    ($t: ty) => {
        unsafe impl shredder::marker::GcDeref for $t {}
        unsafe impl shredder::marker::GcDrop for $t {}
        unsafe impl shredder::marker::GcSafe for $t {}
        unsafe impl shredder::Scan for $t {
            #[inline(always)]
            fn scan(&self, _: &mut shredder::Scanner<'_>) {}
        }

        unsafe impl shredder::Finalize for $t {
            unsafe fn finalize(&mut self) {
                std::ptr::drop_in_place(self);
            }
        }
    };
}
