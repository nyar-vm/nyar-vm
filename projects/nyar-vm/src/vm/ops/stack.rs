use crate::bytecode::decoder::Instruction;
use crate::bytecode::format::Constant;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_stack_op(&mut self, ins: Instruction, module_idx: usize) -> Result<(), VmError> {
        match ins {
            Instruction::Push(idx) => {
                let c = self.modules[module_idx]
                    .constants
                    .get(idx as usize)
                    .ok_or(VmError::IndexOutOfBounds)?;
                match c {
                    Constant::Int(i) => self.push(Value::int(*i)),
                    Constant::Float(x) => self.push(Value::float(*x)),
                    Constant::String(s) => self.push(Value::string(s.clone(), &self.gc)),
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
                let sp = self.sp;
                if sp == 0 {
                    return Err(VmError::StackUnderflow);
                }
                let eff = if (d as usize) >= sp {
                    sp - 1
                } else {
                    d as usize
                };
                self.swap_with(eff)?;
            }
            Instruction::LoadLocal(idx) => {
                let f = self.frames.last().unwrap();
                if (idx as usize) < f.locals.len() {
                    let v = f.locals[idx as usize];
                    self.push(v);
                } else {
                    return Err(VmError::StackUnderflow);
                }
            }
            Instruction::StoreLocal(idx) => {
                let v = self.pop()?;
                let f = self.frames.last_mut().unwrap();
                if (idx as usize) >= f.locals.len() {
                    f.locals.resize((idx as usize) + 1, Value::null());
                }
                f.locals[idx as usize] = v;
            }
            Instruction::LoadGlobal(name_idx) => {
                let module = &self.modules[module_idx];
                let name = match module.constants.get(name_idx as usize) {
                    Some(Constant::String(s)) => s,
                    _ => return Err(VmError::InvalidOpcode),
                };
                if let Some(v) = self.builtins.get(name) {
                    self.push(*v);
                } else if let Some(&(_m_idx, _c_idx)) = self.symbol_table.get(name) {
                    self.push(Value::null());
                } else {
                    return Err(VmError::RuntimeError(format!("Global not found: {}", name)));
                }
            }
            Instruction::StoreGlobal(name_idx) => {
                let v = self.pop()?;
                let module = &self.modules[module_idx];
                let name = match module.constants.get(name_idx as usize) {
                    Some(Constant::String(s)) => s.clone(),
                    _ => return Err(VmError::InvalidOpcode),
                };
                self.builtins.insert(name, v);
            }
            _ => Err(VmError::InvalidOpcode),
        }
        Ok(())
    }
}
