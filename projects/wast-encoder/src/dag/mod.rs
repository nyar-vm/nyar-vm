use std::{
    collections::{BTreeMap, BTreeSet},
    ops::AddAssign,
    sync::Arc,
};

use semver::Version;

use crate::{wasi_module::WasiModule, WasiFunction, WasiInstance, WasiParameter, WasiType};

pub trait ResolveDependencies {
    fn trace_language_types(&self, dict: &mut DependencyLogger);
    fn dependent_modules(&self, registry: &DependentRegistry) -> BTreeSet<WasiModule> {
        let mut logger = DependencyLogger::default();
        self.trace_language_types(&mut logger);
        logger.link_wasi_modules(&registry);
        logger.wasi
    }
}

#[derive(Default)]
pub struct DependentRegistry {
    modules: BTreeMap<WasiModule, Version>,
    types: BTreeMap<Arc<str>, WasiParameter>,
    functions: BTreeMap<Arc<str>, WasiFunction>,
}

#[derive(Default)]
pub struct DependencyLogger {
    types: BTreeSet<Arc<str>>,
    wasi: BTreeSet<WasiModule>,
}

impl AddAssign<WasiParameter> for DependentRegistry {
    fn add_assign(&mut self, rhs: WasiParameter) {
        self.types.insert(rhs.name.clone(), rhs);
    }
}

impl AddAssign<WasiFunction> for DependentRegistry {
    fn add_assign(&mut self, rhs: WasiFunction) {
        self.functions.insert(rhs.name.clone(), rhs);
    }
}

impl DependentRegistry {
    pub fn change_version(&mut self, module: WasiModule, version: Version) {
        self.modules.insert(module, version);
    }
    pub fn erase_version(&mut self, module: &WasiModule) {
        self.modules.remove(module);
    }
    pub fn update_versions(&mut self) {
        for typing in self.types.values_mut() {
            match typing {
                WasiType::Option { .. } => {}
                WasiType::Result { .. } => {}
                WasiType::Resource(_) => {}
                WasiType::Variant(_) => {}
                WasiType::TypeHandler { .. } => {}
                WasiType::TypeAlias { .. } => {}
                _ => {}
            }
        }
    }
}

impl DependencyLogger {
    pub fn link_wasi_modules(&mut self, registry: &DependentRegistry) {}
}

impl ResolveDependencies for WasiInstance {
    fn trace_language_types(&self, dict: &mut DependencyLogger) {
        for function in self.functions.values() {
            for input in &function.inputs {
                input.r#type.trace_language_types(dict)
            }
            for ret in function.output.iter() {
                ret.trace_language_types(dict)
            }
        }
    }
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
            Self::Variant(v) => v.variants.values().for_each(|v| v.trace_language_types(dict)),
            Self::TypeHandler { name, .. } => {
                dict.types.insert(name.clone());
            }
            Self::TypeAlias { .. } => {}
            _ => {}
        };
    }
}
