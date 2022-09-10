use crate::{
    helpers::{IntoWasm, WasmName},
    WasmSymbol, WasmType, WasmValue,
};
use nyar_error::FileSpan;
use wast::{
    core::{Type, TypeDef},
    token::{NameAnnotation, Span},
};

mod codegen;

impl From<ArrayType> for WasmType {
    fn from(value: ArrayType) -> Self {
        Self::Array(Box::new(value))
    }
}
#[derive(Clone, Debug)]
pub struct ArrayType {
    pub symbol: WasmSymbol,
    pub nullable: bool,
    pub mutable: bool,
    /// Item type of the array
    pub item_type: WasmType,
    pub default: WasmValue,
    pub span: FileSpan,
}

impl From<ArrayType> for WasmValue {
    fn from(value: ArrayType) -> Self {
        WasmValue::Array(Box::new(value))
    }
}

impl ArrayType {
    pub fn new<S: Into<WasmSymbol>>(name: S, item: WasmType) -> Self {
        Self {
            symbol: name.into(),
            nullable: false,
            mutable: false,
            item_type: item,
            default: WasmValue::Any,
            span: Default::default(),
        }
    }
}
