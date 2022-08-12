use crate::{
    helpers::{Id, WasmOutput},
    modules::DataItem,
    types::WasmType,
    WasmSymbol, WasmVariable,
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

impl<'a, 'i> WasmOutput<'a, Instruction<'i>> for WasmValue
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Instruction<'i> {
        match self {
            WasmValue::U32(v) => Instruction::I32Const(*v as i32),
            WasmValue::I32(v) => Instruction::I32Const(*v),
            WasmValue::I64(v) => Instruction::I64Const(*v),
            WasmValue::F32(v) => Instruction::F32Const(Float32 { bits: u32::from_le_bytes(v.to_le_bytes()) }),
            WasmValue::F64(v) => Instruction::F64Const(Float64 { bits: u64::from_le_bytes(v.to_le_bytes()) }),
            WasmValue::Function(_) => {
                todo!()
            }
            WasmValue::Structure(name) => Instruction::StructNewDefault(Index::Id(Id::new(name.as_ref()))),
            WasmValue::Array => {
                todo!()
            }
            WasmValue::Any => {
                todo!()
            }
        }
    }
}

#[derive(Debug)]
pub enum WasmValue {
    Boolean(bool),
    U32(u32),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Function(WasmSymbol),
    Structure(WasmSymbol),
    Array,
    Any,
}

impl WasmValue {
    pub fn as_type(&self) -> WasmType {
        match self {
            WasmValue::U32(_) => WasmType::I32,
            WasmValue::I32(_) => WasmType::I32,
            WasmValue::I64(_) => WasmType::I32,
            WasmValue::F32(_) => WasmType::F32,
            WasmValue::F64(_) => WasmType::F32,
            WasmValue::Function(_) => WasmType::I32,
            WasmValue::Structure(name) => WasmType::Structure { symbol: name.clone(), nullable: false },
            WasmValue::Array => WasmType::Array { inner: Box::new(WasmType::I8), nullable: false },
            WasmValue::Any => WasmType::Any,
        }
    }
}
