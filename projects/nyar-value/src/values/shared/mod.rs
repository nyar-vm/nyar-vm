use gc::{BorrowMutError, Gc, GcCell, GcCellRefMut, Trace};
use std::{
    borrow::BorrowMut,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, PoisonError, TryLockResult},
};

///
pub struct Shared<T>
where
    T: Trace + 'static,
{
    inner: Gc<GcCell<T>>,
}

impl<T> DerefMut for Shared<T>
where
    T: Trace + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.inner.try_borrow_mut() {
            Ok(mut o) => &mut o,
            Err(e) => {
                panic!("{}", e)
            }
        }
    }
}

impl<T> Deref for Shared<T>
where
    T: Trace + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.inner.try_borrow() {
            Ok(mut o) => &o,
            Err(e) => {
                panic!("{}", e)
            }
        }
    }
}
