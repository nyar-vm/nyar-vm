use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Debug, Formatter, Write},
    ops::AddAssign,
};

use crate::{
    dag::DependenciesTrace, helpers::ComponentDefine, DependentGraph, Identifier, WasiExternalFunction, WasiModule,
    WasiResource, WasiType, WastEncoder,
};

mod convert;
mod display;

pub struct WasiInstance {
    pub module: WasiModule,
    /// language_name: wasi_name
    pub resources: BTreeMap<Identifier, WasiResource>,
    pub functions: BTreeMap<Identifier, WasiExternalFunction>,
}

impl WasiInstance {
    pub fn new<M>(module: M) -> Self
    where
        M: Into<WasiModule>,
    {
        Self { module: module.into(), resources: Default::default(), functions: Default::default() }
    }
    pub fn insert(&mut self, wasi: &WasiType) {
        match wasi {
            WasiType::Integer8 { .. } => {}
            WasiType::Integer16 { .. } => {}
            WasiType::Integer32 { .. } => {}
            WasiType::Integer64 { .. } => {}
            WasiType::Option { .. } => {
                todo!()
            }
            WasiType::Result { .. } => {
                todo!()
            }
            WasiType::Resource(v) => {
                self.resources.insert(v.symbol.clone(), v.clone());
            }
            WasiType::Variant(_) => {
                todo!()
            }
            WasiType::TypeHandler { .. } => {
                todo!()
            }

            WasiType::External(v) => {
                self.functions.insert(v.symbol.clone(), *v.clone());
            }
            WasiType::Array { .. } => {}
            WasiType::Float32 => {}
            WasiType::Float64 => {}
            WasiType::Record(_) => {}
            WasiType::Boolean => {}
        }
    }
    pub fn dependencies(&self, dict: &DependentGraph) -> BTreeSet<WasiType> {
        let mut dependencies = BTreeSet::default();
        let mut types = vec![];
        self.functions.values().for_each(|f| f.collect_wasi_types(dict, &mut types));
        for ty in types {
            match ty.wasm_module() {
                Some(s) if s.eq(&self.module) => continue,
                _ => {
                    dependencies.insert(ty.clone());
                }
            }
        }
        dependencies
    }
}
