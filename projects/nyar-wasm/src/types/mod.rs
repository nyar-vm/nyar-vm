use crate::{
    helpers::{Id, WasmInstruction, WasmOutput},
    ArrayType, StructureType,
};
use indexmap::IndexMap;
use nyar_error::{FileSpan, NyarError};
use nyar_hir::{FunctionBody, IndexedIterator, NyarType, NyarValue, ParameterType, Symbol};
use wast::{
    core::{
        Expression, Func, FuncKind, FunctionType, HeapType, Import, InlineExport, ItemKind, ItemSig, ModuleField, RefType,
        StorageType, StructField, StructType, Type, TypeDef, TypeUse, ValType,
    },
    token::{NameAnnotation, Span},
};

pub mod array;
pub mod external;
pub mod functional;
pub mod structure;

#[derive(Default)]
pub struct TypeRegister {
    items: IndexMap<String, TypeItem>,
}

pub enum TypeItem {
    Structure(StructureType),
    Array(ArrayType),
    // SubTyping { sub: SubType },
    // Recursion(RecursiveType),
}

impl<'i> IntoIterator for &'i TypeRegister {
    type Item = (usize, &'i str, &'i TypeItem);
    type IntoIter = IndexedIterator<'i, TypeItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl TypeRegister {
    pub fn insert(&mut self, item: TypeItem) -> Option<TypeItem> {
        self.items.insert(item.name().to_string(), item)
    }
}

impl TypeItem {
    pub fn name(&self) -> String {
        match self {
            TypeItem::Structure(v) => v.name(),
            TypeItem::Array(v) => v.symbol.to_string(),
        }
    }
}

impl<'a, 'i> WasmOutput<'a, ValType<'i>> for NyarValue {
    fn as_wast(&'a self) -> ValType<'i> {
        self.as_type().as_wast()
    }
}

impl<'a, 'i> WasmOutput<'a, ValType<'i>> for NyarType {
    fn as_wast(&'a self) -> ValType<'i> {
        match self {
            NyarType::U32 => ValType::I32,
            NyarType::I8 => ValType::I32,
            NyarType::I16 => ValType::I32,
            NyarType::I32 => ValType::I32,
            NyarType::I64 => ValType::I64,
            NyarType::F32 => ValType::F32,
            NyarType::F64 => ValType::F64,
            NyarType::Any => ValType::Ref(RefType { nullable: true, heap: HeapType::Func }),
            NyarType::Structure => ValType::Ref(RefType { nullable: true, heap: HeapType::Struct }),
            NyarType::Array => ValType::Ref(RefType { nullable: true, heap: HeapType::Array }),
        }
    }
}
impl<'a, 'i> WasmOutput<'a, StorageType<'i>> for NyarValue {
    fn as_wast(&'a self) -> StorageType<'i> {
        self.as_type().as_wast()
    }
}
impl<'a, 'i> WasmOutput<'a, StorageType<'i>> for NyarType {
    fn as_wast(&'a self) -> StorageType<'i> {
        match self {
            NyarType::I8 => StorageType::I8,
            NyarType::I16 => StorageType::I16,
            _ => StorageType::Val(self.as_wast()),
        }
    }
}

impl<'a, 'i> WasmOutput<'a, ModuleField<'i>> for TypeItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ModuleField<'i> {
        match self {
            Self::Structure(v) => ModuleField::Type(v.as_wast()),
            Self::Array(v) => ModuleField::Type(v.as_wast()),
        }
    }
}
