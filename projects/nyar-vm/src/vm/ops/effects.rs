use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Frame};
use crate::vm::VmError;
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
    ) -> Result<Option<usize>, VmError> {
        let name = match self.modules[module_idx].constants.get(idx as usize) {
            Some(Constant::QualifiedName(qn)) => qn.clone(),
            Some(Constant::String(s)) => QualifiedName::new(s.split("::").map(|s| s.to_string()).collect()),
            _ => return Err(VmError::IndexOutOfBounds),
        };

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let current_frame = self.frames.last().ok_or(VmError::RuntimeError("No frame".to_string()))?;
        let effect_info = EffectInfo {
            name: name.clone(),
            location: SourceLocation {
                source_id: module_idx as u32,
                offset: current_frame.ip as u32,
            },
        };

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
            let locals_count = chunk.locals as usize;
            let locals = vec![Value::null(); locals_count];
            
            let new_frame = Frame {
                instrs,
                ip: 0,
                locals,
                upvalues: vec![None; locals_count],
                closure: Value::null(),
                module_idx,
                chunk_idx: Some(handler.catch_chunk),
            };
            self.frames.push(new_frame);
            
            // Push arguments and continuation to handler
            self.push(Value::qualified_name(name, &self.gc))?;
            let args_val = Value::array(args, &self.gc);
            self.push(args_val)?;
            self.push(cont)?;
            
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

        let cont = cont_val
            .try_as_continuation()
            .ok_or(VmError::RuntimeError("Resume requires a continuation".to_string()))?;

        // Restore frames and stack from the continuation.
        // The stack and frames are roots, so no write barrier is required here.
        self.frames = cont.frames.clone();
        self.stack = cont.stack_slice.clone();
        self.sp = self.stack.len();

        // Push the resumed value as the result of the 'perform' instruction.
        // Again, pushing to the stack (a root) does not require a write barrier.
        self.push(val)?;

        Ok(Some(cont.ip))
    }

    #[inline(always)]
    pub fn execute_capture_cont(&mut self) -> Result<Option<usize>, VmError> {
        let frame = self.frames.last().ok_or(VmError::RuntimeError("No frame".to_string()))?;
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
    pub fn execute_match_effect(&mut self, idx: u16) -> Result<Option<usize>, VmError> {
        // Pop an effect object and check if it matches the type_idx
        let val = self.pop()?;
        if let Some(effect) = val.try_as_effect() {
            if effect.type_idx == idx {
                // Match! Push arguments and then true
                for arg in &effect.args {
                    self.push(*arg)?;
                }
                self.push(Value::bool(true))?;
            } else {
                // No match. Push the effect back and then false
                self.push(val)?;
                self.push(Value::bool(false))?;
            }
        } else {
            // Not an effect.
            self.push(val)?;
            self.push(Value::bool(false))?;
        }
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_await(&mut self) -> Result<Option<usize>, VmError> {
        let val = self.pop()?;
        if let Some(future) = val.try_as_future() {
            match future.status {
                crate::vm::value::FutureStatus::Ready => {
                    self.push(future.result)?;
                    Ok(None)
                }
                crate::vm::value::FutureStatus::Failed => {
                    Err(VmError::RuntimeError(format!("Future failed: {}", future.result)))
                }
                crate::vm::value::FutureStatus::Pending => {
                    // Push the future back and yield
                    self.push(val)?;
                    Err(VmError::YieldAsync)
                }
            }
        } else {
            // Not a future, just treat as ready
            self.push(val)?;
            Ok(None)
        }
    }

    #[inline(always)]
    pub fn execute_block_on(&mut self) -> Result<Option<usize>, VmError> {
        // Pop the future to block on.
        let future_val = self.pop()?;

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
                    let future = unsafe { self.stack[self.sp - 1].as_future() };
                    let res = future.result;
                    return Err(VmError::RuntimeError(format!("Future failed: {}", res)));
                }
                crate::vm::value::FutureStatus::Pending => {
                    // Drive the VM by one step.
                    match self.execute_step() {
                        Ok(Some(())) => continue,
                        Ok(None) => {
                            // VM has no more instructions to execute in the current frames,
                            // but the future is still pending. This might be a deadlock
                            // or waiting for external I/O.
                            std::thread::yield_now();
                            continue;
                        }
                        Err(VmError::YieldAsync) => {
                            // The task yielded.
                            std::thread::yield_now();
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }
}
