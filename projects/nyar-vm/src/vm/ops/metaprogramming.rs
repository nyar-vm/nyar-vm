use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Frame};
use crate::vm::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_quote(&mut self, idx: u32) -> Result<Option<usize>, NyarError> {
        // Quote converts a chunk of code into a Value::Code.
        // idx is the chunk index in the current module.
        let frame = self.frames.last().ok_or_else(|| self.error(nyar_types::VmErrorKind::NoActiveFrame))?;
        let module_idx = frame.module_idx;
        
        let code = Value::code(module_idx, idx as usize, &self.gc);
        self.push(code)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_splice(&mut self) -> Result<Option<usize>, NyarError> {
        // Splice takes a value and returns it. In a more advanced implementation,
        // this might involve code generation or AST manipulation.
        // For now, it just ensures the value on stack is treated as part of the current execution.
        let val = self.pop()?;
        self.push(val)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_eval(&mut self, argc: u8) -> Result<Option<usize>, NyarError> {
        // Eval takes a Value (code/AST) and executes it with optional arguments.
        let val = self.pop()?;
        if val.tag() == crate::vm::value::ValueTag::Code {
            let code = unsafe {
                let ptr = val.payload() as *const nyar_gc::GcBox<crate::vm::value::Code>;
                &(*ptr).data
            };
            
            let module_idx = code.module_idx;
            let chunk_idx = code.chunk_idx;

            let instrs = self.get_chunk_instructions(module_idx, chunk_idx)?;
            let locals_count = {
                let module = self.get_module(module_idx);
                module.chunks[chunk_idx].locals as usize
            };

            let mut locals = vec![Value::null(); locals_count];
            
            // Pop argc arguments from stack and put into locals
            if argc as usize > locals_count {
                return Err(self.error(nyar_types::VmErrorKind::LimitExceeded));
            }

            let mut args = Vec::with_capacity(argc as usize);
            for _ in 0..argc {
                args.push(self.pop()?);
            }
            args.reverse();
            
            for (i, arg) in args.into_iter().enumerate() {
                locals[i] = arg;
            }
            
            let new_frame = Frame {
                instrs,
                ip: 0,
                locals,
                upvalues: vec![None; locals_count],
                closure: Value::null(),
                module_idx,
                chunk_idx: Some(chunk_idx),
                location: Default::default(),
            };
            
            self.frames.push(new_frame);
            Ok(Some(0))
        } else {
            Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Code".to_string(), found: format!("{:?}", val.tag()) }))
        }
    }

    #[inline(always)]
    pub fn execute_expand_macro(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        // Expand a macro: call chunk at idx with argc arguments.
        // In some systems, macros are executed in a separate phase, but here
        // they can be executed at runtime.
        self.execute_call(idx, argc as u16, module_idx)
    }
}
