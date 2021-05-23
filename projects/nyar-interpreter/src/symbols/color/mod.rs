#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SymbolColor {
    None,
    Keyword,
}

impl Default for SymbolColor {
    fn default() -> Self {
        SymbolColor::None
    }
}
