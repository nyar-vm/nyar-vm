use crate::helpers::{IntoWasm, WasmName};
use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
};
use wast::{
    component::ComponentExternName,
    token::{Id, Index},
};

mod codegen;
mod convert;
mod display;

#[derive(Clone)]
pub struct WasmSymbol {
    inner: Arc<str>,
}

#[derive(Clone, Default)]
pub struct WasmExportName {
    inner: Option<Arc<str>>,
}

impl WasmSymbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}
impl WasmExportName {
    pub fn create<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { inner: Some(name.into().inner) }
    }
    pub fn create_by_str(symbol: &WasmSymbol, export: bool) -> Self {
        match export {
            true => WasmExportName { inner: Some(symbol.inner.clone()) },
            false => WasmExportName::default(),
        }
    }
    pub fn clear(&mut self) {
        self.inner = None;
    }
}
