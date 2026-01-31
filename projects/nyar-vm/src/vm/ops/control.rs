use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_control_op(
        &mut self,
        ins: Instruction,
        cur_ip: usize,
    ) -> Result<Option<usize>, VmError> {
        match ins {
            Instruction::Jump(off) => {
                let target = (cur_ip as isize + off as isize) as usize;
                if off < 0 {
                    self.handle_backedge(target)?;
                }
                Ok(Some(target))
            }
            Instruction::JumpIfFalse(off) => {
                let v = self.pop()?;
                if !v.is_truthy() {
                    let target = (cur_ip as isize + off as isize) as usize;
                    if off < 0 {
                        self.handle_backedge(target)?;
                    }
                    Ok(Some(target))
                } else {
                    Ok(Some(cur_ip + 1))
                }
            }
            Instruction::Return => {
                let val = self.pop()?;
                self.frames.pop();
                while let Some(hf) = self.handler_stack.last() {
                    if hf.frame_depth > self.frames.len() {
                        self.handler_stack.pop();
                    } else {
                        break;
                    }
                }
                if self.frames.is_empty() {
                    // This is handled by the caller of run_loop usually,
                    // but we need to signal that we returned.
                    // We can't return Value here easily without changing signature.
                    // Let's use a special error or just push it back if we want to continue?
                    // Actually, Instruction::Return is the end of run_loop if frames empty.
                    self.push(val);
                    Ok(None)
                } else {
                    self.push(val);
                    Ok(None)
                }
            }
            _ => Err(VmError::InvalidOpcode),
        }
    }

    fn handle_backedge(&mut self, target: usize) -> Result<(), VmError> {
        if let Some(chunk_idx) = self.frames.last().unwrap().chunk_idx {
            let m_idx = self.frames.last().unwrap().module_idx;

            self.local_hotness = self.local_hotness.wrapping_add(1);
            if self.local_hotness == 0 {
                let gc_count = self
                    .gc
                    .total_collections
                    .load(std::sync::atomic::Ordering::Relaxed);
                if gc_count > self.last_gc_count {
                    self.decay_hotness();
                    self.last_gc_count = gc_count;
                }

                let chunk = &self.modules[m_idx].chunks[chunk_idx];
                chunk
                    .hotness
                    .fetch_add(256, std::sync::atomic::Ordering::Relaxed);

                if let Some(jit) = self.jit.clone() {
                    if chunk.hotness.load(std::sync::atomic::Ordering::Relaxed) >= 10240 {
                        if let Ok(entry) = jit.osr(self, m_idx, chunk_idx, target as u32) {
                            println!("OSR triggered for chunk {} at target {}", chunk_idx, target);
                            match self.execute_jit_at(entry) {
                                Ok(Some(val)) => {
                                    // This is tricky, JIT finished the whole function.
                                    // We might need a way to exit run_loop.
                                    // For now, let's just push and signal.
                                    self.push(val);
                                    return Ok(());
                                }
                                Ok(None) => return Ok(()),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
