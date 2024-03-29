use crate::{
    helpers::{IntoWasm, WasmName},
    types::WasmType,
    ArrayType, StructureItem, WasmSymbol, WasmVariable,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use wast::{
    core::{Expression, Global, GlobalKind, Instruction},
    token::{Float32, Float64, Span},
};
pub mod data;
pub mod global;
pub mod variable;
use crate::{data::DataItem, helpers::IndexedIterator};
mod convert;
use wast::core::GlobalType;

impl<'a, 'i> IntoWasm<'a, Instruction<'i>> for WasmValue
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Instruction<'i> {
        match self {
            Self::Bool(v) => match v {
                true => Instruction::I32Const(1),
                false => Instruction::I32Const(0),
            },
            Self::U32(v) => Instruction::I32Const(*v as i32),
            Self::I32(v) => Instruction::I32Const(*v),
            Self::I64(v) => Instruction::I64Const(*v),
            Self::F32(v) => Instruction::F32Const(Float32 { bits: u32::from_le_bytes(v.to_le_bytes()) }),
            Self::F64(v) => Instruction::F64Const(Float64 { bits: u64::from_le_bytes(v.to_le_bytes()) }),
            Self::Function(_) => {
                todo!()
            }
            Self::Structure(o) => Instruction::StructNewDefault(o.symbol.as_index()),
            Self::Array(o) => Instruction::ArrayNewDefault(o.symbol.as_index()),
            Self::Any => {
                todo!()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum WasmValue {
    Bool(bool),
    U32(u32),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Function(WasmSymbol),
    Structure(StructureItem),
    Array(Box<ArrayType>),
    Any,
}
