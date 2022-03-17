use crate::helpers::{WasmBuilder, WasmEmitter};

use nyar_error::NyarError;
use nyar_hir::{ArrayType, FieldBuilder, NyarType, StructureType};
use wasm_encoder::{FieldType, StorageType, SubType, TypeSection, ValType};

pub enum TypeItem {
    Function { input: Vec<ValType>, output: Vec<ValType> },
    Structure(StructureType),
    Array(ArrayType),
    SubTyping { sub: SubType },
    Recursion(RecursiveType),
}

impl WasmEmitter for TypeItem {
    type Receiver = TypeSection;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        match self {
            Self::Function { input, output } => {
                reviver.function(input.iter().cloned(), output.iter().cloned());
            }
            Self::Structure(v) => v.emit(reviver, store)?,
            Self::Array(v) => v.emit(reviver, store)?,
            Self::SubTyping { sub } => {
                reviver.subtype(sub);
            }
            Self::Recursion(v) => {
                reviver.rec(v.rec.iter().cloned());
            }
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

impl WasmBuilder<FieldType> for FieldBuilder {
    fn build(&self, store: &Self::Store) -> Result<FieldType, NyarError> {
        let element_type = self.r#type().build(store)?;
        Ok(FieldType { element_type, mutable: true })
    }
}

impl WasmBuilder<StorageType> for NyarType {
    fn build(&self, _: &Self::Store) -> Result<StorageType, NyarError> {
        Ok(match self {
            NyarType::I8 => StorageType::I8,
            NyarType::I16 => StorageType::I16,
            NyarType::I32 => StorageType::Val(ValType::I32),
            NyarType::I64 => StorageType::Val(ValType::I64),
            NyarType::F32 => StorageType::Val(ValType::F32),
            NyarType::F64 => StorageType::Val(ValType::F64),
        })
    }
}

impl WasmBuilder<StorageType> for ArrayType {
    fn build(&self, store: &Self::Store) -> Result<StorageType, NyarError> {
        self.item_type().build(store)
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
