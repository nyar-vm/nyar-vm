use crate::{
    helpers::{Id, WasmOutput},
    modules::DataItem,
    types::NyarType,
    Symbol, WasmVariable,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use wast::{
    core::{Data, DataKind, DataVal, Expression, Global, GlobalKind, Instruction},
    token::{Float32, Float64, Index, NameAnnotation, Span},
};
pub mod data;
pub mod global;
pub mod variable;

impl<'a, 'i> WasmOutput<'a, Instruction<'i>> for NyarValue
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Instruction<'i> {
        match self {
            NyarValue::U32(v) => Instruction::I32Const(*v as i32),
            NyarValue::I32(v) => Instruction::I32Const(*v),
            NyarValue::I64(v) => Instruction::I64Const(*v),
            NyarValue::F32(v) => Instruction::F32Const(Float32 { bits: u32::from_le_bytes(v.to_le_bytes()) }),
            NyarValue::F64(v) => Instruction::F64Const(Float64 { bits: u64::from_le_bytes(v.to_le_bytes()) }),
            NyarValue::Function(_) => {
                todo!()
            }
            NyarValue::Structure(name) => Instruction::StructNewDefault(Index::Id(Id::new(name.as_ref(), 0))),
            NyarValue::Array => {
                todo!()
            }
            NyarValue::Any => {
                todo!()
            }
        }
    }
}

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
            NyarValue::Structure(name) => NyarType::Named { symbol: name.clone(), nullable: false },
            NyarValue::Array => NyarType::Array { inner: Box::new(NyarType::I8), nullable: false },
            NyarValue::Any => NyarType::Any,
        }
    }
}
