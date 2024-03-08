use super::*;
use crate::WasiModule;

/// wasi export path
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WasiImport {
    ///
    pub module: Option<WasiModule>,
    ///
    pub name: Arc<str>,
}
