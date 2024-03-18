use super::*;

mod arithmetic;
mod display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiEnumeration {
    pub symbol: Identifier,
    pub enumerations: Vec<WasiSemanticIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiSemanticIndex {
    pub name: Arc<str>,
    pub wasi_name: Arc<str>,
}
