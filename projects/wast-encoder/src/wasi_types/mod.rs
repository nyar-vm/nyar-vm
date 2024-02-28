use std::fmt::{Display, Formatter};

use crate::{dag::DependentGraph, DependencyLogger, Identifier, ResolveDependencies, VariantType, WasiResource};

mod display;

#[derive(Clone, Debug, Eq, PartialEq)]
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
    Variant(VariantType),
    TypeHandler {
        /// Resource language name
        name: Identifier,
        own: bool,
    },
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Identifier,
    },
}

impl ResolveDependencies for WasiType {
    fn collect_wasi_types(&self, dict: &mut DependentGraph) {
        match self {
            WasiType::Option { inner } => inner.collect_wasi_types(dict),
            WasiType::Result { success, failure } => {
                success.iter().for_each(|s| s.collect_wasi_types(dict));
                failure.iter().for_each(|f| f.collect_wasi_types(dict));
            }
            WasiType::Resource(v) => v.collect_wasi_types(dict),
            WasiType::Variant(v) => v.collect_wasi_types(dict),
            _ => {}
        };
    }

    fn trace_language_types(&self, dict: &mut DependencyLogger) {
        match self {
            Self::Option { inner } => inner.trace_language_types(dict),
            Self::Result { success, failure } => {
                success.iter().for_each(|s| s.trace_language_types(dict));
                failure.iter().for_each(|f| f.trace_language_types(dict));
            }
            Self::Resource(_) => {}
            Self::Variant(v) => v.trace_language_types(dict),
            Self::TypeHandler { name, .. } => {
                dict.types.insert(name.clone());
            }
            Self::TypeAlias { name } => {
                dict.types.insert(name.clone());
            }
            _ => {}
        };
    }

    fn trace_modules(&self, dict: &mut DependencyLogger) {
        match self {
            WasiType::Integer8 { .. } => {}
            WasiType::Integer16 { .. } => {}
            WasiType::Integer32 { .. } => {}
            WasiType::Integer64 { .. } => {}
            WasiType::Option { .. } => {}
            WasiType::Result { .. } => {}
            WasiType::Resource(v) => v.trace_modules(dict),
            WasiType::Variant(v) => v.trace_modules(dict),
            WasiType::TypeHandler { .. } => {}
            WasiType::TypeAlias { .. } => {}
            WasiType::External(_) => {}
        }
    }
}
