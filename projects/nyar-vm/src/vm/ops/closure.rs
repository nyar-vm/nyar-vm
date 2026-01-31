use crate::bytecode::instruction::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::{Upvalue, Value};
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_closure_op(
        &mut self,
        ins: Instruction,
        module_idx: usize,
    ) -> Result<(), VmError> {
        match ins {
            Instruction::MakeClosure(idx, upvalues) => {
                let mut captured = Vec::with_capacity(upvalues.len());
                for up in upvalues {
                    let val = if up.is_local {
                        let f = self.frames.last().unwrap();
                        f.locals[up.index as usize]
                    } else {
                        let f = self.frames.last().unwrap();
                        let closure = f.closure.try_as_closure().ok_or(VmError::InvalidOpcode)?;
                        closure.upvalues[up.index as usize].0
                    };
                    captured.push(Upvalue(val));
                }
                let v = Value::closure(module_idx, idx, captured, &self.gc);
                self.push(v)?;
            }
            Instruction::LoadUpvalue(idx) => {
                let f = self.frames.last().unwrap();
                let closure = f.closure.try_as_closure().ok_or(VmError::InvalidOpcode)?;
                if (idx as usize) < closure.upvalues.len() {
                    self.push(closure.upvalues[idx as usize].0)?;
                } else {
                    return Err(VmError::IndexOutOfBounds);
                }
            }
            Instruction::StoreUpvalue(idx) => {
                let val = self.pop()?;
                let gc = &self.gc;
                let f = self.frames.last().unwrap();
                let closure = f
                    .closure
                    .try_as_closure_mut()
                    .ok_or(VmError::InvalidOpcode)?;
                if (idx as usize) < closure.upvalues.len() {
                    closure.upvalues[idx as usize].0 = val;
                    val.write_barrier(gc);
                } else {
                    return Err(VmError::IndexOutOfBounds);
                }
            }
            _ => return Err(VmError::InvalidOpcode),
        }
        Ok(())
    }
}
