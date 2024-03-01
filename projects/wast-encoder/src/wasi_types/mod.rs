use std::{
    fmt::{Debug, Display, Formatter, Write},
    hash::{Hash, Hasher},
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{
    dag::DependentGraph,
    DependenciesTrace,
    encoder::WastEncoder,
    ExternalFunction,
    helpers::{TypeDefinition, TypeReference}, Identifier, wasi_types::{array::WasiArrayType, resources::WasiResource, variants::WasiVariantType}, WasiModule, WasiParameter,
};

pub mod array;
mod display;
pub mod functions;
pub mod records;
pub mod resources;
pub mod variants;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    Variant(WasiVariantType),
    TypeHandler {
        /// Resource language name
        name: Identifier,
        own: bool,
    },
    Array(Box<WasiArrayType>),
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Identifier,
    },
    External(Box<ExternalFunction>),
}

impl WasiType {
    pub fn wasm_module(&self) -> Option<&WasiModule> {
        match self {
            Self::Resource(v) => Some(&v.wasi_module),
            Self::External(v) => Some(&v.wasi_module),
            _ => None,
        }
    }
    pub fn language_id(&self) -> Option<&Identifier> {
        match self {
            Self::Variant(v) => Some(&v.symbol),
            Self::Resource(v) => Some(&v.symbol),
            Self::External(v) => Some(&v.symbol),
            _ => None,
        }
    }
}

impl DependenciesTrace for WasiType {
    #[track_caller]
    fn define_language_types(&self, _: &mut DependentGraph) {
        panic!("Invalid Call");
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        match self {
            WasiType::Option { inner } => inner.collect_wasi_types(dict, collected),
            WasiType::Result { success, failure } => {
                success.iter().for_each(|s| s.collect_wasi_types(dict, collected));
                failure.iter().for_each(|f| f.collect_wasi_types(dict, collected));
            }
            WasiType::Resource(_) => collected.push(self),
            WasiType::Variant(_) => collected.push(self),
            WasiType::TypeAlias { name } => collected.extend(dict.types.get(name)),
            WasiType::TypeHandler { name, .. } => collected.extend(dict.types.get(name)),
            WasiType::External(_) => collected.push(self),
            _ => {}
        };
    }
}
