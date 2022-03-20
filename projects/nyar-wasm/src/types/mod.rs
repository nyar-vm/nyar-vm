use crate::helpers::{WasmBuilder, WasmEmitter};
use indexmap::IndexMap;

use nyar_error::NyarError;
use nyar_hir::{
    ArrayType, FieldBuilder, FunctionExternalItem, FunctionItem, FunctionType, Identifier, IndexedIterator, NyarType,
    StructureType,
};
use wasm_encoder::{FieldType, StorageType, SubType, TypeSection, ValType};

#[derive(Default)]
pub struct TypeBuilder {
    items: IndexMap<String, TypeItem>,
}

impl<'i> IntoIterator for &'i TypeBuilder {
    type Item = (usize, &'i str, &'i TypeItem);
    type IntoIter = IndexedIterator<'i, TypeItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.items)
    }
}

impl TypeBuilder {
    pub fn insert(&mut self, item: TypeItem) -> Option<TypeItem> {
        self.items.insert(item.name().to_string(), item)
    }
}

pub enum TypeItem {
    Function(FunctionType),
    Structure(StructureType),
    Array(ArrayType),
    SubTyping { sub: SubType },
    Recursion(RecursiveType),
}

impl TypeItem {
    pub fn name(&self) -> &Identifier {
        match self {
            TypeItem::Function(v) => v.name(),
            TypeItem::Structure(_) => {
                todo!()
            }
            TypeItem::Array(_) => {
                todo!()
            }
            TypeItem::SubTyping { .. } => {
                todo!()
            }
            TypeItem::Recursion(_) => {
                todo!()
            }
        }
    }
}

pub trait WasmFunction {
    fn emit_function(&self, types: &mut TypeSection);
}

impl WasmFunction for FunctionItem {
    fn emit_function(&self, types: &mut TypeSection) {
        types.function(self.input.iter().map(|v| v.build(&()).unwrap()), self.output.iter().map(|v| v.build(&()).unwrap()));
    }
}
impl WasmFunction for FunctionExternalItem {
    fn emit_function(&self, types: &mut TypeSection) {
        types.function(self.input.iter().map(|v| v.build(&()).unwrap()), self.output.iter().map(|v| v.build(&()).unwrap()));
    }
}
impl WasmEmitter for FunctionType {
    type Receiver = TypeSection;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        let inputs = self.input.iter();
        let output = self.output.iter();
        let mut is = Vec::with_capacity(inputs.len());
        for field in inputs {
            is.push(field.build(store)?)
        }
        let mut os = Vec::with_capacity(output.len());
        for field in output {
            os.push(field.build(store)?)
        }
        reviver.function(is, os);
        Ok(())
    }
}

impl WasmEmitter for TypeItem {
    type Receiver = TypeSection;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        match self {
            Self::Function(v) => v.emit(reviver, store)?,
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
impl WasmBuilder<ValType> for NyarType {
    fn build(&self, _: &Self::Store) -> Result<ValType, NyarError> {
        Ok(match self {
            NyarType::I8 => ValType::I32,
            NyarType::I16 => ValType::I32,
            NyarType::I32 => ValType::I32,
            NyarType::I64 => ValType::I64,
            NyarType::F32 => ValType::F32,
            NyarType::F64 => ValType::F64,
        })
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
