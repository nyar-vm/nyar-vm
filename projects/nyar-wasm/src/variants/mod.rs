use crate::{
    helpers::{IntoWasm, WasmName},
    StructureType, WasmSymbol, WasmType,
};
use nyar_error::FileSpan;
use std::collections::BTreeMap;
use wast::{
    component::ComponentDefinedType,
    token::{NameAnnotation, Span},
};

mod codegen;

#[derive(Clone, Debug)]
pub struct VariantType {
    pub symbol: WasmSymbol,
    pub fields: BTreeMap<String, StructureType>,
    pub span: FileSpan,
}

impl From<VariantType> for WasmType {
    fn from(value: VariantType) -> Self {
        WasmType::Variant(Box::new(value))
    }
}

impl VariantType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { symbol: name.into(), fields: Default::default(), span: Default::default() }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn set_field(&mut self, field: StructureType) {
        self.fields.insert(field.name(), field);
    }
    pub fn with_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = StructureType>,
    {
        for field in fields {
            self.set_field(field);
        }
        self
    }
}
