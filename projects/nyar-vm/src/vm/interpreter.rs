use crate::bytecode::decoder::Instruction;
use crate::bytecode::format::Constant;
use crate::vm::effects::{perform_effect_internal, HandlerFrame};
use crate::vm::value::{Value, ValueTag};
use crate::vm::VmError;

#[derive(Clone)]
struct Frame {
    instrs: Vec<Instruction>,
    ip: usize,
}

pub struct NyarVM {
    stack: Vec<Value>,
    sp: usize,
    frames: Vec<Frame>,
    pub constants: Vec<Constant>,
    pub effects: Vec<String>,
    pub handler_stack: Vec<HandlerFrame>,
}

impl NyarVM {
    pub fn new(constants: Vec<Constant>, effects: Vec<String>) -> Self {
        Self {
            stack: Vec::with_capacity(64),
            sp: 0,
            frames: Vec::new(),
            constants,
            effects,
            handler_stack: Vec::new(),
        }
    }
    fn push(&mut self, v: Value) {
        if self.sp >= self.stack.len() {
            self.stack.push(v)
        } else {
            self.stack[self.sp] = v
        }
        self.sp += 1
    }
    fn pop(&mut self) -> Result<Value, VmError> {
        if self.sp == 0 {
            Err(VmError::StackUnderflow)
        } else {
            self.sp -= 1;
            Ok(self.stack[self.sp])
        }
    }
    fn peek_at(&self, depth: usize) -> Result<Value, VmError> {
        if depth >= self.sp {
            Err(VmError::StackUnderflow)
        } else {
            Ok(self.stack[self.sp - 1 - depth])
        }
    }
    fn swap_with(&mut self, depth: usize) -> Result<(), VmError> {
        if depth >= self.sp {
            Err(VmError::StackUnderflow)
        } else {
            let top = self.sp - 1;
            let idx = self.sp - 1 - depth;
            self.stack.swap(top, idx);
            Ok(())
        }
    }
    pub fn execute(&mut self, program: &[Instruction]) -> Result<Value, VmError> {
        let frame = Frame {
            instrs: program.to_vec(),
            ip: 0,
        };
        self.frames.push(frame.clone());
        loop {
            let (ins, cur_ip) = {
                let f = self.frames.last().unwrap();
                if f.ip >= f.instrs.len() {
                    break;
                }
                (f.instrs[f.ip].clone(), f.ip)
            };
            let mut next_ip = Some(cur_ip + 1);
            match ins {
                Instruction::Nop => {}
                Instruction::Push(idx) => {
                    let c = self
                        .constants
                        .get(idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    match c {
                        Constant::Int(i) => self.push(Value::int(*i)),
                        Constant::Float(x) => self.push(Value::float(*x)),
                        Constant::String(_) => self.push(Value::null()),
                    }
                }
                Instruction::Pop => {
                    let _ = self.pop()?;
                }
                Instruction::Dup(d) => {
                    let v = self.peek_at(d as usize)?;
                    self.push(v);
                }
                Instruction::Swap(d) => {
                    self.swap_with(d as usize)?;
                }
                Instruction::Jump(off) => {
                    let target = (cur_ip as isize + off as isize) as usize;
                    next_ip = Some(target);
                }
                Instruction::JumpIfFalse(off) => {
                    let v = self.pop()?;
                    let cond = unsafe {
                        match v.tag {
                            ValueTag::Bool => v.as_bool(),
                            ValueTag::Null => false,
                            _ => false,
                        }
                    };
                    if !cond {
                        next_ip = Some((cur_ip as isize + off as isize) as usize);
                    }
                }
                Instruction::Return => {
                    let v = self.pop()?;
                    self.frames.pop();
                    return Ok(v);
                }
                Instruction::Perform(idx, argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    let name = self.effects.get(idx as usize).cloned().unwrap_or_default();
                    let r = perform_effect_internal(self, name, args)?;
                    if let Some(val) = r {
                        self.push(val)
                    }
                }
                Instruction::TypeOf => {
                    let v = self.pop()?;
                    let tid = match v.tag {
                        ValueTag::Int => 0i64,
                        ValueTag::Float => 1,
                        ValueTag::Bool => 2,
                        ValueTag::Null => 3,
                        _ => 4,
                    };
                    self.push(Value::int(tid));
                }
                Instruction::Halt => break,
                _ => {}
            }
            if let Some(next) = next_ip {
                self.frames.last_mut().unwrap().ip = next;
            }
        }
        Ok(Value::null())
    }
}
