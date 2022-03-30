use crate::helpers::{Id, WasmInstruction};
use nyar_hir::{NyarType, NyarValue, Operation, VariableKind};
use wast::{
    core::{BlockType, Instruction, TableArg, TypeUse},
    token::{Float32, Float64, Index},
};
impl WasmInstruction for Operation {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        match self {
            Self::Sequence { items } => {
                items.iter().for_each(|i| i.emit(w));
            }
            Self::GetVariable { kind, variable } => match kind {
                VariableKind::Global => w.push(Instruction::GlobalGet(Index::Id(Id::new(variable.as_ref(), 0)))),
                VariableKind::Local => w.push(Instruction::LocalGet(Index::Id(Id::new(variable.as_ref(), 0)))),
                VariableKind::Table => {
                    w.push(Instruction::TableGet(TableArg { dst: Index::Id(Id::new(variable.as_ref(), 0)) }))
                }
            },
            Self::SetVariable { kind, variable } => match kind {
                VariableKind::Global => w.push(Instruction::GlobalSet(Index::Id(Id::new(variable.as_ref(), 0)))),
                VariableKind::Local => w.push(Instruction::LocalSet(Index::Id(Id::new(variable.as_ref(), 0)))),
                VariableKind::Table => {
                    w.push(Instruction::TableSet(TableArg { dst: Index::Id(Id::new(variable.as_ref(), 0)) }))
                }
            },
            Self::TeeVariable { variable } => w.push(Instruction::LocalTee(Index::Id(Id::new(variable.as_ref(), 0)))),
            Self::CallFunction { name, input } => {
                input.iter().for_each(|i| i.emit(w));
                w.push(Instruction::Call(Index::Id(Id::new(name.as_ref(), 0))));
            }
            Self::Constant { value } => value.emit(w),
            Self::NativeSum { native, terms } => match terms.as_slice() {
                [] => match native {
                    NyarType::I32 => w.push(Instruction::I32Const(0)),
                    NyarType::I64 => w.push(Instruction::I64Const(0)),
                    NyarType::F32 => w.push(Instruction::F32Const(Float32 { bits: 0 })),
                    NyarType::F64 => w.push(Instruction::F64Const(Float64 { bits: 0 })),
                    _ => todo!(),
                },
                [head, rest @ ..] => {
                    head.emit(w);
                    for i in rest {
                        i.emit(w);
                        match native {
                            NyarType::I32 => w.push(Instruction::I32Add),
                            NyarType::I64 => w.push(Instruction::I64Add),
                            NyarType::F32 => w.push(Instruction::F32Add),
                            NyarType::F64 => w.push(Instruction::F64Add),
                            _ => todo!(),
                        }
                    }
                }
            },
            Self::NativeEqual { .. } => {
                todo!()
            }
            Self::NativeEqualZero { .. } => {
                todo!()
            }
            Self::Conditional { condition, then, r#else } => {
                // condition.emit(w);
                todo!()
            }
            Self::Loop { r#continue, r#break, body } => {
                w.push(Instruction::Loop(Box::new(BlockType {
                    label: Id::type_id(r#continue.as_ref()),
                    label_name: None,
                    ty: TypeUse { index: None, inline: None },
                })));
                w.push(Instruction::Block(Box::new(BlockType {
                    label: Id::type_id(r#break.as_ref()),
                    label_name: None,
                    ty: TypeUse { index: None, inline: None },
                })));
                body.iter().for_each(|i| i.emit(w));
                w.push(Instruction::End(None));
                w.push(Instruction::End(None));
            }
            Self::Goto { label } => w.push(Instruction::Br(Index::Id(Id::new(label.as_ref(), 0)))),
            Self::Drop => w.push(Instruction::Drop),
            Self::Return => w.push(Instruction::Return),
            Self::Unreachable => w.push(Instruction::Unreachable),
            Self::Convert { from, into, code } => {
                code.iter().for_each(|i| i.emit(w));
                match (from, into) {
                    // u32 -> ?
                    (NyarType::U32, NyarType::U32) => {}
                    (NyarType::U32, NyarType::I32) => w.push(Instruction::I32WrapI64),
                    (NyarType::U32, NyarType::I64) => w.push(Instruction::I64ExtendI32U),
                    (NyarType::U32, NyarType::F32) => w.push(Instruction::F32ConvertI32U),
                    (NyarType::U32, NyarType::F64) => w.push(Instruction::F64ConvertI32U),
                    // i32 -> ?
                    (NyarType::I32, NyarType::I32) => {}
                    (NyarType::I32, NyarType::I64) => w.push(Instruction::I64ExtendI32S),
                    (NyarType::I32, NyarType::F32) => w.push(Instruction::F32ConvertI32S),
                    (NyarType::I64, NyarType::F32) => w.push(Instruction::F32ConvertI64S),
                    // f32 -> ?
                    (NyarType::F32, NyarType::I32) => w.push(Instruction::I32TruncF32S),
                    (NyarType::F32, NyarType::I64) => w.push(Instruction::I64TruncF32S),
                    (NyarType::F32, NyarType::F32) => {}
                    (NyarType::F32, NyarType::F64) => w.push(Instruction::F64PromoteF32),
                    // f64 -> ?
                    (NyarType::F64, NyarType::I32) => w.push(Instruction::I32TruncF64S),
                    (NyarType::F64, NyarType::I64) => w.push(Instruction::I64TruncF64S),
                    (NyarType::F64, NyarType::F32) => w.push(Instruction::F32DemoteF64),
                    (NyarType::F64, NyarType::F64) => {}
                    _ => {
                        unimplemented!()
                    }
                }
            }
            Self::Transmute { from, into, code } => {
                code.iter().for_each(|i| i.emit(w));
                match (from, into) {
                    (NyarType::I32, NyarType::F32) => w.push(Instruction::F32ReinterpretI32),
                    (NyarType::I64, NyarType::F64) => w.push(Instruction::F64ReinterpretI64),
                    (NyarType::F32, NyarType::I32) => w.push(Instruction::I32ReinterpretF32),
                    (NyarType::F64, NyarType::I64) => w.push(Instruction::I64ReinterpretF64),

                    _ => {
                        unimplemented!()
                    }
                }
            }
        }
    }
}

impl WasmInstruction for NyarValue {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        match self {
            NyarValue::U32(v) => {
                w.push(Instruction::I32Const(*v as i32));
            }
            NyarValue::I32(v) => {
                w.push(Instruction::I32Const(*v));
            }
            NyarValue::I64(v) => {
                w.push(Instruction::I64Const(*v));
            }
            NyarValue::F32(v) => {
                w.push(Instruction::F32Const(Float32 { bits: u32::from_le_bytes(v.to_le_bytes()) }));
            }
            NyarValue::F64(v) => {
                w.push(Instruction::F64Const(Float64 { bits: u64::from_le_bytes(v.to_le_bytes()) }));
            }
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
