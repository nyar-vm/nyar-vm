use crate::helpers::WasmBuilder;
use indexmap::IndexMap;
use nyar_error::NyarError;
use nyar_hir::{FieldBuilder, Identifier, NyarType, NyarValue, StructureBuilder};
use wasm_encoder::{FieldType, StorageType, SubType, TypeSection, ValType};

impl WasmBuilder<Vec<FieldType>> for StructureBuilder {
    fn build(&self, store: &Self::Store) -> Result<Vec<FieldType>, NyarError> {
        let fields = self.fields();
        let mut fields = Vec::with_capacity(fields.len());
        for (index, name, field) in self.fields() {
            fields.push(field.build(store)?)
        }
        Ok(fields)
    }
}

impl WasmBuilder<FieldType> for FieldBuilder {
    fn build(&self, _: &Self::Store) -> Result<FieldType, NyarError> {
        Ok(FieldType { element_type: StorageType::I8, mutable: true })
    }
}

pub enum TypeItem {
    Function { input: Vec<ValType>, output: Vec<ValType> },
    Structure(StructureBuilder),
    Array { raw: StorageType },
    SubTyping { sub: SubType },
    Rec { rec: Vec<SubType> },
}

impl TypeItem {
    pub fn build(&self, types: &mut TypeSection) -> Result<(), NyarError> {
        match self {
            TypeItem::Function { input, output } => types.function(input.iter().cloned(), output.iter().cloned()),
            TypeItem::Structure(v) => types.struct_(v.build(&())?),
            TypeItem::Array { raw } => types.array(raw, true),
            TypeItem::SubTyping { sub } => types.subtype(sub),
            TypeItem::Rec { rec } => types.rec(rec.iter().cloned()),
        }
        Ok(())
    }
}
