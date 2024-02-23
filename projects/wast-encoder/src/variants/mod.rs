use std::{ops::AddAssign, sync::Arc};

use convert_case::{Case, Casing};
use indexmap::IndexMap;

use crate::WasiType;

mod arithmetic;

#[derive(Clone, Debug)]
pub struct WasiVariant {
    /// Variant language name
    pub name: Arc<str>,
    pub wasi_name: String,
    pub variants: IndexMap<Arc<str>, WasiVariantItem>,
}

#[derive(Clone, Debug)]
pub struct WasiVariantItem {
    pub name: Arc<str>,
    pub wasi_name: String,
    pub r#type: WasiType,
}

impl WasiVariant {
    pub fn new<S, T>(name: S) -> Self
    where
        S: Into<Arc<str>>,
    {
        let name = name.into();
        let wasi_name = name.to_case(Case::Kebab);
        Self { name, wasi_name: wasi_name.to_string(), variants: IndexMap::new() }
    }
    /// Custom wasi name for the variant type
    pub fn with_wasi_name<S>(self, wasi_name: S) -> Self
    where
        S: Into<String>,
    {
        Self { wasi_name: wasi_name.into(), ..self }
    }
}

impl WasiVariantItem {
    pub fn new<S, T>(name: S, r#type: T) -> Self
    where
        S: Into<Arc<str>>,
        T: Into<WasiType>,
    {
        let name = name.into();
        let wasi_name = name.to_case(Case::Kebab);
        Self { name: name.into(), wasi_name: wasi_name.to_string(), r#type }
    }
    /// Custom wasi name for the variant item
    pub fn with_wasi_name<S>(self, wasi_name: S) -> Self
    where
        S: Into<String>,
    {
        Self { wasi_name: wasi_name.into(), ..self }
    }
}

//     (type $stream-error (variant
//         (case "last-operation-failed" (own $io-error))
//         (case "closed")
//     ))
