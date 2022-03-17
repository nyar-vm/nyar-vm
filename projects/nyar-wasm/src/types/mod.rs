use crate::helpers::WasmBuilder;
use indexmap::IndexMap;
use nyar_error::NyarError;
use nyar_hir::{ArrayType, FieldBuilder, Identifier, NyarType, NyarValue, StructureType};
use wasm_encoder::{FieldType, StorageType, SubType, TypeSection, ValType};

impl WasmBuilder<Vec<FieldType>> for StructureType {
    fn build(&self, store: &Self::Store) -> Result<Vec<FieldType>, NyarError> {
        let fields = self.fields();
        let mut fields = Vec::with_capacity(fields.len());
        for (_, _, field) in self.fields() {
            fields.push(field.build(store)?)
        }
        Ok(fields)
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

pub struct RecursiveType {
    rec: Vec<SubType>,
}

pub enum TypeItem {
    Function { input: Vec<ValType>, output: Vec<ValType> },
    Structure(StructureType),
    Array(ArrayType),
    SubTyping { sub: SubType },
    Recursion(RecursiveType),
}

impl TypeItem {
    pub fn build(&self, types: &mut TypeSection) -> Result<(), NyarError> {
        match self {
            TypeItem::Function { input, output } => types.function(input.iter().cloned(), output.iter().cloned()),
            TypeItem::Structure(v) => types.struct_(v.build(&())?),
            TypeItem::Array(v) => types.array(&v.build(&())?, true),
            TypeItem::SubTyping { sub } => types.subtype(sub),
            TypeItem::Recursion(v) => types.rec(v.rec.iter().cloned()),
        };
        Ok(())
    }
}
