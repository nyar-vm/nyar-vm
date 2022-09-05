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
pub struct StructureType {
    pub symbol: WasmSymbol,
    pub fields: BTreeMap<String, FieldType>,
    pub span: FileSpan,
}

#[derive(Clone, Debug)]
pub struct FieldType {
    pub name: WasmSymbol,
    pub readonly: bool,
    pub r#type: WasmType,
    pub default: WasmValue,
}

impl From<StructureType> for WasmType {
    fn from(value: StructureType) -> Self {
        WasmType::Structure(value)
    }
}

impl StructureType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { symbol: name.into(), fields: Default::default(), span: Default::default() }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn set_field(&mut self, field: FieldType) {
        self.fields.insert(field.name.to_string(), field);
    }
    pub fn with_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = FieldType>,
    {
        for field in fields {
            self.set_field(field);
        }
        self
    }
}

impl FieldType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { name: name.into(), readonly: false, r#type: WasmType::Any { nullable: false }, default: WasmValue::Any }
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
        Self { readonly: false, ..self }
    }
    pub fn with_readonly(self) -> Self {
        Self { readonly: true, ..self }
    }
    pub fn r#type(&self) -> WasmType {
        self.default.as_type()
    }
}
