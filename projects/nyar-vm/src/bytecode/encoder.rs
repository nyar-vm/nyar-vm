use crate::bytecode::instruction::Instruction;
use crate::bytecode::opcode::*;

impl Instruction {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            Instruction::Nop => buf.push(Opcode::Nop as u8),
            Instruction::Push(v) => {
                buf.push(Opcode::Push as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::Pop => buf.push(Opcode::Pop as u8),
            Instruction::Dup(v) => {
                buf.push(Opcode::Dup as u8);
                buf.push(*v);
            }
            Instruction::Swap(v) => {
                buf.push(Opcode::Swap as u8);
                buf.push(*v);
            }
            Instruction::LoadLocal(v) => {
                buf.push(Opcode::LoadLocal as u8);
                buf.push(*v);
            }
            Instruction::StoreLocal(v) => {
                buf.push(Opcode::StoreLocal as u8);
                buf.push(*v);
            }
            Instruction::LoadGlobal(v) => {
                buf.push(Opcode::LoadGlobal as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::StoreGlobal(v) => {
                buf.push(Opcode::StoreGlobal as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::LoadUpvalue(v) => {
                buf.push(Opcode::LoadUpvalue as u8);
                buf.push(*v);
            }
            Instruction::StoreUpvalue(v) => {
                buf.push(Opcode::StoreUpvalue as u8);
                buf.push(*v);
            }
            Instruction::CloseUpvalues => buf.push(Opcode::CloseUpvalues as u8),
            Instruction::Jump(v) => {
                buf.push(Opcode::Jump as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::JumpIfFalse(v) => {
                buf.push(Opcode::JumpIfFalse as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::JumpIfNull(v) => {
                buf.push(Opcode::JumpIfNull as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::Return => buf.push(Opcode::Return as u8),
            Instruction::MakeClosure(idx, upvalues) => {
                buf.push(Opcode::MakeClosure as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(upvalues.len() as u8);
                for uv in upvalues {
                    buf.push(if uv.is_local { 1 } else { 0 });
                    buf.push(uv.index);
                }
            }
            Instruction::TailCall(args) => {
                buf.push(Opcode::TailCall as u8);
                buf.push(*args);
            }
            Instruction::Call(idx, args) => {
                buf.push(Opcode::Call as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::CallVirtual(idx, args) => {
                buf.push(Opcode::CallVirtual as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::CallDynamic(idx, args) => {
                buf.push(Opcode::CallDynamic as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::CallClosure(args) => {
                buf.push(Opcode::CallClosure as u8);
                buf.push(*args);
            }
            Instruction::InvokeMethod(idx, args) => {
                buf.push(Opcode::InvokeMethod as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::CallSymbol(idx, args) => {
                buf.push(Opcode::CallSymbol as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::GetField(idx) => {
                buf.push(Opcode::GetField as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::SetField(idx) => {
                buf.push(Opcode::SetField as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::NewObject(idx) => {
                buf.push(Opcode::NewObject as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::NewArray(idx) => {
                buf.push(Opcode::NewArray as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::GetElement => buf.push(Opcode::GetElement as u8),
            Instruction::SetElement => buf.push(Opcode::SetElement as u8),
            Instruction::MakeTuple(args) => {
                buf.push(Opcode::MakeTuple as u8);
                buf.push(*args);
            }
            Instruction::HasKey => buf.push(Opcode::HasKey as u8),
            Instruction::MatchVariant(idx) => {
                buf.push(Opcode::MatchVariant as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::SizeOf => buf.push(Opcode::SizeOf as u8),
            Instruction::NewDynObject => buf.push(Opcode::NewDynObject as u8),
            Instruction::RemoveKey => buf.push(Opcode::RemoveKey as u8),
            Instruction::NewList(idx) => {
                buf.push(Opcode::NewList as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::PushElementLeft => buf.push(Opcode::PushElementLeft as u8),
            Instruction::PopElementLeft => buf.push(Opcode::PopElementLeft as u8),
            Instruction::PushElementRight => buf.push(Opcode::PushElementRight as u8),
            Instruction::PopElementRight => buf.push(Opcode::PopElementRight as u8),
            Instruction::TypeOf => buf.push(Opcode::TypeOf as u8),
            Instruction::InstanceOf(idx) => {
                buf.push(Opcode::InstanceOf as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::CheckCast(idx) => {
                buf.push(Opcode::CheckCast as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::Cast(idx) => {
                buf.push(Opcode::Cast as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::Perform(idx, args) => {
                buf.push(Opcode::Perform as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::WithHandler(idx) => {
                buf.push(Opcode::WithHandler as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::ResumeWith => buf.push(Opcode::ResumeWith as u8),
            Instruction::CaptureCont => buf.push(Opcode::CaptureCont as u8),
            Instruction::Await => buf.push(Opcode::Await as u8),
            Instruction::BlockOn => buf.push(Opcode::BlockOn as u8),
            Instruction::MatchEffect(idx) => {
                buf.push(Opcode::MatchEffect as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::GetWitnessTable(idx1, idx2) => {
                buf.push(Opcode::GetWitnessTable as u8);
                buf.extend_from_slice(&idx1.to_le_bytes());
                buf.extend_from_slice(&idx2.to_le_bytes());
            }
            Instruction::WitnessMethod(idx) => {
                buf.push(Opcode::WitnessMethod as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
            }
            Instruction::OpenExistential => buf.push(Opcode::OpenExistential as u8),
            Instruction::CloseExistential => buf.push(Opcode::CloseExistential as u8),
            Instruction::Quote(v) => {
                buf.push(Opcode::Quote as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::Splice => buf.push(Opcode::Splice as u8),
            Instruction::Eval(args) => {
                buf.push(Opcode::Eval as u8);
                buf.push(*args);
            }
            Instruction::ExpandMacro(idx, args) => {
                buf.push(Opcode::ExpandMacro as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::FFICall(idx, args) => {
                buf.push(Opcode::FFICall as u8);
                buf.extend_from_slice(&idx.to_le_bytes());
                buf.push(*args);
            }
            Instruction::Halt => buf.push(Opcode::Halt as u8),
            Instruction::I32Const(v) => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Const as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::I32Add => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Add as u8);
            }
            Instruction::I32Sub => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Sub as u8);
            }
            Instruction::I32Mul => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Mul as u8);
            }
            Instruction::I32DivS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::DivS as u8);
            }
            Instruction::I32DivU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::DivU as u8);
            }
            Instruction::I32RemS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::RemS as u8);
            }
            Instruction::I32RemU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::RemU as u8);
            }
            Instruction::I32Neg => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Neg as u8);
            }
            Instruction::I32Eq => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Eq as u8);
            }
            Instruction::I32Ne => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Ne as u8);
            }
            Instruction::I32LtS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::LtS as u8);
            }
            Instruction::I32LtU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::LtU as u8);
            }
            Instruction::I32LeS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::LeS as u8);
            }
            Instruction::I32LeU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::LeU as u8);
            }
            Instruction::I32GtS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::GtS as u8);
            }
            Instruction::I32GtU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::GtU as u8);
            }
            Instruction::I32GeS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::GeS as u8);
            }
            Instruction::I32GeU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::GeU as u8);
            }
            Instruction::I32Extend64S => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Extend64S as u8);
            }
            Instruction::I32Extend64U => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Extend64U as u8);
            }
            Instruction::I32Trunc64SLow => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Trunc64SLow as u8);
            }
            Instruction::I32Trunc64S => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Trunc64S as u8);
            }
            Instruction::I32Trunc64U => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::Trunc64U as u8);
            }
            Instruction::I32ToF32S => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::ToF32S as u8);
            }
            Instruction::I32ToF32U => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::ToF32U as u8);
            }
            Instruction::I32ToF64S => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::ToF64S as u8);
            }
            Instruction::I32ToF64U => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::ToF64U as u8);
            }
            Instruction::I32AddSatS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::AddSatS as u8);
            }
            Instruction::I32AddSatU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::AddSatU as u8);
            }
            Instruction::I32SubSatS => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::SubSatS as u8);
            }
            Instruction::I32SubSatU => {
                buf.push(Opcode::I32Ext as u8);
                buf.push(I32Ext::SubSatU as u8);
            }
            Instruction::I64Const(v) => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Const as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::I64Add => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Add as u8);
            }
            Instruction::I64Sub => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Sub as u8);
            }
            Instruction::I64Mul => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Mul as u8);
            }
            Instruction::I64DivS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::DivS as u8);
            }
            Instruction::I64DivU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::DivU as u8);
            }
            Instruction::I64RemS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::RemS as u8);
            }
            Instruction::I64RemU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::RemU as u8);
            }
            Instruction::I64Neg => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Neg as u8);
            }
            Instruction::I64Eq => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Eq as u8);
            }
            Instruction::I64Ne => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::Ne as u8);
            }
            Instruction::I64LtS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::LtS as u8);
            }
            Instruction::I64LtU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::LtU as u8);
            }
            Instruction::I64LeS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::LeS as u8);
            }
            Instruction::I64LeU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::LeU as u8);
            }
            Instruction::I64GtS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::GtS as u8);
            }
            Instruction::I64GtU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::GtU as u8);
            }
            Instruction::I64GeS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::GeS as u8);
            }
            Instruction::I64GeU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::GeU as u8);
            }
            Instruction::I64ToF32S => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::ToF32S as u8);
            }
            Instruction::I64ToF32U => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::ToF32U as u8);
            }
            Instruction::I64ToF64S => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::ToF64S as u8);
            }
            Instruction::I64ToF64U => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::ToF64U as u8);
            }
            Instruction::I64AddSatS => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::AddSatS as u8);
            }
            Instruction::I64AddSatU => {
                buf.push(Opcode::I64Ext as u8);
                buf.push(I64Ext::AddSatU as u8);
            }
            Instruction::F32Const(v) => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Const as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::F32Add => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Add as u8);
            }
            Instruction::F32Sub => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Sub as u8);
            }
            Instruction::F32Mul => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Mul as u8);
            }
            Instruction::F32Div => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Div as u8);
            }
            Instruction::F32Neg => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Neg as u8);
            }
            Instruction::F32Eq => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Eq as u8);
            }
            Instruction::F32Ne => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Ne as u8);
            }
            Instruction::F32Lt => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Lt as u8);
            }
            Instruction::F32Le => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Le as u8);
            }
            Instruction::F32Gt => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Gt as u8);
            }
            Instruction::F32Ge => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::Ge as u8);
            }
            Instruction::F32ToI32S => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::ToI32S as u8);
            }
            Instruction::F32ToI32U => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::ToI32U as u8);
            }
            Instruction::F32ToI64S => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::ToI64S as u8);
            }
            Instruction::F32ToI64U => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::ToI64U as u8);
            }
            Instruction::F32ToF64 => {
                buf.push(Opcode::F32Ext as u8);
                buf.push(F32Ext::ToF64 as u8);
            }
            Instruction::F64Const(v) => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Const as u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
            Instruction::F64Add => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Add as u8);
            }
            Instruction::F64Sub => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Sub as u8);
            }
            Instruction::F64Mul => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Mul as u8);
            }
            Instruction::F64Div => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Div as u8);
            }
            Instruction::F64Neg => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Neg as u8);
            }
            Instruction::F64Eq => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Eq as u8);
            }
            Instruction::F64Ne => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Ne as u8);
            }
            Instruction::F64Lt => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Lt as u8);
            }
            Instruction::F64Le => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Le as u8);
            }
            Instruction::F64Gt => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Gt as u8);
            }
            Instruction::F64Ge => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::Ge as u8);
            }
            Instruction::F64ToI32S => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::ToI32S as u8);
            }
            Instruction::F64ToI32U => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::ToI32U as u8);
            }
            Instruction::F64ToI64S => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::ToI64S as u8);
            }
            Instruction::F64ToI64U => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::ToI64U as u8);
            }
            Instruction::F64ToF32 => {
                buf.push(Opcode::F64Ext as u8);
                buf.push(F64Ext::ToF32 as u8);
            }
            Instruction::BigIntConst { sign, bytes } => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Const as u8);
                buf.push(*sign);
                write_varuint(&mut buf, bytes.len() as u64);
                buf.extend_from_slice(bytes);
            }
            Instruction::BigIntAdd => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Add as u8);
            }
            Instruction::BigIntSub => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Sub as u8);
            }
            Instruction::BigIntMul => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Mul as u8);
            }
            Instruction::BigIntDiv => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Div as u8);
            }
            Instruction::BigIntMod => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Mod as u8);
            }
            Instruction::BigIntNeg => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Neg as u8);
            }
            Instruction::BigIntEq => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Eq as u8);
            }
            Instruction::BigIntNe => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Ne as u8);
            }
            Instruction::BigIntLt => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Lt as u8);
            }
            Instruction::BigIntLe => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Le as u8);
            }
            Instruction::BigIntGt => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Gt as u8);
            }
            Instruction::BigIntGe => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::Ge as u8);
            }
            Instruction::BigIntToI64 => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::ToI64 as u8);
            }
            Instruction::BigIntFromI64 => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::FromI64 as u8);
            }
            Instruction::BigIntToString => {
                buf.push(Opcode::BigIntExt as u8);
                buf.push(BigIntExt::ToString as u8);
            }
            Instruction::StringConst(v) => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Const as u8);
                write_varuint(&mut buf, v.len() as u64);
                buf.extend_from_slice(v.as_bytes());
            }
            Instruction::StringConcat => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Concat as u8);
            }
            Instruction::StringLenBytes => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::LenBytes as u8);
            }
            Instruction::StringSubstr => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Substr as u8);
            }
            Instruction::StringEq => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Eq as u8);
            }
            Instruction::StringNe => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Ne as u8);
            }
            Instruction::StringLt => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Lt as u8);
            }
            Instruction::StringLe => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Le as u8);
            }
            Instruction::StringGt => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Gt as u8);
            }
            Instruction::StringGe => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::Ge as u8);
            }
            Instruction::StringLenChars => {
                buf.push(Opcode::StringExt as u8);
                buf.push(StringExt::LenChars as u8);
            }
        }
        buf
    }
}

pub fn write_varuint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}
