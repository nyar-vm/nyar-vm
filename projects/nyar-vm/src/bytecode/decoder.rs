use crate::bytecode::opcode::Opcode;

#[derive(Debug, Clone, PartialEq)]
pub enum Operand { U8(u8), U16(u16), I16(i16), U32(u32) }

#[derive(Debug, Clone, PartialEq)]
pub struct Instruction { pub opcode: Opcode, pub operands: Vec<Operand> }

pub struct Decoder<'a> { code: &'a [u8], ip: usize }

impl<'a> Decoder<'a> {
    pub fn new(code: &'a [u8]) -> Self { Self { code, ip: 0 } }
    fn read_u8(&mut self) -> Option<u8> { if self.ip >= self.code.len() { None } else { let v = self.code[self.ip]; self.ip += 1; Some(v) } }
    fn read_u16(&mut self) -> Option<u16> { if self.ip + 1 >= self.code.len() { None } else { let v = u16::from_le_bytes([self.code[self.ip], self.code[self.ip+1]]); self.ip += 2; Some(v) } }
    fn read_i16(&mut self) -> Option<i16> { self.read_u16().map(|x| x as i16) }
    fn read_u32(&mut self) -> Option<u32> { if self.ip + 3 >= self.code.len() { None } else { let v = u32::from_le_bytes([self.code[self.ip], self.code[self.ip+1], self.code[self.ip+2], self.code[self.ip+3]]); self.ip += 4; Some(v) } }
    pub fn next(&mut self) -> Option<Instruction> {
        let op = self.read_u8()?;
        let opcode = unsafe { std::mem::transmute::<u8, Opcode>(op) };
        let mut operands = Vec::new();
        match opcode {
            Opcode::PushConst => { operands.push(Operand::U16(self.read_u16()?)); }
            Opcode::Dup => { operands.push(Operand::U8(self.read_u8()?)); }
            Opcode::Swap => { operands.push(Operand::U8(self.read_u8()?)); }
            Opcode::LoadLocal | Opcode::StoreLocal | Opcode::LoadUpvalue | Opcode::StoreUpvalue => { operands.push(Operand::U8(self.read_u8()?)); }
            Opcode::LoadGlobal | Opcode::StoreGlobal | Opcode::GetField | Opcode::SetField | Opcode::NewObject | Opcode::NewArray | Opcode::InstanceOf | Opcode::CheckCast | Opcode::Cast | Opcode::GetWitnessTable | Opcode::WitnessMethod | Opcode::ExpandMacro => { operands.push(Operand::U16(self.read_u16()?)); }
            Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfNull => { operands.push(Operand::I16(self.read_i16()?)); }
            Opcode::Call | Opcode::CallVirtual | Opcode::CallDynamic | Opcode::Perform | Opcode::Eval | Opcode::FFICall => { operands.push(Operand::U16(self.read_u16()?)); operands.push(Operand::U8(self.read_u8()?)); }
            Opcode::Quote => { operands.push(Operand::U32(self.read_u32()?)); }
            _ => {}
        }
        Some(Instruction { opcode, operands })
    }
}

