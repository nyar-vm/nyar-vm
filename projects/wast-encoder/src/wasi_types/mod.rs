use std::fmt::{Debug, Display, Formatter, Write};

use crate::{
    dag::DependentGraph, DependenciesTrace, ExternalFunction, Identifier, VariantType, WasiModule, WasiResource, WastEncoder,
};

mod display;

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

impl WasiType {
    pub fn wasm_module(&self) -> Option<&WasiModule> {
        match self {
            WasiType::Resource(v) => Some(&v.wasi_module),
            WasiType::External(v) => Some(&v.wasi_module),
            _ => None,
        }
    }
}

/// Mark for type who can import to the component instance
pub(crate) trait AliasOuter {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait AliasExport {
    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result;
}

/// Mark for type who can define in component section
pub(crate) trait ComponentDefine {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
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
