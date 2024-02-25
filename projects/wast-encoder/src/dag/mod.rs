use std::{collections::BTreeMap, fmt::Debug, ops::AddAssign};

use dependent_sort::{DependentSort, TopologicalError};

use crate::{CanonicalImport, Identifier, WasiInstance, WasiModule, WasiType};

mod arithmetic;

pub(crate) trait DependenciesTrace {
    fn define_language_types(&self, dict: &mut DependentGraph);
    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i;
}

#[derive(Default, Debug)]
pub struct DependentGraph {
    pub(crate) types: BTreeMap<Identifier, WasiType>,
}

impl DependentGraph {
    fn build_dag(&self) -> DependentSort<WasiType, WasiModule> {
        let mut sorter = DependentSort::default();
        for ty in self.types.values() {
            let mut dependents: Vec<&WasiType> = vec![];
            match ty {
                WasiType::Integer8 { .. } => {}
                WasiType::Integer16 { .. } => {}
                WasiType::Integer32 { .. } => {}
                WasiType::Integer64 { .. } => {}
                WasiType::Option { .. } => {}
                WasiType::Result { .. } => {}
                WasiType::Resource(v) => {
                    sorter += dependent_sort::Task::new(ty).with_group(&v.wasi_module);
                }
                WasiType::Variant(v) => {
                    v.collect_wasi_types(self, &mut dependents);
                    sorter += dependent_sort::Task::new_with_dependent(&ty, dependents);
                }
                WasiType::TypeHandler { .. } => {}
                WasiType::TypeAlias { .. } => {}
                WasiType::External(v) => {
                    v.collect_wasi_types(self, &mut dependents);
                    sorter += dependent_sort::Task { id: ty, group: Some(&v.wasi_module), dependent_tasks: dependents };
                }
            }
        }
        sorter
    }
    pub fn resolve_imports(&self) -> Result<Vec<CanonicalImport>, TopologicalError<WasiType, WasiModule>> {
        let mut imports = vec![];
        for group in self.build_dag().sort_grouped_hash_specialization()? {
            match group.id {
                Some(s) => {
                    let mut instance = WasiInstance::new(s.clone());
                    for task in group.tasks {
                        instance.insert(task);
                    }
                    imports.push(CanonicalImport::Instance(instance));
                }
                None => {
                    for task in group.tasks {
                        // only one task in fact
                        imports.push(CanonicalImport::Type(task.clone()))
                    }
                }
            }
        }
        Ok(imports)
    }
}
