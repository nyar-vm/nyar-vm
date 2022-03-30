use crate::{types::NyarType, Symbol};

#[derive(Debug)]
pub enum NyarValue {
    U32(u32),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Function(Symbol),
    Structure(Symbol),
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
            NyarValue::Structure(name) => NyarType::Named { symbol: name.clone() },
            NyarValue::Array => NyarType::Array { inner: Box::new(NyarType::I8) },
            NyarValue::Any => NyarType::Any,
        }
    }
}
