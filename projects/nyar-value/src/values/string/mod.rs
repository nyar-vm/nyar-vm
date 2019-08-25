use gc::{Finalize, Trace};
use smartstring::{LazyCompact, SmartString};

#[derive(Debug)]
pub struct StringView {
    inner: SmartString<LazyCompact>,
}

impl Finalize for StringView {}

unsafe impl Trace for StringView {
    unsafe fn trace(&self) {
        todo!()
    }

    unsafe fn root(&self) {
        todo!()
    }

    unsafe fn unroot(&self) {
        todo!()
    }

    fn finalize_glue(&self) {
        todo!()
    }
}
