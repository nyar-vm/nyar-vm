use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
    ops::AddAssign,
    sync::Arc,
};

use crate::{
    dag::DependentGraph, DependencyLogger, ExternalFunction, Identifier, ResolveDependencies, WasiModule, WasiResource,
};

mod convert;
mod display;

pub struct WasiInstance {
    pub module: WasiModule,
    /// language_name: wasi_name
    pub resources: BTreeMap<Identifier, WasiResource>,
    pub functions: BTreeMap<Arc<str>, ExternalFunction>,
}

impl WasiInstance {
    pub fn new<M>(module: M) -> Self
    where
        M: Into<WasiModule>,
    {
        Self { module: module.into(), resources: Default::default(), functions: Default::default() }
    }
}

impl ResolveDependencies for WasiInstance {
    fn collect_wasi_types(&self, dict: &mut DependentGraph) {
        for (_, resource) in &self.resources {
            resource.collect_wasi_types(dict);
        }
        for (_, function) in &self.functions {
            function.collect_wasi_types(dict);
        }
    }

    fn trace_language_types(&self, dict: &mut DependencyLogger) {
        for (_, resource) in &self.resources {
            resource.trace_modules(dict);
        }
        for (_, function) in &self.functions {
            function.trace_modules(dict);
        }
    }

    fn trace_modules(&self, dict: &mut DependencyLogger) {
        dict.wasi.remove(&self.module);
    }
}
