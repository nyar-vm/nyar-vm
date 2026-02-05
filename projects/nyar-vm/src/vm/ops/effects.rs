use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Frame};
use crate::vm::NyarError;
use crate::bytecode::format::Constant;
use crate::vm::effects::perform_effect_internal;
use nyar_types::{QualifiedName, EffectInfo, SourceLocation};

impl NyarVM {
    #[inline(always)]
    pub fn execute_perform(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let name = match self.modules[module_idx].constants.get(idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
            _ => return Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize))),
        };

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let current_frame = self.frames.last().ok_or_else(|| self.error(nyar_types::VmErrorKind::NoActiveFrame))?;
        let effect_info = EffectInfo {
            name: name.clone(),
            location: SourceLocation {
                source_id: module_idx as u32,
                offset: current_frame.ip as u32,
            },
        };

        /*
        /*
        #[cfg(debug_assertions)]
        self.log(&format!(
            "[Effect] Perform {} with {} args at {}",
            name, argc, effect_info.location
        ));
        */
        */

        // 1. Check for dynamic handler
        if let Some(handler) = self.handler_stack.pop() {
            // Capture continuation. The current instruction is 'perform', 
            // so we want the continuation to resume at the NEXT instruction.
            let mut captured_frames = self.frames.clone();
            if let Some(f) = captured_frames.last_mut() {
                f.ip += 1;
            }
            let cont = Value::continuation(0, self.stack[..self.sp].to_vec(), captured_frames, &self.gc);
            
            // Unwind to handler depth
            self.frames.truncate(handler.frame_depth);
            
            // Push handler frame
            let handler_module_idx = handler.module_idx;
            let instrs = self.get_chunk_instructions(handler_module_idx, handler.catch_chunk)?;
            let chunk = &self.modules[handler_module_idx].chunks[handler.catch_chunk];
            let locals_count = chunk.locals as usize;
            
            let effect_obj = Value::effect(effect_info, args.clone(), &self.gc);
            let args_list = Value::list(args, &self.gc);
            
            let mut locals = vec![Value::null(); locals_count];
            if locals_count >= 1 {
                locals[0] = effect_obj;
            }
            if locals_count >= 2 {
                locals[1] = args_list;
            }
            if locals_count >= 3 {
                locals[2] = cont;
            }
            
            let new_frame = Frame {
                instrs,
                ip: 0,
                locals,
                upvalues: vec![None; locals_count],
                closure: Value::null(),
                module_idx: handler_module_idx,
                chunk_idx: Some(handler.catch_chunk),
                location: Default::default(),
            };
            self.frames.push(new_frame);
            
            // Push effect object to the stack for MatchEffect
            self.push(effect_obj)?;
            
            return Ok(Some(0));
        }

        // 2. Fallback to internal/builtin effects
        let result = perform_effect_internal(self, module_idx, effect_info, args)?;
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
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        // idx is the chunk index for the handler
        let frame_depth = self.frames.len();
        self.handler_stack.push(crate::vm::effects::HandlerFrame {
            module_idx,
            catch_chunk: idx as usize,
            frame_depth,
        });
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_resume_with(&mut self) -> Result<Option<usize>, NyarError> {
        // Pop the value to resume with and the continuation
        let val = self.pop()?;
        let cont_val = self.pop()?;
        self.execute_resume(cont_val, val)
    }

    pub fn execute_resume(&mut self, cont_val: Value, val: Value) -> Result<Option<usize>, NyarError> {
        let cont = cont_val
            .try_as_continuation()
            .ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidContinuation))?;

        // Restore frames and stack from the continuation.
        self.frames = cont.frames.clone();
        self.stack = cont.stack_slice.clone();
        self.sp = self.stack.len();

        // Push the resumed value as the result of the 'perform' instruction.
        self.push(val)?;

        // Return the instruction pointer where we should resume execution.
        // The continuation's last frame IP already points to the next instruction
        // because we incremented it during capture in execute_perform.
        Ok(Some(cont.frames.last().map(|f| f.ip).unwrap_or(0)))
    }

    #[inline(always)]
    pub fn execute_yield(&mut self) -> Result<Option<usize>, NyarError> {
        Err(self.error(nyar_types::VmErrorKind::YieldAsync))
    }

    pub fn execute_capture_cont(&mut self) -> Result<Option<usize>, NyarError> {
        let frame = self.frames.last().ok_or_else(|| self.error(nyar_types::VmErrorKind::NoActiveFrame))?;
        let cont = Value::continuation(
            frame.ip,
            self.stack[..self.sp].to_vec(),
            self.frames.clone(),
            &self.gc,
        );
        self.push(cont)?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_match_effect(&mut self, idx: u16, module_idx: usize) -> Result<Option<usize>, NyarError> {
        // The effect object is on the stack, pushed by the VM during perform dispatch
        // OR it's in locals[0] of the handler frame if we are using the new logic.
        // Let's check the stack first, as it's more direct for the MatchEffect opcode.
        let val = self.peek_at(0)?;
        
        let target_name = match self.modules[module_idx].constants.get(idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
            _ => return Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize))),
        };

        if let Some(effect) = val.try_as_effect() {
            if effect.info.name == target_name {
                // Match! Pop the effect, push arguments and then true
                self.pop()?;
                for arg in &effect.args {
                    self.push(*arg)?;
                }
                self.push(Value::bool(true))?;
            } else {
                // No match. Keep the effect on stack, push false
                self.push(Value::bool(false))?;
            }
        } else {
            // Not an effect.
            self.push(Value::bool(false))?;
        }
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_await(&mut self) -> Result<Option<usize>, NyarError> {
        let val = self.pop()?;
        if let Some(future) = val.try_as_future() {
            match future.status {
                crate::vm::value::FutureStatus::Ready => {
                    self.push(future.result)?;
                    Ok(None)
                }
                crate::vm::value::FutureStatus::Failed => {
                    Err(self.error(nyar_types::VmErrorKind::FutureFailed))
                }
                crate::vm::value::FutureStatus::Pending => {
                    // Push the future back and yield
                    self.push(val)?;
                    Err(self.error(nyar_types::VmErrorKind::YieldAsync))
                }
            }
        } else if val.is_closure() {
            self.push(val)?;
            self.execute_call_closure(0)
        } else {
            // Not a future, just treat as ready
            self.push(val)?;
            Ok(None)
        }
    }

    #[inline(always)]
    pub fn execute_block_on(&mut self) -> Result<Option<usize>, NyarError> {
        // Pop the future to block on.
        let future_val = self.pop()?;

        if future_val.is_closure() {
            let res = self.call_closure_sync(future_val, vec![])?;
            self.push(res)?;
            return Ok(None);
        }

        if !future_val.is_future() {
            // Not a future, just push it back and continue.
            self.push(future_val)?;
            return Ok(None);
        }

        // Push the future back to the stack so it's rooted during VM execution.
        self.push(future_val)?;

        loop {
            // Re-acquire future status. We use unsafe to get a reference to the GC data.
            // Since future_val is on the stack, it's safe from GC.
            let status = unsafe { self.stack[self.sp - 1].as_future().status };

            match status {
                crate::vm::value::FutureStatus::Ready => {
                    let future = unsafe { self.stack[self.sp - 1].as_future() };
                    let res = future.result;
                    self.pop()?; // pop the future
                    self.push(res)?; // push the result
                    return Ok(None);
                }
                crate::vm::value::FutureStatus::Failed => {
                    return Err(self.error(nyar_types::VmErrorKind::FutureFailed));
                }
                crate::vm::value::FutureStatus::Pending => {
                    // Step the VM
                    match self.execute_step() {
                        Ok(Some(_)) => {
                            // Instruction executed
                            continue;
                        }
                        Ok(None) => {
                            // VM halted
                            break Ok(None);
                        }
                        Err(e) => {
                            // Check if it's a yield
                            match *e.kind {
                            nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::YieldAsync) => {
                                // The task yielded.
                                std::thread::yield_now();
                                continue;
                            }
                            _ => return Err(e),
                        }
                        }
                    }
                }
            }
        }
    }
}
