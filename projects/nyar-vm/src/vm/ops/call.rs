use crate::bytecode::format::Constant;
use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Frame};
use crate::vm::VmError;

use nyar_types::QualifiedName;

impl NyarVM {
    #[inline(always)]
    pub fn execute_call(
        &mut self,
        chunk_idx: u16,
        argc: u16,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let instrs = self.get_chunk_instructions(module_idx, chunk_idx as usize)?;
        let locals_count = self.modules[module_idx].chunks[chunk_idx as usize].locals as usize;

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        if args.len() < locals_count {
            args.resize(locals_count, Value::null());
        }

        let new_frame = Frame {
            instrs,
            ip: 0,
            locals: args,
            upvalues: vec![None; locals_count],
            closure: Value::null(),
            module_idx,
            chunk_idx: Some(chunk_idx as usize),
        };

        self.frames.push(new_frame);
        Ok(Some(0))
    }

    #[inline(always)]
    pub fn execute_call_closure(&mut self, argc: u16) -> Result<Option<usize>, VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let callee = self.pop()?;
        let (instrs, locals_count, c_module_idx, c_chunk_idx) =
            if let Some(closure) = callee.try_as_closure() {
                let chunk_idx = closure.func;
                let instrs = self.get_chunk_instructions(closure.module_idx, chunk_idx)?;
                let locals_count =
                    self.modules[closure.module_idx].chunks[chunk_idx].locals as usize;
                (instrs, locals_count, closure.module_idx, chunk_idx)
            } else {
                return Err(VmError::InvalidOpcode);
            };

        if args.len() < locals_count {
            args.resize(locals_count, Value::null());
        }

        let new_frame = Frame {
            instrs,
            ip: 0,
            locals: args,
            upvalues: vec![None; locals_count],
            closure: callee,
            module_idx: c_module_idx,
            chunk_idx: Some(c_chunk_idx),
        };

        self.frames.push(new_frame);
        Ok(Some(0))
    }

    #[inline(always)]
    pub fn execute_call_symbol(
        &mut self,
        name_idx: u16,
        argc: u16,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let name = match self.modules[module_idx].constants.get(name_idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => {
                QualifiedName::new(s.split("::").map(|s| s.to_string()).collect())
            }
            _ => return Err(VmError::IndexOutOfBounds),
        };

        if let Some(&(m_idx, chunk_idx)) = self.symbol_table.get(&name) {
            let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;
            let locals_count = self.modules[m_idx].chunks[chunk_idx as usize].locals as usize;

            let mut args = Vec::with_capacity(argc as usize);
            for _ in 0..argc {
                args.push(self.pop()?);
            }
            args.reverse();

            if args.len() < locals_count {
                args.resize(locals_count, Value::null());
            }

            let new_frame = Frame {
                instrs,
                ip: 0,
                locals: args,
                upvalues: vec![None; locals_count],
                closure: Value::null(),
                module_idx: m_idx,
                chunk_idx: Some(chunk_idx as usize),
            };

            self.frames.push(new_frame);
            Ok(Some(0))
        } else {
            Err(VmError::RuntimeError(format!("Symbol not found: {}", name)))
        }
    }

    #[inline(always)]
    pub fn execute_invoke_method(
        &mut self,
        name_idx: u16,
        argc: u16,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let receiver = self.pop()?;
        let name = match self.modules[module_idx].constants.get(name_idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => {
                QualifiedName::new(s.split("::").map(|s| s.to_string()).collect())
            }
            _ => return Err(VmError::InvalidOpcode),
        };

        if !receiver.is_object() {
            self.invoke_primitive_method(receiver, &name, args)?;
        } else {
            // Object method invocation logic...
            // For now, let's just push null as a placeholder if not handled
            self.push(Value::null())?;
        }
        Ok(None)
    }

    fn invoke_primitive_method(
        &mut self,
        receiver: Value,
        name: &QualifiedName,
        args: Vec<Value>,
    ) -> Result<(), VmError> {
        let name_str = name.to_string();
        match name_str.as_str() {
            "println" => {
                for arg in args {
                    self.print_line(&arg.to_string());
                }
                self.push(Value::null())?;
            }
            "add" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        self.push(Value::int(l + r))?;
                    } else if let (Some(l), Some(r)) = (receiver.try_as_float(), rhs.try_as_float())
                    {
                        self.push(Value::float(l + r))?;
                    } else if let (Some(l), Some(r)) = (receiver.try_as_str(), rhs.try_as_str()) {
                        let mut s = l.to_string();
                        s.push_str(r);
                        self.push(Value::string(s, &self.gc))?;
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            _ => {
                self.push(Value::null())?;
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub fn execute_tail_call(&mut self, argc: u8) -> Result<Option<usize>, VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let callee = self.pop()?;
        let (instrs, locals_count, c_module_idx, c_chunk_idx) =
            if let Some(closure) = callee.try_as_closure() {
                let chunk_idx = closure.func;
                let instrs = self.get_chunk_instructions(closure.module_idx, chunk_idx)?;
                let locals_count =
                    self.modules[closure.module_idx].chunks[chunk_idx].locals as usize;
                (instrs, locals_count, closure.module_idx, chunk_idx)
            } else {
                return Err(VmError::InvalidOpcode);
            };

        if args.len() < locals_count {
            args.resize(locals_count, Value::null());
        }

        // Reuse the current frame
        if let Some(frame) = self.frames.last_mut() {
            frame.instrs = instrs;
            frame.ip = 0;
            frame.locals = args;
            frame.closure = callee;
            frame.module_idx = c_module_idx;
            frame.chunk_idx = Some(c_chunk_idx);
        }
        
        Ok(Some(0))
    }

    #[inline(always)]
    pub fn execute_call_virtual(
        &mut self,
        idx: u16,
        argc: u8,
        _module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let trait_val = self.pop()?;
        let trait_obj = trait_val.try_as_trait_object().ok_or(VmError::InvalidOpcode)?;
        
        let witness_val = trait_obj.witness;
        let witness = unsafe { witness_val.as_witness_table() };
        
        let chunk_idx = witness.methods.get(idx as usize).ok_or(VmError::IndexOutOfBounds)?;
        let target_module_idx = witness.module_idx;

        // The first argument to a trait method is usually the data (self)
        let mut final_args = Vec::with_capacity(argc as usize + 1);
        final_args.push(trait_obj.data);
        final_args.extend(args);

        let instrs = self.get_chunk_instructions(target_module_idx, *chunk_idx as usize)?;
        let locals_count = self.modules[target_module_idx].chunks[*chunk_idx as usize].locals as usize;

        if final_args.len() < locals_count {
            final_args.resize(locals_count, Value::null());
        }

        let new_frame = Frame {
            instrs,
            ip: 0,
            locals: final_args,
            upvalues: vec![None; locals_count],
            closure: Value::null(),
            module_idx: target_module_idx,
            chunk_idx: Some(*chunk_idx as usize),
        };

        self.frames.push(new_frame);
        Ok(Some(0))
    }

    #[inline(always)]
    pub fn execute_call_dynamic(
        &mut self,
        _idx: u16,
        argc: u8,
        _module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        // Dynamic call logic: receiver is on stack, method name is on stack
        // Pop method name, pop receiver, find method, call it.
        let method_name = self.pop()?;
        let receiver = self.pop()?;
        
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // Placeholder: try to invoke as a method
        if let Some(name_str) = method_name.try_as_str() {
            let name = QualifiedName::new(name_str.split("::").map(|s| s.to_string()).collect());
            self.invoke_primitive_method(receiver, &name, args)?;
        } else {
            return Err(VmError::InvalidOpcode);
        }
        
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_ffi_call(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        let name_qn = match self.modules[module_idx].constants.get(idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => {
                QualifiedName::new(s.split("::").map(|s| s.to_string()).collect())
            }
            _ => return Err(VmError::IndexOutOfBounds),
        };
        let name = name_qn.to_string();

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // Check in FFI registry
        if let Some(func) = self.ffi.get(&name) {
            // Validate signature if present
            if let Some(sig) = func.signature() {
                if sig.params.len() != args.len() {
                    return Err(VmError::RuntimeError(format!(
                        "FFI function {} expects {} arguments, got {}",
                        name,
                        sig.params.len(),
                        args.len()
                    )));
                }
                for (i, (arg, ty)) in args.iter().zip(sig.params.iter()).enumerate() {
                    let matches = match ty {
                        crate::vm::ffi::FFIType::Null => arg.is_null(),
                        crate::vm::ffi::FFIType::Int => arg.is_int(),
                        crate::vm::ffi::FFIType::Float => arg.is_float(),
                        crate::vm::ffi::FFIType::Bool => arg.is_bool(),
                        crate::vm::ffi::FFIType::String => arg.is_string(),
                        crate::vm::ffi::FFIType::List => arg.is_list(),
                        crate::vm::ffi::FFIType::Object => arg.is_object(),
                        crate::vm::ffi::FFIType::Any => true,
                    };
                    if !matches {
                        return Err(VmError::RuntimeError(format!(
                            "FFI function {} argument {} type mismatch (expected {:?}, got {:?})",
                            name, i, ty, arg.tag()
                        )));
                    }
                }
            }

            let result = func.call(args)?;
            self.push(result)?;
        } else {
            // If not found in FFI, maybe it's a builtin?
            if let Some(val) = self.builtins.get(&name_qn).cloned() {
                // If it's a closure/function, we should probably call it, 
                // but FFICall usually implies direct native call.
                // For now, return error if not a native function.
                return Err(VmError::RuntimeError(format!("FFI function not found: {}", name)));
            }
            return Err(VmError::RuntimeError(format!("FFI function not found: {}", name)));
        }

        Ok(None)
    }
}
