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

#[derive(Clone, Debug)]
pub struct EnumerateType {
    pub symbol: WasmSymbol,
    pub fields: BTreeMap<u64, WasmSymbol>,
    pub span: FileSpan,
}

#[derive(Clone, Debug)]
pub struct EncodingType {
    pub name: WasmSymbol,
    pub value: u64,
}

impl From<EnumerateType> for WasmType {
    fn from(value: EnumerateType) -> Self {
        WasmType::Structure(value)
    }
}

impl EnumerateType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { symbol: name.into(), fields: Default::default(), span: Default::default() }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn set_field(&mut self, field: EncodingType) {
        self.fields.insert(field.name.to_string(), field);
    }
    pub fn with_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = EncodingType>,
    {
        for field in fields {
            self.set_field(field);
        }
        self
    }
}

impl EncodingType {
    pub fn new(name: WasmSymbol) -> Self {
        Self { name, mutable: false, r#type: WasmType::Any { nullable: false }, value: WasmValue::Any }
    }
    pub fn with_type(self, r#type: WasmType) -> Self {
        Self { r#type, ..self }
    }
    pub fn with_default(self, default: WasmValue) -> Self {
        Self { value: default, ..self }
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
        self.value.as_type()
    }
}
