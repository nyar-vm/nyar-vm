use indexmap::IndexMap;
use wasm_encoder::{FieldType, StorageType, SubType, TypeSection, ValType};

pub enum TypeBuilder {
    Function {
        input: Vec<ValType>,
        output: Vec<ValType>,
    },
    Structure {
        fields: IndexMap<String, FieldType>,
    },
    Array {
        raw: StorageType,
    },
    SubTyping {
        sub: SubType
    },
    Rec {
        rec: Vec<SubType>
    },
}

impl TypeBuilder {
    pub fn build(&self, types: &mut TypeSection) {
        match self {
            TypeBuilder::Function { input, output } => {
                types.function(input.iter().cloned(), output.iter().cloned())
            }
            TypeBuilder::Structure { fields } => {
                types.struct_(fields.values().cloned())
            }
            TypeBuilder::Array { raw } => {
                types.array(raw, true)
            }
            TypeBuilder::SubTyping { sub } => {
                types.subtype(sub)
            }
            TypeBuilder::Rec { rec } => {
                types.rec(rec.iter().cloned())
            }
        };
    }
}
