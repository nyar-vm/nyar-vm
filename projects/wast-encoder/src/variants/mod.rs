use std::{
    hash::{Hash, Hasher},
    ops::AddAssign,
    sync::Arc,
};

use convert_case::{Case, Casing};
use indexmap::IndexMap;

use crate::{dag::DependentGraph, DependenciesTrace, Identifier, WasiType};

mod arithmetic;
mod display;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantType {
    /// Variant name in language
    pub symbol: Identifier,
    pub wasi_name: String,
    pub variants: IndexMap<Arc<str>, VariantItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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

impl DependenciesTrace for VariantType {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Variant(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.variants.iter().for_each(|(_, v)| v.collect_wasi_types(dict, collected));
    }
}

impl DependenciesTrace for VariantItem {
    fn define_language_types(&self, _: &mut DependentGraph) {}

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.fields.iter().for_each(|f| f.collect_wasi_types(dict, collected));
    }
}
