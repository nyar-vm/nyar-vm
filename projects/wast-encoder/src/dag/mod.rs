use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Debug, Formatter},
    ops::AddAssign,
    sync::Arc,
};

use semver::Version;

use crate::{ExternalFunction, Identifier, VariantType, WasiInstance, WasiModule, WasiResource, WasiType};

mod arithmetic;

pub trait ResolveDependencies {
    fn trace_language_types(&self, dict: &mut DependencyLogger);
    fn trace_modules(&self, dict: &mut DependencyLogger);
    fn dependent_modules(&self, registry: &DependentRegistry) -> BTreeSet<WasiModule> {
        let mut logger = DependencyLogger::default();
        self.trace_language_types(&mut logger);
        // TODO: remove clone
        let types = logger.types.clone();

        for names in types {
            match registry.types.get(&names) {
                None => {
                    println!("Type {} not found", names);
                }
                Some(s) => s.trace_modules(&mut logger),
            }
        }
        self.trace_modules(&mut logger);
        logger.wasi
    }
}

#[derive(Default)]
pub struct DependentRegistry {
    modules: BTreeMap<WasiModule, Version>,
    types: BTreeMap<Identifier, WasiType>,
    functions: BTreeMap<Arc<str>, ExternalFunction>,
}

impl DependentRegistry {
    pub fn group_instances(&self) -> BTreeMap<WasiModule, WasiInstance> {
        let mut instances = BTreeMap::new();
        for (id, item) in self.types.iter() {
            match item {
                WasiType::Integer8 { .. } => {}
                WasiType::Integer16 { .. } => {}
                WasiType::Integer32 { .. } => {}
                WasiType::Integer64 { .. } => {}
                WasiType::Option { .. } => {}
                WasiType::Result { .. } => {}
                WasiType::Resource(v) => {
                    let module = v.wasi_module.clone();
                    let instance = instances.entry(module).or_insert(WasiInstance::new(v.wasi_module.clone()));
                    instance.resources.insert(id.clone(), v.clone());
                }
                WasiType::Variant(_) => {}
                WasiType::TypeHandler { .. } => {}
                WasiType::TypeAlias { .. } => {}
            }
        }
        instances
    }
}

impl Debug for DependentRegistry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DependentRegistry")
            .field("modules", &self.modules)
            .field("types", &self.types.keys())
            .field("functions", &self.functions.keys())
            .finish()
    }
}

impl AddAssign<VariantType> for DependentRegistry {
    fn add_assign(&mut self, rhs: VariantType) {
        self.types.insert(rhs.symbol.clone(), WasiType::Variant(rhs));
    }
}

#[derive(Default, Debug)]
pub struct DependencyLogger {
    pub(crate) types: BTreeSet<Identifier>,
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
