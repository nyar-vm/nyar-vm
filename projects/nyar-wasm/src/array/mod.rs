use crate::{
    helpers::{IntoWasm, WasmName},
    WasmSymbol, WasmType,
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
    pub mutable: bool,
    /// Item type of the array
    pub item_type: WasmType,
    pub span: FileSpan,
}

impl ArrayType {
    pub fn new<S: Into<WasmSymbol>>(name: S, item: WasmType) -> Self {
        Self { symbol: name.into(), mutable: false, item_type: item, span: Default::default() }
    }
}
