use super::*;

mod arithmetic;
mod display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiEnumeration {
    pub symbol: Identifier,
    pub variants: IndexMap<Arc<str>, WasiSemanticIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiSemanticIndex {
    pub name: Arc<str>,
    pub wasi_name: String,
}
