use crate::{
    dag::{DependencyItem, DependentGraph},
    DependencyLogger, Identifier, ResolveDependencies, WasiModule, WasiType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WasiResource {
    /// Resource language name
    pub symbol: Identifier,
    pub wasi_module: WasiModule,
    pub wasi_name: String,
}

impl WasiResource {
    pub fn new<S, M>(wasi_module: M, wasi_name: &str, name: S) -> Self
    where
        S: Into<Identifier>,
        M: Into<WasiModule>,
    {
        Self { symbol: name.into(), wasi_module: wasi_module.into(), wasi_name: wasi_name.to_string() }
    }
}

impl ResolveDependencies for WasiResource {
    fn collect_wasi_types(&self, dict: &mut DependentGraph) {
        dict.insert(DependencyItem::Item(WasiType::Resource(self.clone())));
    }

    fn trace_language_types(&self, _: &mut DependencyLogger) {}

    fn trace_modules(&self, dict: &mut DependencyLogger) {
        dict.wasi.insert(self.wasi_module.clone());
    }
}
