use crate::{
    functions::FunctionType,
    helpers::{IndexedIterator, IntoWasm, WasmName},
    ArrayType, ExternalType, StructureType, WasmSymbol,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use wast::{
    core::{HeapType, ModuleField, RefType, StorageType, StructField, StructType, Type, TypeDef, ValType},
    token::{Index, NameAnnotation, Span},
};

pub mod array;
pub mod external;
pub mod functional;
pub mod structure;

#[derive(Default)]
pub struct TypeSection {
    items: IndexMap<String, WasmType>,
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
    Array { inner: Box<WasmType>, nullable: bool },
}

impl WasmType {
    pub fn set_nullable(&mut self, nullable: bool) {
        match self {
            Self::Structure(_) => {
                todo!()
            }
            Self::Array { nullable: n, .. } => *n = nullable,
            _ => {}
        }
    }
}

pub enum TypeItem {
    Structure(StructureType),
    Array(ArrayType),
    // SubTyping { sub: SubType },
    // Recursion(RecursiveType),
}

impl<'i> IntoIterator for &'i TypeSection {
    type Item = (usize, &'i str, &'i TypeItem);
    type IntoIter = IndexedIterator<'i, TypeItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl TypeSection {
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
            Self::Any { nullable } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Any }),
            Self::Structure { symbol, nullable } => ValType::Ref(RefType {
                nullable: *nullable,
                heap: HeapType::Concrete(Index::Id(WasmName::new(symbol.as_ref()))),
            }),
            // type erased
            Self::Array { nullable, .. } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Array }),
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

impl<'a, 'i> IntoWasm<'a, ModuleField<'i>> for TypeItem
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
