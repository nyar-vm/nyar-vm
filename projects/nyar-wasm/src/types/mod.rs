use crate::helpers::{Id, WasmBuilder, WasmDefineType, WasmEmitter, WasmOutput};
use nyar_hir::{ExternalType, FieldType, NyarType, NyarValue, StructureType, TypeItem};
use wast::{
    core::{
        ArrayType, FunctionType, HeapType, Import, InlineImport, ItemKind, ItemSig, ModuleField, RefType, StorageType,
        StructField, StructType, Type, TypeDef, TypeUse, ValType,
    },
    token::{NameAnnotation, Span},
};

mod array;
mod external;
mod functional;
mod structure;

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
