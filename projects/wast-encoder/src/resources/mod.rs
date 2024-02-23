use std::sync::Arc;

use crate::WasiModule;

#[derive(Clone, Debug)]
pub struct WasiResource {
    /// Resource language name
    pub name: Arc<str>,
    pub wasi_module: WasiModule,
    pub wasi_name: String,
}

impl crate::WasiResource {
    pub fn new<S, M>(wasi_module: M, wasi_name: &str, name: S) -> Self
        where
            S: Into<Arc<str>>,
            M: Into<WasiModule>,
    {
        Self { name: name.into(), wasi_module: wasi_module.into(), wasi_name: wasi_name.to_string() }
    }
}
