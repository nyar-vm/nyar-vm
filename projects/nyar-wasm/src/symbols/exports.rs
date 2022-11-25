use super::*;
use crate::WasiModule;

/// wasi import path
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WasiExport {
    ///
    pub module: WasiModule,
    ///
    pub name: Arc<str>,
}
