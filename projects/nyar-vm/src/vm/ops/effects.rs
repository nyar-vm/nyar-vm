use crate::vm::value::{Value, Frame};
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

        // 1. Check for dynamic handler
        if let Some(handler) = self.handler_stack.pop() {
            // Capture continuation
            let frame = self.frames.last().ok_or(VmError::RuntimeError("No frame".to_string()))?;
            let cont = Value::continuation(frame.ip, self.stack[..self.sp].to_vec(), self.frames.clone(), &self.gc);
            
            // Unwind to handler depth
            self.frames.truncate(handler.frame_depth);
            
            // Push handler frame
            let instrs = self.get_chunk_instructions(module_idx, handler.catch_chunk)?;
            let chunk = &self.modules[module_idx].chunks[handler.catch_chunk];
            let locals = vec![Value::null(); chunk.locals as usize];
            
            let new_frame = Frame {
                instrs,
                ip: 0,
                locals,
                closure: Value::null(),
                module_idx,
                chunk_idx: Some(handler.catch_chunk),
            };
            self.frames.push(new_frame);
            
            // Push arguments and continuation to handler
            self.push(Value::string(name, &self.gc))?;
            let args_val = Value::array(args, &self.gc);
            self.push(args_val)?;
            self.push(cont)?;
            
            return Ok(Some(0));
        }

        // 2. Fallback to internal/builtin effects
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
        // Pop the value to resume with and the continuation
        let val = self.pop()?;
        let cont_val = self.pop()?;
        
        let cont = cont_val.try_as_continuation().ok_or(VmError::RuntimeError("Resume requires a continuation".to_string()))?;
        
        // Restore frames and stack
        self.frames = cont.frames.clone();
        self.stack = cont.stack_slice.clone();
        self.sp = self.stack.len();
        
        // Push the resumed value as the result of the 'perform' instruction
        self.push(val)?;
        
        Ok(Some(cont.ip))
    }

    #[inline(always)]
    pub fn execute_capture_cont(&mut self) -> Result<Option<usize>, VmError> {
        // Capture the current stack, frames and IP as a continuation
        let frame = self.frames.last().ok_or(VmError::RuntimeError("No frame".to_string()))?;
        let cont = Value::continuation(frame.ip, self.stack[..self.sp].to_vec(), self.frames.clone(), &self.gc);
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
