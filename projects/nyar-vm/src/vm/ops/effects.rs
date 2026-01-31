use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;
use crate::bytecode::format::Constant;
use crate::vm::effects::perform_effect_internal;

impl NyarVM {
    #[inline(always)]
    pub fn execute_perform(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let name = match self.modules[module_idx].constants.get(idx as usize) {
            Some(Constant::String(s)) => s.clone(),
            _ => return Err(VmError::IndexOutOfBounds),
        };

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let result = perform_effect_internal(self, module_idx, name, args)?;
        if let Some(val) = result {
            self.push(val)?;
        } else {
            self.push(Value::null())?;
        }
        
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_with_handler(
        &mut self,
        idx: u16,
        _module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        // idx is the chunk index for the handler
        let frame_depth = self.frames.len();
        self.handler_stack.push(crate::vm::effects::HandlerFrame {
            catch_chunk: idx as usize,
            frame_depth,
        });
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_resume_with(&mut self) -> Result<Option<usize>, VmError> {
        // resume with value
        let val = self.pop()?;
        // In a real implementation, we would restore the continuation
        // For now, just push the value back and return
        self.push(val)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_capture_cont(&mut self) -> Result<Option<usize>, VmError> {
        // Capture the current stack and IP as a continuation
        let frame = self.frames.last().ok_or(VmError::RuntimeError("No frame".to_string()))?;
        let cont = Value::continuation(frame.ip, self.stack[..self.sp].to_vec(), &self.gc);
        self.push(cont)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_await(&mut self) -> Result<Option<usize>, VmError> {
        // Pop a value (presumably a future/promise) and await it.
        // For now, it's a no-op that assumes the value is already resolved.
        let _val = self.pop()?;
        self.push(_val)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_block_on(&mut self) -> Result<Option<usize>, VmError> {
        // Similar to await but blocking.
        let _val = self.pop()?;
        self.push(_val)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_match_effect(&mut self, _idx: u16) -> Result<Option<usize>, VmError> {
        // Used in effect handlers to check if the effect matches.
        // For now, return false (0) as a placeholder.
        self.push(Value::bool(false))?;
        Ok(None)
    }
}
