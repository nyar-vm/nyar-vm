use crate::{
    functions::FunctionType,
    helpers::{Id, IndexedIterator, WasmInstruction, WasmOutput},
    ArrayType, StructureType, WasmSymbol,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use wast::{
    core::{
        HeapType, Import, ItemKind, ItemSig, ModuleField, RefType, StorageType, StructField, StructType, Type, TypeDef,
        TypeUse, ValType,
    },
    token::{Index, NameAnnotation, Span},
};

pub mod array;
pub mod external;
pub mod functional;
pub mod structure;

#[derive(Default)]
pub struct TypeSection {
    items: IndexMap<String, TypeItem>,
}

#[derive(Debug)]
pub enum WasmType {
    U8,
    U32,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Any,
    Named { symbol: WasmSymbol, nullable: bool },
    Array { inner: Box<WasmType>, nullable: bool },
}

impl WasmType {
    pub fn set_nullable(&mut self, nullable: bool) {
        match self {
            Self::Named { nullable: n, .. } => *n = nullable,
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

impl<'a, 'i> WasmOutput<'a, ValType<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ValType<'i> {
        match self {
            WasmType::U8 => ValType::I32,
            WasmType::U32 => ValType::I32,
            WasmType::I8 => ValType::I32,
            WasmType::I16 => ValType::I32,
            WasmType::I32 => ValType::I32,
            WasmType::I64 => ValType::I64,
            WasmType::F32 => ValType::F32,
            WasmType::F64 => ValType::F64,
            WasmType::Any => ValType::Ref(RefType { nullable: true, heap: HeapType::Func }),
            WasmType::Named { symbol, nullable } => {
                ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Concrete(Index::Id(Id::new(symbol.as_ref()))) })
            }
            // type erased
            WasmType::Array { nullable, .. } => ValType::Ref(RefType { nullable: *nullable, heap: HeapType::Array }),
        }
    }
}

impl<'a, 'i> WasmOutput<'a, StorageType<'i>> for WasmType
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
