use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_quote(&mut self, idx: u32) -> Result<Option<usize>, VmError> {
        // Quote converts a chunk of code into a Value::Code.
        // idx is the chunk index in the current module.
        let frame = self.frames.last().ok_or(VmError::RuntimeError("No frame".to_string()))?;
        let module_idx = frame.module_idx;
        
        let code = Value::code(module_idx, idx as usize, &self.gc);
        self.push(code)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_splice(&mut self) -> Result<Option<usize>, VmError> {
        // Splice takes a Value and injects it into the current quote context.
        // This is complex and usually handled during macro expansion or quote evaluation.
        // For now, it's a no-op that pops the value.
        let _val = self.pop()?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_eval(&mut self, _argc: u8) -> Result<Option<usize>, VmError> {
        // Eval takes a Value (code/AST) and executes it.
        let val = self.pop()?;
        if val.tag() == crate::vm::value::ValueTag::Code {
            let code = unsafe {
                let ptr = val.payload() as *const crate::vm::value::GcBox<crate::vm::value::Code>;
                &(*ptr).data
            };
            
            let module_idx = code.module_idx;
            let chunk_idx = code.chunk_idx;
            
            let instrs = self.get_chunk_instructions(module_idx, chunk_idx)?;
            let chunk = &self.modules[module_idx].chunks[chunk_idx];
            
            let mut locals = vec![Value::null(); chunk.locals as usize];
            // If we have argc, we might want to pass arguments to the eval'd code.
            // For now, let's just use empty locals or nulls.
            
            let new_frame = crate::vm::core::Frame {
                instrs,
                ip: 0,
                locals,
                closure: Value::null(),
                module_idx,
                chunk_idx: Some(chunk_idx),
            };
            
            self.frames.push(new_frame);
            Ok(Some(0))
        } else {
            Err(VmError::RuntimeError("Eval requires a Code object".to_string()))
        }
    }

    #[inline(always)]
    pub fn execute_expand_macro(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        // Expand a macro: call chunk at idx with argc arguments.
        // In some systems, macros are executed in a separate phase, but here
        // they can be executed at runtime.
        self.execute_call(idx, argc as u16, module_idx)
    }
}
