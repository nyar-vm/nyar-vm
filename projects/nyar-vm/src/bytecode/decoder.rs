use crate::bytecode::opcode::{BigIntExt, F32Ext, F64Ext, I32Ext, I64Ext, Opcode, StringExt};
use byteorder::{LittleEndian, ReadBytesExt};
pub use nyar_error::DecodeError;
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq)]
pub struct UpvalueRef {
    pub is_local: bool,
    pub index: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Nop,
    Push(u16),
    Pop,
    Dup(u8),
    Swap(u8),
    LoadLocal(u8),
    StoreLocal(u8),
    LoadGlobal(u16),
    StoreGlobal(u16),
    LoadUpvalue(u8),
    StoreUpvalue(u8),
    CloseUpvalues,
    Jump(i16),
    JumpIfFalse(i16),
    JumpIfNull(i16),
    Return,
    MakeClosure(u16, Vec<UpvalueRef>),
    TailCall,
    Call(u16, u8),
    CallVirtual(u16, u8),
    CallDynamic(u16, u8),
    CallClosure(u8),
    InvokeMethod(u16, u8),
    CallSymbol(u16, u8),
    GetField(u16),
    SetField(u16),
    NewObject(u16),
    NewArray(u16),
    GetElement,
    SetElement,
    MakeTuple(u8),
    HasKey,
    MatchVariant(u16),
    SizeOf,
    NewDynObject,
    RemoveKey,
    NewList(u16),
    PushElementLeft,
    PopElementLeft,
    PushElementRight,
    PopElementRight,
    TypeOf,
    InstanceOf(u16),
    CheckCast(u16),
    Cast(u16),
    Perform(u16, u8),
    WithHandler(u16),
    ResumeWith,
    CaptureCont,
    Await,
    BlockOn,
    MatchEffect(u16),
    GetWitnessTable(u16, u16),
    WitnessMethod(u16),
    OpenExistential,
    CloseExistential,
    Quote(u32),
    Splice,
    Eval(u8),
    ExpandMacro(u16, u8),
    FFICall(u16, u8),
    Halt,
    I32Const(i32),
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32Neg,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32LeS,
    I32LeU,
    I32GtS,
    I32GtU,
    I32GeS,
    I32GeU,
    I32Extend64S,
    I32Extend64U,
    I32Trunc64SLow,
    I32Trunc64S,
    I32Trunc64U,
    I32ToF32S,
    I32ToF32U,
    I32ToF64S,
    I32ToF64U,
    I64Const(i64),
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64Neg,
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64LeS,
    I64LeU,
    I64GtS,
    I64GtU,
    I64GeS,
    I64GeU,
    I64ToF32S,
    I64ToF32U,
    I64ToF64S,
    I64ToF64U,
    F32Const(f32),
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Neg,
    F32Eq,
    F32Ne,
    F32Lt,
    F32Le,
    F32Gt,
    F32Ge,
    F32ToI32S,
    F32ToI32U,
    F32ToI64S,
    F32ToI64U,
    F32ToF64,
    F64Const(f64),
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Neg,
    F64Eq,
    F64Ne,
    F64Lt,
    F64Le,
    F64Gt,
    F64Ge,
    F64ToI32S,
    F64ToI32U,
    F64ToI64S,
    F64ToI64U,
    F64ToF32,
    BigIntConst { sign: u8, bytes: Vec<u8> },
    BigIntAdd,
    BigIntSub,
    BigIntMul,
    BigIntDiv,
    BigIntMod,
    BigIntNeg,
    BigIntEq,
    BigIntNe,
    BigIntLt,
    BigIntLe,
    BigIntGt,
    BigIntGe,
    BigIntToI64,
    BigIntFromI64,
    BigIntToString,
    StringConst(String),
    StringConcat,
    StringLenBytes,
    StringSubstr,
    StringEq,
    StringNe,
    StringLt,
    StringLe,
    StringGt,
    StringGe,
    StringLenChars,
}

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
            Instruction::TailCall => buf.push(Opcode::TailCall as u8),
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
                Self::write_varuint(&mut buf, bytes.len() as u64);
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
                Self::write_varuint(&mut buf, v.len() as u64);
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
    fn write_varuint(buf: &mut Vec<u8>, mut v: u64) {
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

}

pub struct Decoder<'a> {
    code: &'a [u8],
    cursor: Cursor<&'a [u8]>,
}

impl<'a> Decoder<'a> {
    pub fn new(code: &'a [u8]) -> Self {
        Self {
            code,
            cursor: Cursor::new(code),
        }
    }
    pub fn position(&self) -> u64 {
        self.cursor.position()
    }
    fn read_u8(&mut self) -> Option<u8> {
        self.cursor.read_u8().ok()
    }
    fn read_u16(&mut self) -> Option<u16> {
        self.cursor.read_u16::<LittleEndian>().ok()
    }
    fn read_i16(&mut self) -> Option<i16> {
        self.cursor.read_i16::<LittleEndian>().ok()
    }
    fn read_u32(&mut self) -> Option<u32> {
        self.cursor.read_u32::<LittleEndian>().ok()
    }
    fn read_i32(&mut self) -> Option<i32> {
        self.cursor.read_i32::<LittleEndian>().ok()
    }
    fn read_i64(&mut self) -> Option<i64> {
        self.cursor.read_i64::<LittleEndian>().ok()
    }
    fn read_f32(&mut self) -> Option<f32> {
        self.cursor.read_f32::<LittleEndian>().ok()
    }
    fn read_f64(&mut self) -> Option<f64> {
        self.cursor.read_f64::<LittleEndian>().ok()
    }
    fn read_varuint(&mut self) -> Option<u64> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        loop {
            let b = self.read_u8()? as u64;
            result |= (b & 0x7F) << shift;
            if (b & 0x80) == 0 {
                break;
            }
            shift += 7;
            if shift > 63 {
                return None;
            }
        }
        Some(result)
    }
    fn parse_opcode(b: u8) -> Option<Opcode> {
        Some(match b {
            0x00 => Opcode::Nop,
            0x01 => Opcode::Push,
            0x02 => Opcode::Pop,
            0x03 => Opcode::Dup,
            0x04 => Opcode::Swap,
            0x05 => Opcode::LoadLocal,
            0x06 => Opcode::StoreLocal,
            0x07 => Opcode::LoadGlobal,
            0x08 => Opcode::StoreGlobal,
            0x09 => Opcode::LoadUpvalue,
            0x0A => Opcode::StoreUpvalue,
            0x0B => Opcode::CloseUpvalues,
            0x10 => Opcode::Jump,
            0x11 => Opcode::JumpIfFalse,
            0x12 => Opcode::JumpIfNull,
            0x13 => Opcode::Return,
            0x14 => Opcode::TailCall,
            0x15 => Opcode::MakeClosure,
            0x20 => Opcode::Call,
            0x21 => Opcode::CallVirtual,
            0x22 => Opcode::CallDynamic,
            0x23 => Opcode::CallClosure,
            0x24 => Opcode::InvokeMethod,
            0x25 => Opcode::CallSymbol,
            0x30 => Opcode::GetField,
            0x31 => Opcode::SetField,
            0x32 => Opcode::NewObject,
            0x33 => Opcode::NewArray,
            0x34 => Opcode::GetElement,
            0x35 => Opcode::SetElement,
            0x36 => Opcode::MakeTuple,
            0x37 => Opcode::HasKey,
            0x38 => Opcode::MatchVariant,
            0x39 => Opcode::SizeOf,
            0x3A => Opcode::NewDynObject,
            0x3B => Opcode::RemoveKey,
            0x3C => Opcode::NewList,
            0x3D => Opcode::PushElementLeft,
            0x3E => Opcode::PopElementLeft,
            0x3F => Opcode::PushElementRight,
            0x40 => Opcode::TypeOf,
            0x41 => Opcode::InstanceOf,
            0x42 => Opcode::CheckCast,
            0x43 => Opcode::Cast,
            0x44 => Opcode::PopElementRight,
            0x50 => Opcode::Perform,
            0x51 => Opcode::WithHandler,
            0x52 => Opcode::ResumeWith,
            0x53 => Opcode::CaptureCont,
            0x54 => Opcode::Await,
            0x55 => Opcode::BlockOn,
            0x56 => Opcode::MatchEffect,
            0x60 => Opcode::GetWitnessTable,
            0x61 => Opcode::WitnessMethod,
            0x62 => Opcode::OpenExistential,
            0x63 => Opcode::CloseExistential,
            0x70 => Opcode::Quote,
            0x71 => Opcode::Splice,
            0x72 => Opcode::Eval,
            0x73 => Opcode::ExpandMacro,
            0xF0 => Opcode::FFICall,
            0xFF => Opcode::Halt,
            _ => return None,
        })
    }
    pub fn next_result(&mut self) -> Result<Instruction, DecodeError> {
        let op = self.read_u8().ok_or(DecodeError::Truncated)?;
        if (0xC1..=0xC6).contains(&op) {
            let sub = self.read_u8().ok_or(DecodeError::Truncated)?;
            let ins = match op {
                x if x == 0xC1 => match sub {
                    x if x == I32Ext::Const as u8 => {
                        Instruction::I32Const(self.read_i32().ok_or(DecodeError::Truncated)?)
                    }
                    x if x == I32Ext::Add as u8 => Instruction::I32Add,
                    x if x == I32Ext::Sub as u8 => Instruction::I32Sub,
                    x if x == I32Ext::Mul as u8 => Instruction::I32Mul,
                    x if x == I32Ext::DivS as u8 => Instruction::I32DivS,
                    x if x == I32Ext::DivU as u8 => Instruction::I32DivU,
                    x if x == I32Ext::RemS as u8 => Instruction::I32RemS,
                    x if x == I32Ext::RemU as u8 => Instruction::I32RemU,
                    x if x == I32Ext::Neg as u8 => Instruction::I32Neg,
                    x if x == I32Ext::Eq as u8 => Instruction::I32Eq,
                    x if x == I32Ext::Ne as u8 => Instruction::I32Ne,
                    x if x == I32Ext::LtS as u8 => Instruction::I32LtS,
                    x if x == I32Ext::LtU as u8 => Instruction::I32LtU,
                    x if x == I32Ext::LeS as u8 => Instruction::I32LeS,
                    x if x == I32Ext::LeU as u8 => Instruction::I32LeU,
                    x if x == I32Ext::GtS as u8 => Instruction::I32GtS,
                    x if x == I32Ext::GtU as u8 => Instruction::I32GtU,
                    x if x == I32Ext::GeS as u8 => Instruction::I32GeS,
                    x if x == I32Ext::GeU as u8 => Instruction::I32GeU,
                    x if x == I32Ext::Extend64S as u8 => Instruction::I32Extend64S,
                    x if x == I32Ext::Extend64U as u8 => Instruction::I32Extend64U,
                    x if x == I32Ext::Trunc64SLow as u8 => Instruction::I32Trunc64SLow,
                    x if x == I32Ext::Trunc64S as u8 => Instruction::I32Trunc64S,
                    x if x == I32Ext::Trunc64U as u8 => Instruction::I32Trunc64U,
                    x if x == I32Ext::ToF32S as u8 => Instruction::I32ToF32S,
                    x if x == I32Ext::ToF32U as u8 => Instruction::I32ToF32U,
                    x if x == I32Ext::ToF64S as u8 => Instruction::I32ToF64S,
                    x if x == I32Ext::ToF64U as u8 => Instruction::I32ToF64U,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC2 => match sub {
                    x if x == I64Ext::Const as u8 => {
                        Instruction::I64Const(self.read_i64().ok_or(DecodeError::Truncated)?)
                    }
                    x if x == I64Ext::Add as u8 => Instruction::I64Add,
                    x if x == I64Ext::Sub as u8 => Instruction::I64Sub,
                    x if x == I64Ext::Mul as u8 => Instruction::I64Mul,
                    x if x == I64Ext::DivS as u8 => Instruction::I64DivS,
                    x if x == I64Ext::DivU as u8 => Instruction::I64DivU,
                    x if x == I64Ext::RemS as u8 => Instruction::I64RemS,
                    x if x == I64Ext::RemU as u8 => Instruction::I64RemU,
                    x if x == I64Ext::Neg as u8 => Instruction::I64Neg,
                    x if x == I64Ext::Eq as u8 => Instruction::I64Eq,
                    x if x == I64Ext::Ne as u8 => Instruction::I64Ne,
                    x if x == I64Ext::LtS as u8 => Instruction::I64LtS,
                    x if x == I64Ext::LtU as u8 => Instruction::I64LtU,
                    x if x == I64Ext::LeS as u8 => Instruction::I64LeS,
                    x if x == I64Ext::LeU as u8 => Instruction::I64LeU,
                    x if x == I64Ext::GtS as u8 => Instruction::I64GtS,
                    x if x == I64Ext::GtU as u8 => Instruction::I64GtU,
                    x if x == I64Ext::GeS as u8 => Instruction::I64GeS,
                    x if x == I64Ext::GeU as u8 => Instruction::I64GeU,
                    x if x == I64Ext::ToF32S as u8 => Instruction::I64ToF32S,
                    x if x == I64Ext::ToF32U as u8 => Instruction::I64ToF32U,
                    x if x == I64Ext::ToF64S as u8 => Instruction::I64ToF64S,
                    x if x == I64Ext::ToF64U as u8 => Instruction::I64ToF64U,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC3 => match sub {
                    x if x == F32Ext::Const as u8 => {
                        Instruction::F32Const(self.read_f32().ok_or(DecodeError::Truncated)?)
                    }
                    x if x == F32Ext::Add as u8 => Instruction::F32Add,
                    x if x == F32Ext::Sub as u8 => Instruction::F32Sub,
                    x if x == F32Ext::Mul as u8 => Instruction::F32Mul,
                    x if x == F32Ext::Div as u8 => Instruction::F32Div,
                    x if x == F32Ext::Neg as u8 => Instruction::F32Neg,
                    x if x == F32Ext::Eq as u8 => Instruction::F32Eq,
                    x if x == F32Ext::Ne as u8 => Instruction::F32Ne,
                    x if x == F32Ext::Lt as u8 => Instruction::F32Lt,
                    x if x == F32Ext::Le as u8 => Instruction::F32Le,
                    x if x == F32Ext::Gt as u8 => Instruction::F32Gt,
                    x if x == F32Ext::Ge as u8 => Instruction::F32Ge,
                    x if x == F32Ext::ToI32S as u8 => Instruction::F32ToI32S,
                    x if x == F32Ext::ToI32U as u8 => Instruction::F32ToI32U,
                    x if x == F32Ext::ToI64S as u8 => Instruction::F32ToI64S,
                    x if x == F32Ext::ToI64U as u8 => Instruction::F32ToI64U,
                    x if x == F32Ext::ToF64 as u8 => Instruction::F32ToF64,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC4 => match sub {
                    x if x == F64Ext::Const as u8 => {
                        Instruction::F64Const(self.read_f64().ok_or(DecodeError::Truncated)?)
                    }
                    x if x == F64Ext::Add as u8 => Instruction::F64Add,
                    x if x == F64Ext::Sub as u8 => Instruction::F64Sub,
                    x if x == F64Ext::Mul as u8 => Instruction::F64Mul,
                    x if x == F64Ext::Div as u8 => Instruction::F64Div,
                    x if x == F64Ext::Neg as u8 => Instruction::F64Neg,
                    x if x == F64Ext::Eq as u8 => Instruction::F64Eq,
                    x if x == F64Ext::Ne as u8 => Instruction::F64Ne,
                    x if x == F64Ext::Lt as u8 => Instruction::F64Lt,
                    x if x == F64Ext::Le as u8 => Instruction::F64Le,
                    x if x == F64Ext::Gt as u8 => Instruction::F64Gt,
                    x if x == F64Ext::Ge as u8 => Instruction::F64Ge,
                    x if x == F64Ext::ToI32S as u8 => Instruction::F64ToI32S,
                    x if x == F64Ext::ToI32U as u8 => Instruction::F64ToI32U,
                    x if x == F64Ext::ToI64S as u8 => Instruction::F64ToI64S,
                    x if x == F64Ext::ToI64U as u8 => Instruction::F64ToI64U,
                    x if x == F64Ext::ToF32 as u8 => Instruction::F64ToF32,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC5 => match sub {
                    x if x == BigIntExt::Const as u8 => {
                        let sign = self.read_u8().ok_or(DecodeError::Truncated)?;
                        let len = self.read_varuint().ok_or(DecodeError::Truncated)? as usize;
                        let mut bytes = vec![0u8; len];
                        for i in 0..len {
                            bytes[i] = self.read_u8().ok_or(DecodeError::Truncated)?;
                        }
                        Instruction::BigIntConst { sign, bytes }
                    }
                    x if x == BigIntExt::Add as u8 => Instruction::BigIntAdd,
                    x if x == BigIntExt::Sub as u8 => Instruction::BigIntSub,
                    x if x == BigIntExt::Mul as u8 => Instruction::BigIntMul,
                    x if x == BigIntExt::Div as u8 => Instruction::BigIntDiv,
                    x if x == BigIntExt::Mod as u8 => Instruction::BigIntMod,
                    x if x == BigIntExt::Neg as u8 => Instruction::BigIntNeg,
                    x if x == BigIntExt::Eq as u8 => Instruction::BigIntEq,
                    x if x == BigIntExt::Ne as u8 => Instruction::BigIntNe,
                    x if x == BigIntExt::Lt as u8 => Instruction::BigIntLt,
                    x if x == BigIntExt::Le as u8 => Instruction::BigIntLe,
                    x if x == BigIntExt::Gt as u8 => Instruction::BigIntGt,
                    x if x == BigIntExt::Ge as u8 => Instruction::BigIntGe,
                    x if x == BigIntExt::ToI64 as u8 => Instruction::BigIntToI64,
                    x if x == BigIntExt::FromI64 as u8 => Instruction::BigIntFromI64,
                    x if x == BigIntExt::ToString as u8 => Instruction::BigIntToString,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC6 => match sub {
                    x if x == StringExt::Const as u8 => {
                        let len = self.read_varuint().ok_or(DecodeError::Truncated)? as usize;
                        let mut bytes = vec![0u8; len];
                        for i in 0..len {
                            bytes[i] = self.read_u8().ok_or(DecodeError::Truncated)?;
                        }
                        let s = String::from_utf8(bytes)
                            .map_err(|_| DecodeError::InvalidOpcode(sub))?;
                        Instruction::StringConst(s)
                    }
                    x if x == StringExt::Concat as u8 => Instruction::StringConcat,
                    x if x == StringExt::LenBytes as u8 => Instruction::StringLenBytes,
                    x if x == StringExt::Substr as u8 => Instruction::StringSubstr,
                    x if x == StringExt::Eq as u8 => Instruction::StringEq,
                    x if x == StringExt::Ne as u8 => Instruction::StringNe,
                    x if x == StringExt::Lt as u8 => Instruction::StringLt,
                    x if x == StringExt::Le as u8 => Instruction::StringLe,
                    x if x == StringExt::Gt as u8 => Instruction::StringGt,
                    x if x == StringExt::Ge as u8 => Instruction::StringGe,
                    x if x == StringExt::LenChars as u8 => Instruction::StringLenChars,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                _ => return Err(DecodeError::InvalidOpcode(op)),
            };
            return Ok(ins);
        }
        let opcode = Self::parse_opcode(op).ok_or(DecodeError::InvalidOpcode(op))?;
        let ins = match opcode {
            Opcode::Nop => Instruction::Nop,
            Opcode::Push => Instruction::Push(self.read_u16().ok_or(DecodeError::Truncated)?),
            Opcode::Pop => Instruction::Pop,
            Opcode::Dup => Instruction::Dup(self.read_u8().ok_or(DecodeError::Truncated)?),
            Opcode::Swap => Instruction::Swap(self.read_u8().ok_or(DecodeError::Truncated)?),
            Opcode::LoadLocal => {
                Instruction::LoadLocal(self.read_u8().ok_or(DecodeError::Truncated)?)
            }
            Opcode::StoreLocal => {
                Instruction::StoreLocal(self.read_u8().ok_or(DecodeError::Truncated)?)
            }
            Opcode::LoadGlobal => {
                Instruction::LoadGlobal(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::StoreGlobal => {
                Instruction::StoreGlobal(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::LoadUpvalue => {
                Instruction::LoadUpvalue(self.read_u8().ok_or(DecodeError::Truncated)?)
            }
            Opcode::StoreUpvalue => {
                Instruction::StoreUpvalue(self.read_u8().ok_or(DecodeError::Truncated)?)
            }
            Opcode::CloseUpvalues => Instruction::CloseUpvalues,
            Opcode::Jump => Instruction::Jump(self.read_i16().ok_or(DecodeError::Truncated)?),
            Opcode::JumpIfFalse => {
                Instruction::JumpIfFalse(self.read_i16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::JumpIfNull => {
                Instruction::JumpIfNull(self.read_i16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::Return => Instruction::Return,
            Opcode::MakeClosure => {
                let func_idx = self.read_u16().ok_or(DecodeError::Truncated)?;
                let count = self.read_u8().ok_or(DecodeError::Truncated)?;
                let mut upvalues = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let is_local = self.read_u8().ok_or(DecodeError::Truncated)? != 0;
                    let index = self.read_u8().ok_or(DecodeError::Truncated)?;
                    upvalues.push(UpvalueRef { is_local, index });
                }
                Instruction::MakeClosure(func_idx, upvalues)
            }
            Opcode::TailCall => Instruction::TailCall,
            Opcode::Call => Instruction::Call(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::CallVirtual => Instruction::CallVirtual(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::CallDynamic => Instruction::CallDynamic(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::CallClosure => {
                Instruction::CallClosure(self.read_u8().ok_or(DecodeError::Truncated)?)
            }
            Opcode::InvokeMethod => Instruction::InvokeMethod(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::CallSymbol => Instruction::CallSymbol(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::GetField => {
                Instruction::GetField(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::SetField => {
                Instruction::SetField(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::NewObject => {
                Instruction::NewObject(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::NewArray => {
                Instruction::NewArray(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::GetElement => Instruction::GetElement,
            Opcode::SetElement => Instruction::SetElement,
            Opcode::NewDynObject => Instruction::NewDynObject,
            Opcode::RemoveKey => Instruction::RemoveKey,
            Opcode::NewList => Instruction::NewList(self.read_u16().ok_or(DecodeError::Truncated)?),
            Opcode::PushElementLeft => Instruction::PushElementLeft,
            Opcode::PopElementLeft => Instruction::PopElementLeft,
            Opcode::PushElementRight => Instruction::PushElementRight,
            Opcode::PopElementRight => Instruction::PopElementRight,
            Opcode::MakeTuple => {
                Instruction::MakeTuple(self.read_u8().ok_or(DecodeError::Truncated)?)
            }
            Opcode::HasKey => Instruction::HasKey,
            Opcode::MatchVariant => {
                Instruction::MatchVariant(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::SizeOf => Instruction::SizeOf,
            Opcode::TypeOf => Instruction::TypeOf,
            Opcode::InstanceOf => {
                Instruction::InstanceOf(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::CheckCast => {
                Instruction::CheckCast(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::Cast => Instruction::Cast(self.read_u16().ok_or(DecodeError::Truncated)?),
            Opcode::Perform => Instruction::Perform(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::WithHandler => {
                Instruction::WithHandler(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::ResumeWith => Instruction::ResumeWith,
            Opcode::CaptureCont => Instruction::CaptureCont,
            Opcode::Await => Instruction::Await,
            Opcode::BlockOn => Instruction::BlockOn,
            Opcode::MatchEffect => {
                Instruction::MatchEffect(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::GetWitnessTable => Instruction::GetWitnessTable(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u16().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::WitnessMethod => {
                Instruction::WitnessMethod(self.read_u16().ok_or(DecodeError::Truncated)?)
            }
            Opcode::OpenExistential => Instruction::OpenExistential,
            Opcode::CloseExistential => Instruction::CloseExistential,
            Opcode::Quote => Instruction::Quote(self.read_u32().ok_or(DecodeError::Truncated)?),
            Opcode::Splice => Instruction::Splice,
            Opcode::Eval => Instruction::Eval(self.read_u8().ok_or(DecodeError::Truncated)?),
            Opcode::ExpandMacro => Instruction::ExpandMacro(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::FFICall => Instruction::FFICall(
                self.read_u16().ok_or(DecodeError::Truncated)?,
                self.read_u8().ok_or(DecodeError::Truncated)?,
            ),
            Opcode::Halt => Instruction::Halt,
            Opcode::I32Ext
            | Opcode::I64Ext
            | Opcode::F32Ext
            | Opcode::F64Ext
            | Opcode::BigIntExt
            | Opcode::StringExt => return Err(DecodeError::InvalidOpcode(op)),
        };
        Ok(ins)
    }
    pub fn decode_all(mut self) -> Result<Vec<Instruction>, DecodeError> {
        let mut out = Vec::new();
        while (self.cursor.position() as usize) < self.code.len() {
            out.push(self.next_result()?);
        }
        Ok(out)
    }
}
