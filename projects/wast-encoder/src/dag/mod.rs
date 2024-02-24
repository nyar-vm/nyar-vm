use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{Debug, Formatter},
    mem::take,
    ops::AddAssign,
};

use petgraph::{algo::toposort, graph::DiGraph};

use crate::{Identifier, WasiModule, WasiType};

mod arithmetic;

pub trait ResolveDependencies {
    fn define_language_types(&self, dict: &mut DependentGraph);
    fn collect_wasi_types(&self, dict: &mut DependentGraph);
}

#[derive(Default, Debug)]
pub struct DependentGraph {
    dag: DiGraph<DependencyItem, ()>,
    pub(crate) types: BTreeMap<Identifier, WasiType>,
    pub(crate) types_buffer: Vec<WasiType>,
}

#[derive(Clone, Eq, PartialEq)]
pub enum DependencyItem {
    Module(WasiModule),
    Item(WasiType),
}

impl Debug for DependencyItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Module(s) => write!(f, "Module({})", s),
            Self::Item(WasiType::External(v)) => write!(f, "External({}, {})", v.wasi_module, v.name),
            Self::Item(WasiType::Resource(v)) => write!(f, "Resource({}, {})", v.wasi_module, v.wasi_name),
            Self::Item(WasiType::Variant(v)) => write!(f, "Variant({})", v.wasi_name),
            _ => panic!("Not implemented: {:?}", self),
        }
    }
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

    pub fn insert_with_dependency(&mut self, item: DependencyItem, dependent: DependencyItem) {
        let node = self.insert(item);
        let dependency = self.insert(dependent);
        self.dag.add_edge(node, dependency, ());
    }

    pub(crate) fn finalize_buffer(&mut self, r#type: WasiType) {
        // println!("Buffer: {:?}", self.types_buffer);
        for i in take(&mut self.types_buffer) {
            self.insert_with_dependency(DependencyItem::Item(r#type.clone()), DependencyItem::Item(i));
        }
    }

    pub fn finalize(mut self) -> Self {
        // TODO: remove clone here
        let types = self.types.clone();
        // println!("{:#?}", types);
        for typing in types.values() {
            match typing {
                WasiType::Integer8 { .. } => {
                    panic!()
                }
                WasiType::Integer16 { .. } => {
                    panic!()
                }
                WasiType::Integer32 { .. } => {
                    panic!()
                }
                WasiType::Integer64 { .. } => {
                    panic!()
                }
                WasiType::Option { .. } => {
                    panic!()
                }
                WasiType::Result { .. } => {
                    panic!()
                }
                WasiType::Resource(v) => v.collect_wasi_types(&mut self),
                WasiType::Variant(v) => {
                    // println!("Variant {:?}", v);
                    v.collect_wasi_types(&mut self)
                }
                WasiType::TypeHandler { .. } => {
                    panic!()
                }
                WasiType::TypeAlias { .. } => {
                    panic!()
                }
                WasiType::External(v) => v.collect_wasi_types(&mut self),
            }
            // println!("{} {:?}", typing, self.dag);
        }
        self
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

// Why is the following code error, how should I modify it to pass the test?
type GroupId = usize;
type TaskId = usize;

#[derive(Default)]
pub struct TaskGroup {
    tasks: HashMap<TaskId, Task>,
}
pub struct Group {
    id: GroupId,
    dependent_groups: Vec<GroupId>,
}

pub struct Task {
    id: usize,
    group: Option<GroupId>,
    dependent_tasks: Vec<TaskId>,
}

impl AddAssign<Task> for TaskGroup {
    fn add_assign(&mut self, rhs: Task) {
        self.tasks.insert(rhs.id, rhs);
    }
}

impl TaskGroup {
    pub fn arrange_order(&self) -> Vec<TaskId> {
        let mut sorted: Vec<TaskId> = Vec::new();
        let mut visited_tasks: HashSet<TaskId> = HashSet::new();
        let mut visited_groups: HashSet<GroupId> = HashSet::new();
        for (_, task) in &self.tasks {
            if let Some(group_id) = task.group {
                if !visited_groups.contains(&group_id) {
                    self.topological_sort(group_id, &mut sorted, &mut visited_tasks);
                    visited_groups.insert(group_id);
                }
            }
        }
        for (_, task) in &self.tasks {
            if !visited_tasks.contains(&task.id) {
                self.topological_sort(task.id, &mut sorted, &mut visited_tasks);
            }
        }
        sorted.into_iter().rev().collect()
    }

    fn topological_sort(&self, id: usize, sorted: &mut Vec<usize>, visited: &mut HashSet<usize>) {
        if visited.contains(&id) {
            return;
        }
        visited.insert(id);
        if let Some(task) = self.tasks.get(&id) {
            for dependent_id in &task.dependent_tasks {
                self.topological_sort(*dependent_id, sorted, visited);
            }
        }
        sorted.push(id);
    }
}

#[test]
fn test() {
    let mut tasks = TaskGroup::default();
    tasks += Task { id: 0, group: None, dependent_tasks: vec![] };
    tasks += Task { id: 1, group: None, dependent_tasks: vec![6] };
    tasks += Task { id: 2, group: Some(1), dependent_tasks: vec![5] };
    tasks += Task { id: 3, group: Some(0), dependent_tasks: vec![6] };
    tasks += Task { id: 4, group: Some(0), dependent_tasks: vec![3, 6] };
    tasks += Task { id: 5, group: Some(1), dependent_tasks: vec![] };
    tasks += Task { id: 6, group: Some(0), dependent_tasks: vec![] };
    tasks += Task { id: 7, group: None, dependent_tasks: vec![] };
    let sorted = tasks.arrange_order();
    assert_eq!(sorted, vec![6, 3, 4, 1, 5, 2, 0, 7]);
}
