use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

use crate::{DependencyLogger, encode_id, ResolveDependencies, VariantType, WasiResource};

mod display;

#[derive(Clone, Debug)]
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
        name: Arc<str>,
        own: bool,
    },
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Arc<str>,
    },
}

impl ResolveDependencies for WasiType {
    fn trace_language_types(&self, dict: &mut DependencyLogger) {
        match self {
            Self::Option { inner } => inner.trace_language_types(dict),
            Self::Result { success, failure } => {
                success.iter().for_each(|s| s.trace_language_types(dict));
                failure.iter().for_each(|f| f.trace_language_types(dict));
            }
            Self::Resource(_) => {}
            Self::Variant(v) => v.variants.values().for_each(|v| v.r#type.trace_language_types(dict)),
            Self::TypeHandler { name, .. } => {
                dict.types.insert(name.clone());
            }
            Self::TypeAlias { name } => {
                dict.types.insert(name.clone());
            }
            _ => {}
        };
    }
}
