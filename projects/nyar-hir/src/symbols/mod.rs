use nyar_error::FileSpan;
use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};
mod display;

#[derive(Clone, Debug)]
pub struct Identifier {
    pub path: Vec<Symbol>,
    pub span: FileSpan,
}
#[derive(Clone, Debug)]
pub struct Symbol {
    inner: Arc<str>,
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

impl Symbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}

impl FromIterator<Symbol> for Identifier {
    fn from_iter<T: IntoIterator<Item = Symbol>>(iter: T) -> Self {
        Self { path: iter.into_iter().collect(), span: Default::default() }
    }
}
