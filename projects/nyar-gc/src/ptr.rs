use std::ptr::NonNull;

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
