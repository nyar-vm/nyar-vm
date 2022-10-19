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

    fn collect_wasi_types(&self, dict: &mut DependentGraph) {
        match self {
            WasiType::Option { inner } => {
                inner.collect_wasi_types(dict);
            }
            WasiType::Result { success, failure } => {
                success.iter().for_each(|s| s.collect_wasi_types(dict));
                failure.iter().for_each(|f| f.collect_wasi_types(dict));
            }
            WasiType::Resource(v) => {
                dict.types_buffer.push(WasiType::Resource(v.clone()));
            }
            WasiType::Variant(v) => {
                dict.types_buffer.push(WasiType::Variant(v.clone()));
            }
            WasiType::TypeAlias { name } => {
                dict.types_buffer.extend(dict.types.get(name).cloned());
            }
            WasiType::TypeHandler { name, .. } => {
                dict.types_buffer.extend(dict.types.get(name).cloned());
                // println!("Buffer2: {:?}", name);
                // println!("Buffer3: {:?}", dict.types);
                // println!("Buffer4: {:?}", dict.types_buffer);
            }
            WasiType::External(v) => {
                dict.types_buffer.push(WasiType::External(v.clone()));
            }
            _ => {}
        };
    }
}
