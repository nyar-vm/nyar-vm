use crate::bytecode::opcode::Opcode;
use crate::bytecode::format::{Chunk, Constant};
use crate::vm::effects::{HandlerFrame, perform_effect_internal};
use crate::vm::value::{Value, ValueTag};
use crate::vm::VmError;

#[derive(Clone)]
struct Frame { code: Vec<u8>, ip: usize }

pub struct VM {
    stack: Vec<Value>,
    sp: usize,
    frames: Vec<Frame>,
    pub constants: Vec<Constant>,
    pub effects: Vec<String>,
    pub handler_stack: Vec<HandlerFrame>,
}

impl VM {
    pub fn new(constants: Vec<Constant>, effects: Vec<String>) -> Self { Self { stack: Vec::with_capacity(64), sp: 0, frames: Vec::new(), constants, effects, handler_stack: Vec::new() } }
    fn push(&mut self, v: Value) { if self.sp >= self.stack.len() { self.stack.push(v) } else { self.stack[self.sp] = v } self.sp += 1 }
    fn pop(&mut self) -> Result<Value, VmError> { if self.sp == 0 { Err(VmError::StackUnderflow) } else { self.sp -= 1; Ok(self.stack[self.sp]) } }
    fn peek_at(&self, depth: usize) -> Result<Value, VmError> { if depth >= self.sp { Err(VmError::StackUnderflow) } else { Ok(self.stack[self.sp - 1 - depth]) } }
    fn swap_with(&mut self, depth: usize) -> Result<(), VmError> { if depth >= self.sp { Err(VmError::StackUnderflow) } else { let top = self.sp - 1; let idx = self.sp - 1 - depth; self.stack.swap(top, idx); Ok(()) } }
    fn read_u8(frame: &mut Frame) -> Option<u8> { if frame.ip >= frame.code.len() { None } else { let v = frame.code[frame.ip]; frame.ip += 1; Some(v) } }
    fn read_u16(frame: &mut Frame) -> Option<u16> { if frame.ip + 1 >= frame.code.len() { None } else { let v = u16::from_le_bytes([frame.code[frame.ip], frame.code[frame.ip+1]]); frame.ip += 2; Some(v) } }
    fn read_i16(frame: &mut Frame) -> Option<i16> { Self::read_u16(frame).map(|x| x as i16) }
    pub fn execute(&mut self, chunk: &Chunk) -> Result<Value, VmError> {
        let mut frame = Frame { code: chunk.code.clone(), ip: 0 };
        self.frames.push(frame.clone());
        loop {
            let f = self.frames.last_mut().unwrap();
            let op = match Self::read_u8(f) { Some(v) => v, None => break };
            let opcode = unsafe { std::mem::transmute::<u8, Opcode>(op) };
            match opcode {
                Opcode::PushConst => {
                    let idx = match Self::read_u16(f) { Some(v) => v as usize, None => return Err(VmError::InvalidOpcode) };
                    let c = self.constants.get(idx).ok_or(VmError::IndexOutOfBounds)?;
                    match c {
                        Constant::Int(i) => self.push(Value::int(*i)),
                        Constant::Float(x) => self.push(Value::float(*x)),
                        Constant::String(_) => self.push(Value::null()),
                    }
                }
                Opcode::Pop => { let _ = self.pop()?; }
                Opcode::Dup => { let d = Self::read_u8(f).ok_or(VmError::InvalidOpcode)? as usize; let v = self.peek_at(d)?; self.push(v); }
                Opcode::Swap => { let d = Self::read_u8(f).ok_or(VmError::InvalidOpcode)? as usize; self.swap_with(d)?; }
                Opcode::Jump => { let off = Self::read_i16(f).ok_or(VmError::InvalidOpcode)?; if off < 0 { f.ip -= (-off) as usize } else { f.ip += off as usize } }
                Opcode::JumpIfFalse => { let off = Self::read_i16(f).ok_or(VmError::InvalidOpcode)?; let v = self.pop()?; let cond = unsafe { match v.tag { ValueTag::Bool => v.as_bool(), ValueTag::Null => false, _ => false } }; if !cond { if off < 0 { f.ip -= (-off) as usize } else { f.ip += off as usize } } }
                Opcode::Return => { let v = self.pop()?; self.frames.pop(); return Ok(v) }
                Opcode::Perform => { let idx = Self::read_u16(f).ok_or(VmError::InvalidOpcode)? as usize; let argc = Self::read_u8(f).ok_or(VmError::InvalidOpcode)? as usize; let mut args = Vec::with_capacity(argc); for _ in 0..argc { args.push(self.pop()?); } let name = self.effects.get(idx).cloned().unwrap_or_default(); let r = perform_effect_internal(self, name, args)?; if let Some(val) = r { self.push(val) } }
                Opcode::TypeOf => { let v = self.pop()?; let tid = match v.tag { ValueTag::Int => 0i64, ValueTag::Float => 1, ValueTag::Bool => 2, ValueTag::Null => 3, _ => 4 }; self.push(Value::int(tid)) }
                Opcode::Halt => { break }
                _ => {}
            }
        }
        Ok(Value::null())
    }
}

