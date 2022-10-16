use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{encode_id, wasi_module::WasiModule};

mod display;

#[derive(Clone, Debug)]
pub enum WasiType {
    Integer8 {
        signed: bool,
    },
    Integer16 {
        signed: bool,
    },
    Integer32 {
        signed: bool,
    },
    Integer64 {
        signed: bool,
    },
    Option {
        inner: Box<WasiType>,
    },
    Result {
        success: Option<Box<WasiType>>,
        failure: Option<Box<WasiType>>,
    },
    Resource(WasiResource),
    Variant(WasiVariant),
    TypeHandler {
        /// Resource language name
        name: Arc<str>,
        own: bool,
    },
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Arc<str>,
    },
}

#[derive(Clone, Debug)]
pub struct WasiResource {
    /// Resource language name
    pub name: Arc<str>,
    pub wasi_module: WasiModule,
    pub wasi_name: String,
    pub owned: bool,
}

impl WasiResource {
    pub fn new<S>(name: S, wasi_name: &str) -> Self
    where
        S: Into<Arc<str>>,
    {
        Self { name: name.into(), wasi_module: WasiModule::default(), wasi_name: wasi_name.to_string(), owned: true }
    }
    pub fn with_module<M: Into<WasiModule>>(self, wasi: WasiModule) -> Self {
        Self { wasi_module: wasi, ..self }
    }
}

#[derive(Clone, Debug)]
pub struct WasiVariant {
    /// Variant language name
    pub name: Arc<str>,
    pub variants: IndexMap<Arc<str>, WasiType>,
}

#[derive(Clone, Debug)]
pub struct WasiVariantItem {
    pub name: Arc<str>,
    pub variant: Arc<str>,
}
