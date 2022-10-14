use std::{collections::BTreeSet, sync::Arc};

use crate::{wasi_module::WasiModule, wasi_types::DependencyLogger, WasiType};

pub trait ResolveDependencies {
    fn resolve_language_types(&self, dict: &mut DependencyLogger) {}
    fn resolve_wasi_module(&self, dict: &mut DependencyLogger) {}
}

#[derive(Default)]
pub struct DependencyLogger {
    types: BTreeSet<Arc<str>>,
    wasi: BTreeSet<WasiModule>,
}

impl ResolveDependencies for WasiType {
    fn resolve_language_types(&self, dict: &mut DependencyLogger) {
        match self {
            Self::Option { inner } => inner.resolve_dependencies(dict),
            Self::Result { success, failure } => {
                success.iter().for_each(|s| s.resolve_dependencies(dict));
                failure.iter().for_each(|f| f.resolve_dependencies(dict));
            }
            Self::Resource(_) => {}
            Self::Variant(v) => v.variants.values().for_each(|v| v.resolve_dependencies(dict)),
            Self::TypeHandler { name, .. } => {
                *dict += name.clone();
            }
            Self::TypeAlias { .. } => {}
            _ => {}
        };
    }
}
