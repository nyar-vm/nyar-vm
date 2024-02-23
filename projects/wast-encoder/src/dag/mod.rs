use std::{
    collections::{BTreeMap, BTreeSet},
    ops::AddAssign,
    sync::Arc,
};

use semver::Version;

use crate::{wasi_module::WasiModule, WasiFunction, WasiInstance, WasiParameter, WasiResource, WasiType};

mod arithmetic;

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
    resources: BTreeMap<Arc<str>, WasiResource>,
    types: BTreeMap<Arc<str>, WasiParameter>,
    functions: BTreeMap<Arc<str>, WasiFunction>,
}

#[derive(Default, Debug)]
pub struct DependencyLogger {
    pub(crate) types: BTreeSet<Arc<str>>,
    pub(crate) wasi: BTreeSet<WasiModule>,
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
            match typing.r#type {
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
