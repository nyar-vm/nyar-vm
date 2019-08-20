use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, PoisonError, TryLockResult},
};

/// TODO: replace with gc pointer
pub struct Shared<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.inner.get_mut().as_deref_mut() {
            Ok(o) => o,
            Err(e) => panic!("{}", e),
        }
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.inner.try_lock() {
            Ok(o) => o.deref(),
            Err(e) => panic!("{}", e),
        }
    }
}
