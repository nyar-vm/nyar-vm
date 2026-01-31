use crate::bytecode::instruction::{Instruction, UpvalueRef};
use crate::bytecode::opcode::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodeError {
    Truncated,
    InvalidOpcode(u8),
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
        let mut shift: u32 = 0;
        loop {
            let byte = self.read_u8()?;
            result |= ((byte & 0x7F) as u64) << shift;
            if (byte & 0x80) == 0 {
                break;
            }
            shift += 7;
        }
        Some(result)
    }
    fn parse_opcode(op: u8) -> Option<Opcode> {
        if op <= 0x73 || op == 0xF0 || op == 0xFF || (0xC1..=0xC6).contains(&op) {
            return unsafe { std::mem::transmute(op) };
        }
        None
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
                    x if x == I32Ext::AddSatS as u8 => Instruction::I32AddSatS,
                    x if x == I32Ext::AddSatU as u8 => Instruction::I32AddSatU,
                    x if x == I32Ext::SubSatS as u8 => Instruction::I32SubSatS,
                    x if x == I32Ext::SubSatU as u8 => Instruction::I32SubSatU,
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
                    x if x == I64Ext::AddSatS as u8 => Instruction::I64AddSatS,
                    x if x == I64Ext::AddSatU as u8 => Instruction::I64AddSatU,
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
