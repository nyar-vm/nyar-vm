use std::{
    collections::{BTreeMap, VecDeque},
    fmt::{Debug, Formatter},
    mem::take,
    ops::{AddAssign, Index},
    str::FromStr,
};

use petgraph::{algo::toposort, data::FromElements, graph::DiGraph};

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

fn top_sort(adj: Vec<Vec<usize>>, mut indeg: Vec<u32>) -> Vec<usize> {
    let mut q = indeg.iter().enumerate().filter(|(_, &d)| d == 0).map(|(node, _)| node).collect::<VecDeque<_>>();
    let mut ret = Vec::new();
    while let Some(node) = q.pop_front() {
        ret.push(node);
        for &nnode in adj[node].iter() {
            indeg[nnode] -= 1;
            if indeg[nnode] == 0 {
                q.push_back(nnode)
            }
        }
    }
    ret
}
struct Solution {}

impl Solution {
    pub fn sort_items(group_count: i32, mut group: Vec<i32>, before_items: Vec<Vec<i32>>) -> Vec<i32> {
        let items = before_items.len();
        let mut indeg = vec![0; items];
        let mut adj = vec![vec![]; items];
        for (item, deps) in before_items.iter().enumerate() {
            for &dep in deps {
                adj[dep as usize].push(item);
                indeg[item] += 1;
            }
        }
        let items_ids = top_sort(adj, indeg);
        if items_ids.len() != items {
            return vec![];
        }

        let mut groups = items_ids.into_iter().fold(vec![vec![]; group_count as usize], |mut acc, i| {
            if group[i] == -1 {
                group[i] = acc.len() as i32;
                acc.push(vec![]);
            }
            acc[group[i] as usize].push(i as i32);
            acc
        });

        let mut indeg = vec![0; groups.len()];
        let mut adj = vec![vec![]; groups.len()];
        for (item, deps) in before_items.into_iter().enumerate() {
            for dep in deps {
                let (src, dst) = (group[dep as usize] as usize, group[item] as usize);
                if src == dst {
                    continue;
                }
                adj[src].push(dst);
                indeg[dst] += 1;
            }
        }
        let group_ids = top_sort(adj, indeg);
        if group_ids.len() != groups.len() {
            return vec![];
        }

        group_ids.into_iter().fold(Vec::new(), |mut acc, i| {
            acc.append(&mut groups[i]);
            acc
        })
    }
}
