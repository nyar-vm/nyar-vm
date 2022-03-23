use crate::helpers::{WasmBuilder, WasmDefineType, WasmEmitter};

use nyar_error::NyarError;
use nyar_hir::{ArrayType, ExternalType, FunctionItem, NyarType, StructureType, TypeItem};
use wasm_encoder::{FieldType, HeapType, RefType, StorageType, SubType, TypeSection, ValType};

pub trait WasmFunction {
    fn emit_function(&self, types: &mut TypeSection);
}

impl WasmFunction for FunctionItem {
    fn emit_function(&self, types: &mut TypeSection) {
        types.function(self.input.iter().map(|v| v.build(&()).unwrap()), self.output.iter().map(|v| v.build(&()).unwrap()));
    }
}

impl WasmFunction for ExternalType {
    fn emit_function(&self, types: &mut TypeSection) {
        types.function(self.input.iter().map(|v| v.build(&()).unwrap()), self.output.iter().map(|v| v.build(&()).unwrap()));
    }
}

impl WasmEmitter for TypeItem {
    type Receiver = TypeSection;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        match self {
            Self::Structure(v) => v.emit(reviver, store)?,
            Self::Array(v) => v.emit(reviver, store)?,
            // Self::SubTyping { sub } => {
            //     reviver.subtype(sub);
            // }
            // Self::Recursion(v) => {
            //     reviver.rec(v.rec.iter().cloned());
            // }
        };
        Ok(())
    }
}

impl WasmEmitter for StructureType {
    type Receiver = TypeSection;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        let fields = self.fields();
        let mut fields = Vec::with_capacity(fields.len());
        for (_, _, field) in self.fields() {
            fields.push(field.build(store)?)
        }
        reviver.struct_(fields);
        Ok(())
    }
}

impl WasmBuilder<FieldType> for nyar_hir::FieldType {
    fn build(&self, store: &Self::Store) -> Result<FieldType, NyarError> {
        let element_type = self.default.as_type().build(store)?;
        Ok(FieldType { element_type, mutable: self.mutable })
    }
}

impl WasmBuilder<StorageType> for NyarType {
    fn build(&self, store: &Self::Store) -> Result<StorageType, NyarError> {
        Ok(match self {
            Self::I8 => StorageType::I8,
            Self::I16 => StorageType::I16,
            _ => StorageType::Val(self.build(store)?),
        })
    }
}
impl WasmBuilder<ValType> for NyarType {
    fn build(&self, _: &Self::Store) -> Result<ValType, NyarError> {
        Ok(match self {
            Self::I8 => ValType::I32,
            Self::I16 => ValType::I32,
            Self::I32 => ValType::I32,
            Self::I64 => ValType::I64,
            Self::F32 => ValType::F32,
            Self::F64 => ValType::F64,
            Self::Structure => ValType::Ref(RefType { nullable: true, heap_type: HeapType::Struct }),
            Self::Array => ValType::Ref(RefType { nullable: true, heap_type: HeapType::Array }),
            Self::Any => ValType::Ref(RefType { nullable: true, heap_type: HeapType::Any }),
            Self::Any => ValType::Ref(RefType { nullable: true, heap_type: HeapType::I31 }),
        })
    }
}

impl WasmBuilder<StorageType> for ArrayType {
    fn build(&self, store: &Self::Store) -> Result<StorageType, NyarError> {
        self.item_type.build(store)
    }
}

impl WasmEmitter for ArrayType {
    type Receiver = TypeSection;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        reviver.array(&self.build(store)?, true);
        Ok(())
    }
}

pub struct RecursiveType {
    rec: Vec<SubType>,
}
