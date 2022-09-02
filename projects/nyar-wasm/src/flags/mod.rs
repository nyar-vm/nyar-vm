use crate::{
    helpers::{IntoWasm, WasmName},
    WasmSymbol, WasmType,
};
use nyar_error::FileSpan;
use std::collections::BTreeMap;
use wast::{component::ComponentDefinedType, token::Span};
mod codegen;

#[derive(Clone, Debug)]
pub struct FlagType {
    pub symbol: WasmSymbol,
    pub fields: BTreeMap<u64, EncodingType>,
    pub span: FileSpan,
}

#[derive(Clone, Debug)]
pub struct EncodingType {
    pub name: WasmSymbol,
    pub value: u64,
}

impl From<FlagType> for WasmType {
    fn from(value: FlagType) -> Self {
        WasmType::Flag(value)
    }
}

impl FlagType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self { symbol: name.into(), fields: Default::default(), span: Default::default() }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn set_field(&mut self, field: EncodingType) {
        self.fields.insert(field.value, field);
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
