use crate::bytecode::opcode::Opcode;
use std::io::Cursor;
use byteorder::{ReadBytesExt, LittleEndian};

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
    TailCall,
    Call(u16, u8),
    CallVirtual(u16, u8),
    CallDynamic(u16, u8),
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
}

pub struct Decoder<'a> {
    code: &'a [u8],
    cursor: Cursor<&'a [u8]>,
}

#[derive(Debug)]
pub enum DecodeError {
    InvalidOpcode(u8),
    Truncated,
}

impl<'a> Decoder<'a> {
    pub fn new(code: &'a [u8]) -> Self { Self { code, cursor: Cursor::new(code) } }
    fn read_u8(&mut self) -> Option<u8> { self.cursor.read_u8().ok() }
    fn read_u16(&mut self) -> Option<u16> { self.cursor.read_u16::<LittleEndian>().ok() }
    fn read_i16(&mut self) -> Option<i16> { self.cursor.read_i16::<LittleEndian>().ok() }
    fn read_u32(&mut self) -> Option<u32> { self.cursor.read_u32::<LittleEndian>().ok() }
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
            0x20 => Opcode::Call,
            0x21 => Opcode::CallVirtual,
            0x22 => Opcode::CallDynamic,
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
