use crate::{
    helpers::{Id, WasmOutput},
    modules::DataItem,
};
use nyar_hir::NyarValue;
use wast::{
    core::{Data, DataKind, DataVal, Expression, Global, GlobalKind, Instruction},
    token::{Float32, Float64, Index, NameAnnotation, Span},
};

mod data;

impl<'a, 'i> WasmOutput<'a, Global<'i>> for nyar_hir::NamedValue
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Global<'i> {
        Global {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            exports: Default::default(),
            ty: wast::core::GlobalType { ty: self.value.as_wast(), mutable: self.mutable() },
            kind: GlobalKind::Inline(self.as_wast()),
        }
    }
}

impl<'a, 'i> WasmOutput<'a, Expression<'i>> for nyar_hir::NamedValue
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Expression<'i> {
        Expression { instrs: Box::from(vec![self.value.as_wast()]) }
    }
}

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
            NyarValue::Structure => {
                todo!()
            }
            NyarValue::Array => {
                todo!()
            }
            NyarValue::Any => {
                todo!()
            }
        }
    }
}
