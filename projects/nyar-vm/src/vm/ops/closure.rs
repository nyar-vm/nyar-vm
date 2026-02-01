use crate::vm::core::NyarVM;
use crate::vm::value::{Upvalue, Value};
use crate::vm::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_make_closure(
        &mut self,
        idx: u16,
        upvalues: Vec<crate::bytecode::instruction::UpvalueRef>,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let mut captured = Vec::with_capacity(upvalues.len());
        for up in upvalues {
            let upvalue = if up.is_local {
                let f = self.frames.last_mut().unwrap();
                let index = up.index as usize;
                if let Some(existing) = f.upvalues.get(index).and_then(|x| x.as_ref()) {
                    existing.clone()
                } else {
                    let new_up = Upvalue::new(f.locals[index]);
                    if index >= f.upvalues.len() {
                        f.upvalues.resize(index + 1, None);
                    }
                    f.upvalues[index] = Some(new_up.clone());
                    new_up
                }
            } else {
                let f = self.frames.last().unwrap();
                let closure = f.closure.try_as_closure().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x15)))?;
                closure.upvalues[up.index as usize].clone()
            };
            captured.push(upvalue);
        }
        let v = Value::closure(module_idx, idx, captured, &self.gc);
        self.push(v)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_load_upvalue(&mut self, idx: u16) -> Result<Option<usize>, NyarError> {
        let f = self.frames.last().unwrap();
        let closure = f.closure.try_as_closure().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x09)))?;
        if (idx as usize) < closure.upvalues.len() {
            self.push(closure.upvalues[idx as usize].get())?;
            Ok(None)
        } else {
            Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize)))
        }
    }

    #[inline(always)]
    pub fn execute_store_upvalue(&mut self, idx: u16) -> Result<Option<usize>, NyarError> {
        let val = self.pop()?;
        let gc = &self.gc;
        let f = self.frames.last().unwrap();
        let closure = f.closure.try_as_closure().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x0A)))?;
        if (idx as usize) < closure.upvalues.len() {
            closure.upvalues[idx as usize].set(val);
            val.write_barrier(gc);
            Ok(None)
        } else {
            Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize)))
        }
    }

    #[inline(always)]
    pub fn execute_close_upvalues(&mut self) -> Result<Option<usize>, NyarError> {
        // In our current implementation using Arc<AtomicU64>,
        // "closing" an upvalue means it's no longer tracked in the current frame's
        // open upvalues list. The actual value is already in the Upvalue's AtomicU64.
        // When the frame is popped, these upvalues will be naturally closed.
        // However, if the instruction is meant to close upvalues within a scope
        // (e.g. at the end of a block), we clear the open upvalues list.
        let f = self.frames.last_mut().unwrap();
        f.upvalues.clear();
        Ok(None)
    }
}

