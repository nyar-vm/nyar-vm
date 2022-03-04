use super::*;

pub struct ValkyrieLiteralType<T> {
    data: T,
}

impl<T> ValkyrieLiteralType<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}
