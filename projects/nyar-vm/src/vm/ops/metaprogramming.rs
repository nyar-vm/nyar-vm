use crate::vm::value::{Value, Frame};
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
        // Splice takes a value and returns it. In a more advanced implementation,
        // this might involve code generation or AST manipulation.
        // For now, it just ensures the value on stack is treated as part of the current execution.
        let val = self.pop()?;
        self.push(val)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_eval(&mut self, argc: u8) -> Result<Option<usize>, VmError> {
        // Eval takes a Value (code/AST) and executes it with optional arguments.
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
            
            // Pop argc arguments from stack and put into locals
            let mut args = Vec::with_capacity(argc as usize);
            for _ in 0..argc {
                args.push(self.pop()?);
            }
            args.reverse();
            
            for (i, arg) in args.into_iter().enumerate() {
                if i < locals.len() {
                    locals[i] = arg;
                }
            }
            
            let new_frame = Frame {
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
