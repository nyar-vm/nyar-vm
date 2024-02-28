use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Debug, Formatter},
    ops::{AddAssign, Index},
    str::FromStr,
    sync::Arc,
};

use petgraph::{algo::toposort, data::FromElements, graph::DiGraph};
use semver::Version;

use crate::{ExternalFunction, Identifier, VariantType, WasiInstance, WasiModule, WasiResource, WasiType};

mod arithmetic;

pub trait ResolveDependencies {
    fn collect_wasi_types(&self, dict: &mut DependentGraph);
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
                WasiType::External(_) => {}
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

#[derive(Debug, Default)]
pub struct WasmType {
    name: &'static str,
    module: Option<&'static str>,
    dependencies: Vec<&'static str>,
}

impl WasmType {
    pub fn new(name: &'static str) -> Self {
        Self { name, module: None, dependencies: vec![] }
    }
    pub fn with_module(mut self, module: &'static str) -> Self {
        self.module = Some(module);
        self
    }
    pub fn with_dependency(mut self, dependency: &'static str) -> Self {
        self.dependencies.push(dependency);
        self
    }
}
// #[test]
// fn test_dag() {
//     let mut root = Dag::<WasmType, String, u32>::new();
//     let function_write = root.add_node(WasmType::new("write".to_string()));
//     let (_, stream_error) =
//         root.add_child(function_write, "stream-error".to_string(), WasmType::new("stream-error".to_string()));
//     let (_, io_error) = root.add_child(stream_error, "io-error".to_string(), WasmType::new("io-error".to_string()));
//
//     for (edge, node) in root.recursive_walk(&root) {
//         let item = root.index(node);
//         println!("{:?}", item);
//     }
// }

#[derive(Default, Debug)]
pub struct DependentGraph {
    dag: DiGraph<DependencyItem, ()>,
    types: BTreeSet<WasiType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyItem {
    Item(WasiType),
    Module(WasiModule),
}

impl DependentGraph {
    pub fn insert(&mut self, item: DependencyItem) -> petgraph::graph::NodeIndex {
        for index in self.dag.node_indices() {
            match self.dag.node_weight(index) {
                Some(s) if item.eq(s) => return index,
                _ => continue,
            }
        }
        self.dag.add_node(item)
    }
    pub fn insert_with_dependency(&mut self, item: WasiType, dependent: DependencyItem) {
        let node = self.insert(DependencyItem::Item(item));
        let dependency = self.insert(dependent);
        self.dag.add_edge(node, dependency, ());
    }
    pub fn topological_sort(&self) -> Vec<DependencyItem> {
        let mut output = Vec::with_capacity(self.dag.node_count());
        let sort = match toposort(&self.dag, None) {
            Ok(o) => o,
            Err(e) => {
                panic!("{:?}", e)
            }
        };
        for node in sort.into_iter().rev() {
            match self.dag.node_weight(node) {
                Some(s) => output.push(s.clone()),
                None => continue,
            }
        }
        output
    }
}
