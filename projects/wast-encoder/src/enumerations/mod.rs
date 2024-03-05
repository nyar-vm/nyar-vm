use std::sync::Arc;

use indexmap::IndexMap;

mod display;

pub struct WasiEnumeration {
    pub name: Arc<str>,
    pub wasi_name: String,
    pub variants: IndexMap<Arc<str>, WasiEnumerationItem>,
}

pub struct WasiEnumerationItem {
    pub name: Arc<str>,
    pub wasi_name: String,
}
