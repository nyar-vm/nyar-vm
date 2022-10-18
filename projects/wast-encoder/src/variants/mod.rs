use std::{ops::AddAssign, sync::Arc};

use convert_case::{Case, Casing};
use indexmap::IndexMap;

use crate::{DependencyLogger, Identifier, ResolveDependencies, WasiType};

mod arithmetic;

#[derive(Clone, Debug)]
pub struct VariantType {
    /// Variant name in language
    pub symbol: Identifier,
    pub wasi_name: String,
    pub variants: IndexMap<Arc<str>, VariantItem>,
}

#[derive(Clone, Debug)]
pub struct VariantItem {
    /// Variant name in language
    pub symbol: Arc<str>,
    pub wasi_name: String,
    pub fields: Option<WasiType>,
}

impl VariantType {
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Identifier>,
    {
        let name = name.into();
        let wasi_name = name.wasi_name();
        Self { symbol: name, wasi_name, variants: IndexMap::new() }
    }
    /// Custom wasi name for the variant type
    pub fn with_wasi_name<S>(self, wasi_name: S) -> Self
    where
        S: Into<String>,
    {
        Self { wasi_name: wasi_name.into(), ..self }
    }
}

impl VariantItem {
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Arc<str>>,
    {
        let name = name.into();
        let wasi_name = name.as_ref().to_case(Case::Kebab);
        Self { symbol: name, wasi_name, fields: None }
    }
    /// Custom wasi name for the variant item
    pub fn with_wasi_name<S>(self, wasi_name: S) -> Self
    where
        S: Into<String>,
    {
        Self { wasi_name: wasi_name.into(), ..self }
    }
    pub fn with_fields<T>(self, field: T) -> Self
    where
        T: Into<WasiType>,
    {
        Self { fields: Some(field.into()), ..self }
    }
}

//     (type $stream-error (variant
//         (case "last-operation-failed" (own $io-error))
//         (case "closed")
//     ))

impl ResolveDependencies for VariantType {
    fn trace_language_types(&self, dict: &mut DependencyLogger) {
        self.variants.values().for_each(|v| v.trace_language_types(dict));
    }

    fn trace_modules(&self, _: &mut DependencyLogger) {}
}

impl ResolveDependencies for VariantItem {
    fn trace_language_types(&self, dict: &mut DependencyLogger) {
        self.fields.iter().for_each(|f| f.trace_language_types(dict));
    }

    fn trace_modules(&self, _: &mut DependencyLogger) {}
}
