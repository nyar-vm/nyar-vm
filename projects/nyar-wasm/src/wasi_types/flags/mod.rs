use super::*;

mod arithmetic;
mod display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiFlags {
    pub symbol: Identifier,
    pub variants: IndexMap<Arc<str>, WasiSemanticIndex>,
}

impl Display for WasiFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}
