use super::*;
use crate::WasiModule;

/// wasi export path
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WasiImport {
    ///
    pub module: WasiModule,
    ///
    pub name: Arc<str>,
}
