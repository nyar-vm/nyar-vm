use crate::{types::NyarType, Symbol};

#[derive(Debug)]
pub enum NyarValue {
    U32(u32),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Function(Symbol),
    Structure,
    Array,
    Any,
}

impl NyarValue {
    pub fn as_type(&self) -> NyarType {
        match self {
            NyarValue::U32(_) => NyarType::I32,
            NyarValue::I32(_) => NyarType::I32,
            NyarValue::I64(_) => NyarType::I32,
            NyarValue::F32(_) => NyarType::F32,
            NyarValue::F64(_) => NyarType::F32,
            NyarValue::Function(_) => NyarType::I32,
            NyarValue::Structure => NyarType::Structure,
            NyarValue::Array => NyarType::Array,
            NyarValue::Any => NyarType::Any,
        }
    }
}
