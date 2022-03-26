use super::*;
use nyar_error::FileSpan;

impl From<ArrayType> for TypeItem {
    fn from(value: ArrayType) -> Self {
        Self::Array(value)
    }
}
pub struct ArrayType {
    pub symbol: Symbol,
    pub mutable: bool,
    /// Item type of the array
    pub item_type: NyarType,
    pub span: FileSpan,
}

impl ArrayType {
    pub fn new(name: Symbol, item: NyarType) -> Self {
        Self { symbol: name, mutable: false, item_type: item, span: Default::default() }
    }
}
