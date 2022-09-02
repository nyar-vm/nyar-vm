use crate::{
    helpers::{IntoWasm, WasmName},
    values::WasmValue,
    WasmSymbol, WasmType,
};
use nyar_error::FileSpan;
use std::collections::BTreeMap;
use wast::{
    component::{ComponentDefinedType, Record, RecordField},
    core::{StructField, StructType},
    token::{NameAnnotation, Span},
};

mod codegen;

#[derive(Clone, Debug)]
pub struct VariantType {
    pub symbol: WasmSymbol,
    pub fields: BTreeMap<String, VariantFieldType>,
    pub span: FileSpan,
}

#[derive(Clone, Debug)]
pub struct VariantFieldType {
    pub name: WasmSymbol,
    pub mutable: bool,
    pub r#type: WasmType,
    pub default: WasmValue,
}

impl From<VariantType> for WasmType {
    fn from(value: VariantType) -> Self {
        WasmType::Structure(value)
    }
}

impl VariantType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { symbol: name.into(), fields: Default::default(), span: Default::default() }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn set_field(&mut self, field: VariantFieldType) {
        self.fields.insert(field.name.to_string(), field);
    }
    pub fn with_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = VariantFieldType>,
    {
        for field in fields {
            self.set_field(field);
        }
        self
    }
}

impl VariantFieldType {
    pub fn new(name: WasmSymbol) -> Self {
        Self { name, mutable: false, r#type: WasmType::Any { nullable: false }, default: WasmValue::Any }
    }
    pub fn with_type(self, r#type: WasmType) -> Self {
        Self { r#type, ..self }
    }
    pub fn with_default(self, default: WasmValue) -> Self {
        Self { default, ..self }
    }

    pub fn set_nullable(&mut self, nullable: bool) {
        self.r#type.set_nullable(nullable);
    }

    pub fn with_mutable(self) -> Self {
        Self { mutable: true, ..self }
    }
    pub fn with_readonly(self) -> Self {
        Self { mutable: false, ..self }
    }
    pub fn r#type(&self) -> WasmType {
        self.default.as_type()
    }
}
