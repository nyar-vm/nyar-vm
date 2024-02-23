use std::sync::Arc;

use indexmap::IndexMap;

#[derive(Clone, Debug)]
pub enum WasiType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
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
    pub wasi_name: String,
    pub owned: bool,
}

impl WasiResource {
    pub fn new<S>(name: S, wasi_name: &str) -> Self
        where
            S: Into<Arc<str>>,
    {
        Self { name: name.into(), wasi_name: wasi_name.to_string(), owned: true }
    }
    pub fn with_owned(self) -> Self {
        Self { owned: true, ..self }
    }
    pub fn with_borrow(self) -> Self {
        Self { owned: false, ..self }
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
