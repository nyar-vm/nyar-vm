use crate::helpers::{IntoWasm, WasmName};
use semver::Version;
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

#[derive(Clone)]
pub struct WasmExportName {
    inner: Arc<str>,
    version: Option<Version>,
}

impl WasmSymbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}
impl WasmExportName {
    pub fn create<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { inner: name.into().inner, version: None }
    }
    pub fn create_by(symbol: &WasmSymbol, export: bool) -> Option<Self> {
        match export {
            true => Some(WasmExportName { inner: symbol.inner.clone(), version: None }),
            false => None,
        }
    }
}
