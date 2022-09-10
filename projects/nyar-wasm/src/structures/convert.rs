use super::*;

impl<'a> From<&'a StructureItem> for StructureType {
    fn from(value: &'a StructureItem) -> Self {
        StructureType { symbol: value.symbol.clone(), nullable: false, fields: value.fields.clone() }
    }
}

impl From<StructureItem> for WasmType {
    fn from(value: StructureItem) -> Self {
        WasmType::Structure((&value).into())
    }
}
impl From<StructureItem> for WasmValue {
    fn from(value: StructureItem) -> Self {
        WasmValue::Structure(value)
    }
}
impl From<StructureType> for WasmType {
    fn from(value: StructureType) -> Self {
        WasmType::Structure(value)
    }
}
