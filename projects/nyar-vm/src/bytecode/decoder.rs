use crate::bytecode::opcode::{Opcode, I32Op, I64Op, F32Op, F64Op, BigIntOp, StringOp};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
pub use nyar_error::DecodeError;

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
    GetField(u16),
    SetField(u16),
    NewObject(u16),
    NewArray(u16),
    GetElement,
    SetElement,
    TypeOf,
    InstanceOf(u16),
    CheckCast(u16),
    Cast(u16),
    Perform(u16, u8),
    WithHandler(u16),
    ResumeWith,
    CaptureCont,
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
            if (b & 0x80) == 0 { break; }
            shift += 7;
            if shift > 63 { return None; }
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
            0x30 => Opcode::GetField,
            0x31 => Opcode::SetField,
            0x32 => Opcode::NewObject,
            0x33 => Opcode::NewArray,
            0x34 => Opcode::GetElement,
            0x35 => Opcode::SetElement,
            0x40 => Opcode::TypeOf,
            0x41 => Opcode::InstanceOf,
            0x42 => Opcode::CheckCast,
            0x43 => Opcode::Cast,
            0x50 => Opcode::Perform,
            0x51 => Opcode::WithHandler,
            0x52 => Opcode::ResumeWith,
            0x53 => Opcode::CaptureCont,
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
                    x if x == I32Op::Const as u8 => Instruction::I32Const(self.read_i32().ok_or(DecodeError::Truncated)?),
                    x if x == I32Op::Add as u8 => Instruction::I32Add,
                    x if x == I32Op::Sub as u8 => Instruction::I32Sub,
                    x if x == I32Op::Mul as u8 => Instruction::I32Mul,
                    x if x == I32Op::DivS as u8 => Instruction::I32DivS,
                    x if x == I32Op::DivU as u8 => Instruction::I32DivU,
                    x if x == I32Op::RemS as u8 => Instruction::I32RemS,
                    x if x == I32Op::RemU as u8 => Instruction::I32RemU,
                    x if x == I32Op::Neg as u8 => Instruction::I32Neg,
                    x if x == I32Op::Eq as u8 => Instruction::I32Eq,
                    x if x == I32Op::Ne as u8 => Instruction::I32Ne,
                    x if x == I32Op::LtS as u8 => Instruction::I32LtS,
                    x if x == I32Op::LtU as u8 => Instruction::I32LtU,
                    x if x == I32Op::LeS as u8 => Instruction::I32LeS,
                    x if x == I32Op::LeU as u8 => Instruction::I32LeU,
                    x if x == I32Op::GtS as u8 => Instruction::I32GtS,
                    x if x == I32Op::GtU as u8 => Instruction::I32GtU,
                    x if x == I32Op::GeS as u8 => Instruction::I32GeS,
                    x if x == I32Op::GeU as u8 => Instruction::I32GeU,
                    x if x == I32Op::Extend64S as u8 => Instruction::I32Extend64S,
                    x if x == I32Op::Extend64U as u8 => Instruction::I32Extend64U,
                    x if x == I32Op::Trunc64SLow as u8 => Instruction::I32Trunc64SLow,
                    x if x == I32Op::Trunc64S as u8 => Instruction::I32Trunc64S,
                    x if x == I32Op::Trunc64U as u8 => Instruction::I32Trunc64U,
                    x if x == I32Op::ToF32S as u8 => Instruction::I32ToF32S,
                    x if x == I32Op::ToF32U as u8 => Instruction::I32ToF32U,
                    x if x == I32Op::ToF64S as u8 => Instruction::I32ToF64S,
                    x if x == I32Op::ToF64U as u8 => Instruction::I32ToF64U,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC2 => match sub {
                    x if x == I64Op::Const as u8 => Instruction::I64Const(self.read_i64().ok_or(DecodeError::Truncated)?),
                    x if x == I64Op::Add as u8 => Instruction::I64Add,
                    x if x == I64Op::Sub as u8 => Instruction::I64Sub,
                    x if x == I64Op::Mul as u8 => Instruction::I64Mul,
                    x if x == I64Op::DivS as u8 => Instruction::I64DivS,
                    x if x == I64Op::DivU as u8 => Instruction::I64DivU,
                    x if x == I64Op::RemS as u8 => Instruction::I64RemS,
                    x if x == I64Op::RemU as u8 => Instruction::I64RemU,
                    x if x == I64Op::Neg as u8 => Instruction::I64Neg,
                    x if x == I64Op::Eq as u8 => Instruction::I64Eq,
                    x if x == I64Op::Ne as u8 => Instruction::I64Ne,
                    x if x == I64Op::LtS as u8 => Instruction::I64LtS,
                    x if x == I64Op::LtU as u8 => Instruction::I64LtU,
                    x if x == I64Op::LeS as u8 => Instruction::I64LeS,
                    x if x == I64Op::LeU as u8 => Instruction::I64LeU,
                    x if x == I64Op::GtS as u8 => Instruction::I64GtS,
                    x if x == I64Op::GtU as u8 => Instruction::I64GtU,
                    x if x == I64Op::GeS as u8 => Instruction::I64GeS,
                    x if x == I64Op::GeU as u8 => Instruction::I64GeU,
                    x if x == I64Op::ToF32S as u8 => Instruction::I64ToF32S,
                    x if x == I64Op::ToF32U as u8 => Instruction::I64ToF32U,
                    x if x == I64Op::ToF64S as u8 => Instruction::I64ToF64S,
                    x if x == I64Op::ToF64U as u8 => Instruction::I64ToF64U,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC3 => match sub {
                    x if x == F32Op::Const as u8 => Instruction::F32Const(self.read_f32().ok_or(DecodeError::Truncated)?),
                    x if x == F32Op::Add as u8 => Instruction::F32Add,
                    x if x == F32Op::Sub as u8 => Instruction::F32Sub,
                    x if x == F32Op::Mul as u8 => Instruction::F32Mul,
                    x if x == F32Op::Div as u8 => Instruction::F32Div,
                    x if x == F32Op::Neg as u8 => Instruction::F32Neg,
                    x if x == F32Op::Eq as u8 => Instruction::F32Eq,
                    x if x == F32Op::Ne as u8 => Instruction::F32Ne,
                    x if x == F32Op::Lt as u8 => Instruction::F32Lt,
                    x if x == F32Op::Le as u8 => Instruction::F32Le,
                    x if x == F32Op::Gt as u8 => Instruction::F32Gt,
                    x if x == F32Op::Ge as u8 => Instruction::F32Ge,
                    x if x == F32Op::ToI32S as u8 => Instruction::F32ToI32S,
                    x if x == F32Op::ToI32U as u8 => Instruction::F32ToI32U,
                    x if x == F32Op::ToI64S as u8 => Instruction::F32ToI64S,
                    x if x == F32Op::ToI64U as u8 => Instruction::F32ToI64U,
                    x if x == F32Op::ToF64 as u8 => Instruction::F32ToF64,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC4 => match sub {
                    x if x == F64Op::Const as u8 => Instruction::F64Const(self.read_f64().ok_or(DecodeError::Truncated)?),
                    x if x == F64Op::Add as u8 => Instruction::F64Add,
                    x if x == F64Op::Sub as u8 => Instruction::F64Sub,
                    x if x == F64Op::Mul as u8 => Instruction::F64Mul,
                    x if x == F64Op::Div as u8 => Instruction::F64Div,
                    x if x == F64Op::Neg as u8 => Instruction::F64Neg,
                    x if x == F64Op::Eq as u8 => Instruction::F64Eq,
                    x if x == F64Op::Ne as u8 => Instruction::F64Ne,
                    x if x == F64Op::Lt as u8 => Instruction::F64Lt,
                    x if x == F64Op::Le as u8 => Instruction::F64Le,
                    x if x == F64Op::Gt as u8 => Instruction::F64Gt,
                    x if x == F64Op::Ge as u8 => Instruction::F64Ge,
                    x if x == F64Op::ToI32S as u8 => Instruction::F64ToI32S,
                    x if x == F64Op::ToI32U as u8 => Instruction::F64ToI32U,
                    x if x == F64Op::ToI64S as u8 => Instruction::F64ToI64S,
                    x if x == F64Op::ToI64U as u8 => Instruction::F64ToI64U,
                    x if x == F64Op::ToF32 as u8 => Instruction::F64ToF32,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC5 => match sub {
                    x if x == BigIntOp::Const as u8 => {
                        let sign = self.read_u8().ok_or(DecodeError::Truncated)?;
                        let len = self.read_varuint().ok_or(DecodeError::Truncated)? as usize;
                        let mut bytes = vec![0u8; len];
                        for i in 0..len { bytes[i] = self.read_u8().ok_or(DecodeError::Truncated)?; }
                        Instruction::BigIntConst { sign, bytes }
                    }
                    x if x == BigIntOp::Add as u8 => Instruction::BigIntAdd,
                    x if x == BigIntOp::Sub as u8 => Instruction::BigIntSub,
                    x if x == BigIntOp::Mul as u8 => Instruction::BigIntMul,
                    x if x == BigIntOp::Div as u8 => Instruction::BigIntDiv,
                    x if x == BigIntOp::Mod as u8 => Instruction::BigIntMod,
                    x if x == BigIntOp::Neg as u8 => Instruction::BigIntNeg,
                    x if x == BigIntOp::Eq as u8 => Instruction::BigIntEq,
                    x if x == BigIntOp::Ne as u8 => Instruction::BigIntNe,
                    x if x == BigIntOp::Lt as u8 => Instruction::BigIntLt,
                    x if x == BigIntOp::Le as u8 => Instruction::BigIntLe,
                    x if x == BigIntOp::Gt as u8 => Instruction::BigIntGt,
                    x if x == BigIntOp::Ge as u8 => Instruction::BigIntGe,
                    x if x == BigIntOp::ToI64 as u8 => Instruction::BigIntToI64,
                    x if x == BigIntOp::FromI64 as u8 => Instruction::BigIntFromI64,
                    x if x == BigIntOp::ToString as u8 => Instruction::BigIntToString,
                    _ => return Err(DecodeError::InvalidOpcode(sub)),
                },
                x if x == 0xC6 => match sub {
                    x if x == StringOp::Const as u8 => {
                        let len = self.read_varuint().ok_or(DecodeError::Truncated)? as usize;
                        let mut bytes = vec![0u8; len];
                        for i in 0..len { bytes[i] = self.read_u8().ok_or(DecodeError::Truncated)?; }
                        let s = String::from_utf8(bytes).map_err(|_| DecodeError::InvalidOpcode(sub))?;
                        Instruction::StringConst(s)
                    }
                    x if x == StringOp::Concat as u8 => Instruction::StringConcat,
                    x if x == StringOp::LenBytes as u8 => Instruction::StringLenBytes,
                    x if x == StringOp::Substr as u8 => Instruction::StringSubstr,
                    x if x == StringOp::Eq as u8 => Instruction::StringEq,
                    x if x == StringOp::Ne as u8 => Instruction::StringNe,
                    x if x == StringOp::Lt as u8 => Instruction::StringLt,
                    x if x == StringOp::Le as u8 => Instruction::StringLe,
                    x if x == StringOp::Gt as u8 => Instruction::StringGt,
                    x if x == StringOp::Ge as u8 => Instruction::StringGe,
                    x if x == StringOp::LenChars as u8 => Instruction::StringLenChars,
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
            Opcode::I32Ext | Opcode::I64Ext | Opcode::F32Ext | Opcode::F64Ext | Opcode::BigIntExt | Opcode::StringExt => return Err(DecodeError::InvalidOpcode(op)),
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
