use crate::{DependencyLogger, Identifier, ResolveDependencies, WasiModule};

#[derive(Clone, Debug)]
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
    fn trace_language_types(&self, _: &mut DependencyLogger) {}

    fn trace_modules(&self, dict: &mut DependencyLogger) {
        dict.wasi.insert(self.wasi_module.clone());
    }
}
