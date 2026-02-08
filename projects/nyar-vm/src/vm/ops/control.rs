use crate::vm::core::NyarVM;
use crate::vm::NyarError;
use crate::vm::value::Value;

impl NyarVM {
    #[inline(always)]
    pub fn execute_jump(&mut self, off: i16, cur_ip: usize) -> Result<Option<usize>, NyarError> {
        let target = (cur_ip as isize + off as isize) as usize;
        if off < 0 {
            if self.handle_backedge(target)? {
                return Ok(None);
            }
        }
        Ok(Some(target))
    }

    #[inline(always)]
    pub fn execute_jump_if_false(&mut self, off: i16, cur_ip: usize) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_truthy() {
            let target = (cur_ip as isize + off as isize) as usize;
            if off < 0 {
                if self.handle_backedge(target)? {
                    return Ok(None);
                }
            }
            Ok(Some(target))
        } else {
            Ok(Some(cur_ip + 1))
        }
    }

    #[inline(always)]
    pub fn execute_jump_if_true(&mut self, off: i16, cur_ip: usize) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if v.is_truthy() {
            let target = (cur_ip as isize + off as isize) as usize;
            if off < 0 {
                if self.handle_backedge(target)? {
                    return Ok(None);
                }
            }
            Ok(Some(target))
        } else {
            Ok(Some(cur_ip + 1))
        }
    }

    #[inline(always)]
    pub fn execute_jump_if_null(&mut self, off: i16, cur_ip: usize) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if v.is_null() {
            let target = (cur_ip as isize + off as isize) as usize;
            if off < 0 {
                if self.handle_backedge(target)? {
                    return Ok(None);
                }
            }
            Ok(Some(target))
        } else {
            Ok(Some(cur_ip + 1))
        }
    }

    #[inline(always)]
    pub fn execute_return(&mut self) -> Result<Option<usize>, NyarError> {
        let val = if self.sp > 0 {
            self.pop()?
        } else {
            Value::null()
        };
        self.frames.pop();
        while let Some(hf) = self.handler_stack.last() {
            if hf.frame_depth > self.frames.len() {
                self.handler_stack.pop();
            } else {
                break;
            }
        }
        self.push(val)?;
        Ok(None)
    }

    fn handle_backedge(&mut self, target: usize) -> Result<bool, NyarError> {
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

                let hotness = {
                    let module = self.get_module(m_idx);
                    module.chunks[chunk_idx].hotness.fetch_add(256, std::sync::atomic::Ordering::Relaxed) + 256
                };

                if let Some(jit) = self.jit.clone() {
                    if hotness >= 10240 {
                        if let Ok(entry) = jit.osr(self, m_idx, chunk_idx, target as u32) {
                            println!("OSR triggered for chunk {} at target {}", chunk_idx, target);
                            match self.execute_jit_at(entry) {
                                Ok(Some(val)) => {
                                    self.frames.pop();
                                    self.push(val)?;
                                    return Ok(true);
                                }
                                Ok(None) => return Ok(false),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }
}
