use std::{collections::BTreeMap, fmt::Debug, ops::AddAssign};

use dependent_sort::{DependentSort, TopologicalError};

use crate::{helpers::GroupedTask, CanonicalImport, Identifier, WasiInstance, WasiModule, WasiType, WasiTypeReference};

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
    pub fn get(&self, type_id: &WasiTypeReference) -> &WasiType {
        match self.types.get(&type_id.symbol) {
            Some(s) => s,
            None => panic!("Missing Type `{}` in DependentGraph", type_id.symbol),
        }
    }

    fn build_dag(&self) -> DependentSort<WasiType, WasiModule> {
        let mut sorter = DependentSort::default();
        for ty in self.types.values() {
            match ty.dependent_task(self) {
                Some(s) => {
                    sorter += s;
                }
                None => {}
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
