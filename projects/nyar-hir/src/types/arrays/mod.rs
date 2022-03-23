use super::*;

impl From<ArrayType> for TypeItem {
    fn from(value: ArrayType) -> Self {
        Self::Array(value)
    }
}
pub struct ArrayType {
    pub namepath: Identifier,
    pub mutable: bool,
    /// Item type of the array
    pub item_type: NyarType,
}

impl ArrayType {
    pub fn new(name: Identifier, item: NyarType) -> Self {
        Self { namepath: name, mutable: false, item_type: item }
    }
}
