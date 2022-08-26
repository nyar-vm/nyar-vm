use crate::{
    helpers::{IndexedIterator, IntoWasm},
    ArrayType, ExternalType, StructureType, WasmSymbol,
};
use indexmap::IndexMap;
use wast::token::Span;

use std::collections::{btree_map::Values, BTreeMap};
use wast::{
    component::{ComponentDefinedType, ComponentValType, PrimitiveValType},
    core::{HeapType, RefType, StorageType, ValType},
};

pub mod array;
pub mod external;

mod codegen;

pub struct TypeItem {
    pub name: WasmSymbol,
    pub r#type: WasmType,
}

#[derive(Clone, Debug)]
pub enum WasmType {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Unicode,
    UTF8Text,
    Any { nullable: bool },
    Structure(StructureType),
    Array(Box<ArrayType>),
}

impl WasmType {
    pub fn set_nullable(&mut self, _: bool) {}
}
