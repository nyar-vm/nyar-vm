use std::{
    fmt::{Debug, Display, Formatter, Write},
    hash::{Hash, Hasher},
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{
    dag::DependentGraph,
    encoder::WastEncoder,
    helpers::{AliasOuter, ComponentDefine, TypeDefinition, TypeReference},
    wasi_types::{array::WasiArrayType, resources::WasiResource, variants::WasiVariantType},
    DependenciesTrace, Identifier, WasiExternalFunction, WasiModule, WasiRecordType,
};
use std::{cmp::Ordering, ops::AddAssign};

pub mod array;
mod display;
pub mod functions;
pub mod records;
pub mod resources;
pub mod variants;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiType {
    /// `s8` or `u8` type in WASI
    Integer8 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `s16` or `u16` type in WASI
    Integer16 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `s32` or `u32` type in WASI
    Integer32 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `s64` or `u64` type in WASI
    Integer64 {
        /// Whether the integer is signed or not
        signed: bool,
    },
    /// `f32` type in WASI
    Float32,
    /// `f64` type in WASI
    Float64,
    Option {
        inner: Box<WasiType>,
    },
    Result {
        success: Option<Box<WasiType>>,
        failure: Option<Box<WasiType>>,
    },
    /// `resource` type in WASI
    Resource(WasiResource),
    /// `record` type in WASI
    Record(WasiRecordType),
    /// `variant` type in WASI
    Variant(WasiVariantType),
    TypeHandler {
        /// Resource language name
        name: Identifier,
        own: bool,
    },
    /// A referenced type, the real type needs to be found later
    TypeQuery {
        /// Type language name
        name: Identifier,
    },
    /// `list` type in WASI
    Array(Box<WasiArrayType>),
    /// The host function type in WASI
    External(Box<WasiExternalFunction>),
}

impl WasiType {
    /// Get the type definition of the type, composite type returns `None`
    pub fn wasm_module(&self) -> Option<&WasiModule> {
        match self {
            Self::Resource(v) => Some(&v.wasi_module),
            Self::External(v) => Some(&v.wasi_module),
            _ => None,
        }
    }
    /// Returns the language identifier of the type, anonymous type returns `None`
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
            WasiType::TypeQuery { name } => collected.extend(dict.types.get(name)),
            WasiType::TypeHandler { name, .. } => collected.extend(dict.types.get(name)),
            WasiType::External(_) => collected.push(self),
            _ => {}
        };
    }
}
