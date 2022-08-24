use crate::helpers::WasmName;
use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
};
use wast::token::{Id, Index};

mod convert;
mod display;

#[derive(Clone)]
pub struct WasmSymbol {
    inner: Arc<str>,
}

impl WasmSymbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
    pub fn as_index<'a, 'i>(&'a self) -> Index<'i>
    where
        'a: 'i,
    {
        Index::Id(WasmName::new(self.inner.as_ref()))
    }
    pub fn as_id<'a, 'i>(&'a self) -> Option<Id<'i>>
    where
        'a: 'i,
    {
        Some(WasmName::new(self.inner.as_ref()))
    }
}
