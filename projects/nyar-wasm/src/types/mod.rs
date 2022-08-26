use crate::{
    helpers::{IndexedIterator, IntoWasm},
    ArrayType, ExternalType, StructureType, WasmSymbol,
};
use indexmap::IndexMap;

use std::collections::{btree_map::Values, BTreeMap};
use wast::core::{HeapType, RefType, StorageType, ValType};

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

impl<'a, 'i> IntoWasm<'a, ValType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ValType<'i> {
        match self {
            Self::Bool => ValType::I32,
            Self::U8 => ValType::I32,
            Self::U16 => ValType::I32,
            Self::U32 => ValType::I32,
            Self::U64 => ValType::I64,
            Self::I8 => ValType::I32,
            Self::I16 => ValType::I32,
            Self::I32 => ValType::I32,
            Self::I64 => ValType::I64,
            Self::F32 => ValType::F32,
            Self::F64 => ValType::F64,
            Self::Unicode => ValType::I32,
            Self::Any { nullable } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Any }),
            Self::Structure(v) => ValType::Ref(RefType { nullable: false, heap: HeapType::Struct }),
            _ => unimplemented!("Cast `{:?}` to core value type fail", self),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, StorageType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> StorageType<'i> {
        match self {
            WasmType::I8 => StorageType::I8,
            WasmType::I16 => StorageType::I16,
            _ => StorageType::Val(self.as_wast()),
        }
    }
}
