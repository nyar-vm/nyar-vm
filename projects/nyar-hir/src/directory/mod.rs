use crate::Identifier;

pub enum NyarType {
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

pub struct ArrayType {
    /// Item type of the array
    item: NyarType,
}

pub struct FunctionType {
    pub name: Identifier,
    pub input: Vec<NyarType>,
    pub output: Vec<NyarType>,
}

impl FunctionType {
    pub fn name(&self) -> &Identifier {
        &self.name
    }
}

impl ArrayType {
    pub fn item_type(&self) -> &NyarType {
        &self.item
    }
}
