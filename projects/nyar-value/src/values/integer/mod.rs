use gc::{Finalize, Trace};
use num::BigInt;

pub struct NyarInteger {
    inner: BigInt,
}

impl Finalize for NyarInteger {}

unsafe impl Trace for NyarInteger {
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
