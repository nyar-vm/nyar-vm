use crate::{
    helpers::{IndexedIterator, IntoWasm},
    ArrayType, EnumerateType, ExternalType, FlagType, VariantType, WasmSymbol,
};
use indexmap::IndexMap;
use wast::{component::CoreType, token::Span};

use crate::structures::StructureType;
use wast::{
    component::{ComponentDefinedType, ComponentValType, PrimitiveValType},
    core::{HeapType, RefType, StorageType, ValType},
};

pub mod array;
pub mod external;

mod codegen;

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
    Flag(FlagType),
    Enumerate(EnumerateType),
    Structure(StructureType),
    Variant(Box<VariantType>),
    Reference { name: WasmSymbol, nullable: bool },
    Array(Box<ArrayType>),
}

impl WasmType {
    pub fn set_nullable(&mut self, null: bool) {
        match self {
            Self::Bool => {}
            Self::U8 => {}
            Self::U16 => {}
            Self::U32 => {}
            Self::U64 => {}
            Self::I8 => {}
            Self::I16 => {}
            Self::I32 => {}
            Self::I64 => {}
            Self::F32 => {}
            Self::F64 => {}
            Self::Unicode => {}
            Self::UTF8Text => {}
            Self::Any { .. } => {}
            Self::Flag(_) => {}
            Self::Enumerate(_) => {}
            Self::Structure(v) => v.nullable = null,
            Self::Variant(_) => {}
            Self::Array(_) => {}
            Self::Reference { nullable, .. } => *nullable = null,
        }
    }

    pub fn with_nullable(mut self, null: bool) -> Self {
        self.set_nullable(null);
        self
    }
}
