use nyar_error::FileSpan;
use std::{
    fmt::{Display, Formatter},
    rc::Rc,
    sync::Arc,
};
mod display;

#[derive(Clone, Debug)]
pub struct Identifier {
    path: Vec<Symbol>,
    span: FileSpan,
}
#[derive(Clone, Debug)]
pub struct Symbol {
    inner: Arc<str>,
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
