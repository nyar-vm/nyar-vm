use std::ptr::NonNull;

#[repr(transparent)]
pub struct SendPtr<T: ?Sized>(pub NonNull<T>);
unsafe impl<T: ?Sized> Send for SendPtr<T> {}
unsafe impl<T: ?Sized> Sync for SendPtr<T> {}

impl<T: ?Sized> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<T: ?Sized> Copy for SendPtr<T> {}
impl<T: ?Sized> std::ops::Deref for SendPtr<T> {
    type Target = NonNull<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> SendPtr<T> {
    pub fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }

    pub unsafe fn as_ref(&self) -> &T {
        self.0.as_ref()
    }

    pub unsafe fn as_mut(&mut self) -> &mut T {
        self.0.as_mut()
    }
}
