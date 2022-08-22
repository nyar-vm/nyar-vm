use super::*;

impl WasmInstruction for Operation {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        match self {
            Self::Sequence { code: items } => {
                items.iter().for_each(|i| i.emit(w));
            }
            Self::Repeats { code, repeats } => {
                for _ in 0..*repeats {
                    code.iter().for_each(|i| i.emit(w));
                }
            }
            Self::GetVariable { kind, variable } => match kind {
                VariableKind::Global => w.push(Instruction::GlobalGet(Index::Id(WasmName::new(variable.as_ref())))),
                VariableKind::Local => w.push(Instruction::LocalGet(Index::Id(WasmName::new(variable.as_ref())))),
                VariableKind::Table => {
                    w.push(Instruction::TableGet(TableArg { dst: Index::Id(WasmName::new(variable.as_ref())) }))
                }
            },
            Self::SetVariable { kind, variable } => match kind {
                VariableKind::Global => w.push(Instruction::GlobalSet(Index::Id(WasmName::new(variable.as_ref())))),
                VariableKind::Local => w.push(Instruction::LocalSet(Index::Id(WasmName::new(variable.as_ref())))),
                VariableKind::Table => {
                    w.push(Instruction::TableSet(TableArg { dst: Index::Id(WasmName::new(variable.as_ref())) }))
                }
            },
            Self::TeeVariable { variable } => w.push(Instruction::LocalTee(Index::Id(WasmName::new(variable.as_ref())))),
            Self::CallFunction { name, input } => {
                input.iter().for_each(|i| i.emit(w));
                w.push(Instruction::Call(Index::Id(WasmName::new(name.as_ref()))));
            }
            Self::Default { typed } => typed.emit(w),
            Self::Constant { value } => value.emit(w),
            Self::NativeSum { native, terms } => match terms.as_slice() {
                [] => {}
                [head, rest @ ..] => {
                    head.emit(w);
                    for i in rest {
                        i.emit(w);
                        match native {
                            WasmType::I32 => w.push(Instruction::I32Add),
                            WasmType::I64 => w.push(Instruction::I64Add),
                            WasmType::F32 => w.push(Instruction::F32Add),
                            WasmType::F64 => w.push(Instruction::F64Add),
                            _ => todo!(),
                        }
                    }
                }
            },
            Self::NativeEqual { native, codes } => match codes.as_slice() {
                [] => {}
                [head, rest @ ..] => {
                    head.emit(w);
                    for i in rest {
                        i.emit(w);
                        match native {
                            WasmType::Bool | WasmType::I32 => w.push(Instruction::I32Eq),
                            WasmType::I64 => w.push(Instruction::I64Eq),
                            WasmType::F32 => w.push(Instruction::F32Eq),
                            WasmType::F64 => w.push(Instruction::F64Eq),
                            WasmType::Any { nullable } => match nullable {
                                true => w.push(Instruction::RefEq),
                                false => w.push(Instruction::RefEq),
                            },
                            _ => todo!(),
                        }
                    }
                }
            },
            Self::NativeEqualZero { .. } => {
                todo!()
            }
            Self::JumpBranch(branch) => branch.emit(w),
            Self::JumpTable(table) => table.emit(w),
            Self::Loop { r#continue, r#break, body } => {
                w.push(Instruction::Loop(Box::new(BlockType {
                    label: WasmName::id(r#continue.as_ref()),
                    label_name: None,
                    ty: TypeUse { index: None, inline: None },
                })));
                w.push(Instruction::Block(Box::new(BlockType {
                    label: WasmName::id(r#break.as_ref()),
                    label_name: None,
                    ty: TypeUse { index: None, inline: None },
                })));
                body.iter().for_each(|i| i.emit(w));
                w.push(Instruction::End(None));
                w.push(Instruction::End(None));
            }
            Self::Goto { label } => w.push(Instruction::Br(Index::Id(WasmName::new(label.as_ref())))),
            Self::Drop => {
                w.push(Instruction::Drop);
            }
            Self::Return => w.push(Instruction::Return),
            Self::Unreachable => w.push(Instruction::Unreachable),
            Self::Convert { from, into, code } => {
                code.iter().for_each(|i| i.emit(w));
                match (from, into) {
                    // u32 -> ?
                    (WasmType::U32, WasmType::U32) => {}
                    (WasmType::U32, WasmType::I32) => w.push(Instruction::I32WrapI64),
                    (WasmType::U32, WasmType::I64) => w.push(Instruction::I64ExtendI32U),
                    (WasmType::U32, WasmType::F32) => w.push(Instruction::F32ConvertI32U),
                    (WasmType::U32, WasmType::F64) => w.push(Instruction::F64ConvertI32U),
                    // i32 -> ?
                    (WasmType::I32, WasmType::I32) => {}
                    (WasmType::I32, WasmType::I64) => w.push(Instruction::I64ExtendI32S),
                    (WasmType::I32, WasmType::F32) => w.push(Instruction::F32ConvertI32S),
                    (WasmType::I64, WasmType::F32) => w.push(Instruction::F32ConvertI64S),
                    // f32 -> ?
                    (WasmType::F32, WasmType::I32) => w.push(Instruction::I32TruncF32S),
                    (WasmType::F32, WasmType::I64) => w.push(Instruction::I64TruncF32S),
                    (WasmType::F32, WasmType::F32) => {}
                    (WasmType::F32, WasmType::F64) => w.push(Instruction::F64PromoteF32),
                    // f64 -> ?
                    (WasmType::F64, WasmType::I32) => w.push(Instruction::I32TruncF64S),
                    (WasmType::F64, WasmType::I64) => w.push(Instruction::I64TruncF64S),
                    (WasmType::F64, WasmType::F32) => w.push(Instruction::F32DemoteF64),
                    (WasmType::F64, WasmType::F64) => {}
                    _ => {
                        unimplemented!()
                    }
                }
            }
            Self::Transmute { from, into, code } => {
                code.iter().for_each(|i| i.emit(w));
                match (from, into) {
                    (WasmType::I32, WasmType::F32) => w.push(Instruction::F32ReinterpretI32),
                    (WasmType::I64, WasmType::F64) => w.push(Instruction::F64ReinterpretI64),
                    (WasmType::F32, WasmType::I32) => w.push(Instruction::I32ReinterpretF32),
                    (WasmType::F64, WasmType::I64) => w.push(Instruction::I64ReinterpretF64),

                    _ => {
                        unimplemented!()
                    }
                }
            }
            Self::JumpEnumeration(_) => {}
        }
    }
}

fn block_return(returns: &[WasmType]) -> Option<wast::core::FunctionType> {
    if returns.is_empty() {
        None
    }
    else {
        let result: Vec<_> = returns.iter().map(|i| i.as_wast()).collect();
        Some(wast::core::FunctionType { params: Box::default(), results: Box::from(result) })
    }
}

impl WasmInstruction for JumpBranch {
    /// `if { then } else { else } end`
    /// `{ then } { else } condition select`
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        let inline = block_return(&self.r#return);
        self.main.condition.iter().for_each(|i| i.emit(w));
        w.push(Instruction::If(Box::new(BlockType { label: None, label_name: None, ty: TypeUse { index: None, inline } })));
        self.main.action.iter().for_each(|i| i.emit(w));
        w.push(Instruction::Else(None));
        self.default.iter().for_each(|i| i.emit(w));
        w.push(Instruction::End(None))
    }
}

impl WasmInstruction for JumpTable {
    /// ```v
    /// if a {
    /// }
    /// else if b {
    /// }
    /// else {
    /// }
    ///
    /// if a {
    /// }
    /// else {
    ///     if b {
    ///     }
    ///     else {
    ///     }
    /// }
    ///
    /// a
    /// if
    ///   a_body
    /// else  
    ///   b
    ///   if
    ///   b_body
    ///   else
    ///   c_body
    ///   end
    /// end
    /// ```
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        let inline = block_return(&self.r#return);
        for branch in &self.branches {
            branch.condition.iter().for_each(|i| i.emit(w));
            w.push(Instruction::If(Box::new(BlockType {
                label: None,
                label_name: None,
                ty: TypeUse { index: None, inline: inline.clone() },
            })));
            branch.action.iter().for_each(|i| i.emit(w));
            w.push(Instruction::Else(None));
        }
        self.default.iter().for_each(|i| i.emit(w));
        for _ in &self.branches {
            w.push(Instruction::End(None));
        }
    }
}
impl WasmInstruction for WasmType {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        match self {
            // false
            Self::Bool => w.push(Instruction::I32Const(0)),
            Self::U8 => w.push(Instruction::I32Const(0)),
            Self::U16 => w.push(Instruction::I32Const(0)),
            Self::U32 => w.push(Instruction::I32Const(0)),
            Self::U64 => w.push(Instruction::I64Const(0)),
            Self::I8 => w.push(Instruction::I32Const(0)),
            Self::I16 => w.push(Instruction::I32Const(0)),
            Self::I32 => w.push(Instruction::I32Const(0)),
            Self::I64 => w.push(Instruction::I64Const(0)),
            Self::F32 => w.push(Instruction::F32Const(Float32 { bits: 0 })),
            Self::F64 => w.push(Instruction::F64Const(Float64 { bits: 0 })),
            Self::Any { .. } => {
                todo!()
            }
            Self::Structure(_) => todo!(),
            Self::Array { .. } => {
                todo!()
            }
            Self::Unicode => {
                todo!()
            }
            Self::UTF8Text => {
                todo!()
            }
        }
    }
}

impl WasmInstruction for WasmValue {
    fn emit<'a, 'i>(&'a self, w: &mut Vec<Instruction<'i>>)
    where
        'a: 'i,
    {
        match self {
            Self::Bool(v) => match v {
                true => w.push(Instruction::I32Const(1)),
                false => w.push(Instruction::I32Const(0)),
            },
            Self::U32(v) => {
                w.push(Instruction::I32Const(*v as i32));
            }
            Self::I32(v) => {
                w.push(Instruction::I32Const(*v));
            }
            Self::I64(v) => {
                w.push(Instruction::I64Const(*v));
            }
            Self::F32(v) => {
                w.push(Instruction::F32Const(Float32 { bits: u32::from_le_bytes(v.to_le_bytes()) }));
            }
            Self::F64(v) => {
                w.push(Instruction::F64Const(Float64 { bits: u64::from_le_bytes(v.to_le_bytes()) }));
            }
            Self::Function(_) => {
                todo!()
            }
            Self::Structure(_) => {
                todo!()
            }
            Self::Array => {
                todo!()
            }
            Self::Any => {
                todo!()
            }
        }
    }
}
