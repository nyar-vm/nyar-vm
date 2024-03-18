use super::*;

mod arithmetic;
mod display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiFlags {
    pub symbol: Identifier,
    pub flags: Vec<WasiSemanticIndex>,
}
