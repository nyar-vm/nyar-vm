use super::*;
use crate::{modules::WasmItem, WasmBuilder};

impl From<StructureItem> for StructureType {
    fn from(value: StructureItem) -> Self {
        StructureType { symbol: value.symbol, nullable: false, fields: value.fields }
    }
}

impl WasmItem for StructureItem {
    fn register(self, builder: &mut WasmBuilder) {
        StructureType::from(self).register(builder);
    }
}

impl WasmItem for StructureType {
    fn register(self, builder: &mut WasmBuilder) {
        builder.structures.insert(self.symbol.to_string(), self);
    }
}

impl From<StructureItem> for WasmType {
    fn from(value: StructureItem) -> Self {
        WasmType::Structure(value.into())
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
