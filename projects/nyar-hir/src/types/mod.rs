use crate::Symbol;

#[derive(Debug)]
pub enum NyarType {
    U8,
    U32,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Any,
    Named { symbol: Symbol },
    Array { inner: Box<NyarType> },
}
