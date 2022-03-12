use crate::Identifier;

pub enum NyarType {
    I32,
    F32,
}

pub enum NyarValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Function(Identifier),
}
