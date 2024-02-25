use std::fmt::{Display, Formatter};

use crate::{dag::DependentGraph, ExternalFunction, Identifier, ResolveDependencies, VariantType, WasiResource};

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
    External(Box<ExternalFunction>),
}

impl ResolveDependencies for WasiType {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        panic!("Not implemented");
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
