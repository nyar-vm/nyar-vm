use crate::helpers::{IntoWasm, WasmName};
use nyar_error::SyntaxError;
use semver::Version;
use std::{
    fmt::{Debug, Display, Formatter},
    str::FromStr,
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

/// e.g.: `wasi:random` in `wasi:random/random@0.2.0`
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasmPublisher {
    organization: Arc<str>,
    project: Arc<str>,
}

impl WasmSymbol {
    /// Create a new symbol
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}

/// e.g: `wasi:random/random@0.2.0`
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiName {
    name: Arc<str>,
    package: Option<WasmPublisher>,
    version: Option<Version>,
}

impl WasiName {
    /// Create a new module without a publisher
    pub fn create<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { package: None, name: name.into().inner, version: None }
    }
    /// Create a new module with automatic export
    pub fn create_by(symbol: &WasmSymbol, export: bool) -> Option<Self> {
        match export {
            true => Some(WasiName { package: None, name: symbol.inner.clone(), version: None }),
            false => None,
        }
    }
    /// Set the publisher for the module
    pub fn with_publisher(self, publisher: WasmPublisher) -> Self {
        Self { package: Some(publisher), ..self }
    }
    /// Set the version for the module
    pub fn with_version(self, version: Version) -> Self {
        Self { version: Some(version), ..self }
    }
    /// Set the organization and project for the module
    pub fn with_project<O: Into<WasmSymbol>, P: Into<WasmSymbol>>(self, organization: O, project: P) -> Self {
        Self { package: Some(WasmPublisher { organization: organization.into().inner, project: project.into().inner }), ..self }
    }
}

impl WasiName {
    pub fn short_name(&self) -> &str {
        self.name.as_ref()
    }
    pub fn long_name(&self) -> &str {
        // SAFETY: StringPool will deallocate this string when there are no more references to it
        unsafe {
            let cache = string_pool::String::from(self.to_string());
            &*(cache.as_str() as *const str)
        }
    }
}
