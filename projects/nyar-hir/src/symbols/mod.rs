use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
};

mod convert;
mod display;

#[derive(Clone)]
pub struct Symbol {
    inner: Arc<str>,
}

impl Symbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}
