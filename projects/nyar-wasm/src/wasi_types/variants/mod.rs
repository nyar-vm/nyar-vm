use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter, Write},
    hash::{Hash, Hasher},
    ops::AddAssign,
    sync::Arc,
};

use crate::{
    dag::DependentGraph,
    helpers::{ComponentSections, DependenciesTrace},
    Identifier, WasiType, WastEncoder,
};

mod arithmetic;
mod display;

/// `variant v { empty, wrapper($t) }`
#[derive(Debug, Clone, Eq)]
pub struct WasiVariantType {
    /// Variant name in language
    pub symbol: Identifier,
    /// All variant items of this variant
    pub variants: Vec<WasiVariantItem>,
}

impl Display for WasiVariantType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl PartialEq for WasiVariantType {
    fn eq(&self, other: &Self) -> bool {
        self.symbol.eq(&other.symbol)
    }
}
impl PartialOrd for WasiVariantType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.symbol.partial_cmp(&other.symbol)
    }
}
impl Ord for WasiVariantType {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol.cmp(&other.symbol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiVariantItem {
    /// Variant name in language
    pub symbol: Arc<str>,
    pub wasi_name: Arc<str>,
    pub fields: Option<WasiType>,
}

impl DependenciesTrace for WasiVariantType {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Variant(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.variants.iter().for_each(|v| v.collect_wasi_types(dict, collected));
    }
}

impl DependenciesTrace for WasiVariantItem {
    fn define_language_types(&self, _: &mut DependentGraph) {
        unreachable!()
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.fields.iter().for_each(|f| f.collect_wasi_types(dict, collected));
    }
}
