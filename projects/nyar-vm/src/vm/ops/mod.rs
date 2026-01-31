pub mod bigint;
pub mod call;
pub mod closure;
pub mod control;
pub mod float;
pub mod i32;
pub mod i64;
pub mod stack;
pub mod string;

use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute(&mut self, module_idx: usize, chunk_idx: usize) -> Result<Value, VmError> {
        if let Some(jit) = self.jit.clone() {
            if let Some(res) = jit.try_execute(self, module_idx, chunk_idx) {
                return res;
            }
        }
        println!("VM: Executing module {}, chunk {}", module_idx, chunk_idx);
        let instrs = self.get_chunk_instructions(module_idx, chunk_idx)?;

        let frame = crate::vm::core::Frame {
            instrs,
            ip: 0,
            locals: vec![Value::null(); 32],
            closure: Value::null(),
            module_idx,
            chunk_idx: Some(chunk_idx),
        };

        self.frames.push(frame);
        self.run_loop()
    }

    pub fn execute_symbol(&mut self, name: &str, args: Vec<Value>) -> Result<Value, VmError> {
        if let Some(&(m_idx, chunk_idx)) = self.symbol_table.get(name) {
            if let Some(jit) = self.jit.clone() {
                if let Some(res) = jit.try_execute(self, m_idx, chunk_idx as usize) {
                    return res;
                }
            }
            let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;

            let mut locals = args;
            if locals.len() < 32 {
                locals.resize(32, Value::null());
            }

            let frame = crate::vm::core::Frame {
                instrs,
                ip: 0,
                locals,
                closure: Value::null(),
                module_idx: m_idx,
                chunk_idx: Some(chunk_idx as usize),
            };

            self.frames.push(frame);
            self.run_loop()
        } else {
            Err(VmError::RuntimeError(format!("Symbol not found: {}", name)))
        }
    }

    pub fn get_chunk_instructions(
        &mut self,
        module_idx: usize,
        chunk_idx: usize,
    ) -> Result<std::sync::Arc<Vec<Instruction>>, VmError> {
        let module = self.modules.get_mut(module_idx).ok_or_else(|| {
            VmError::RuntimeError(format!("Module index out of bounds: {}", module_idx))
        })?;
        let chunk = module.chunks.get_mut(chunk_idx).ok_or_else(|| {
            VmError::RuntimeError(format!(
                "Chunk index out of bounds: {} in module {}",
                chunk_idx, module_idx
            ))
        })?;

        if let Some(ref instrs) = chunk.decoded {
            return Ok(instrs.clone());
        }

        let mut decoder = crate::bytecode::decoder::Decoder::new(&chunk.code);
        let mut instructions = Vec::new();
        while let Ok(ins) = decoder.next_result() {
            instructions.push(ins);
        }

        let instrs = std::sync::Arc::new(instructions);
        chunk.decoded = Some(instrs.clone());
        Ok(instrs)
    }

    pub fn execute_jit_at(&mut self, entry_ptr: *const u8) -> Result<Option<Value>, VmError> {
        type JitEntry = unsafe extern "C" fn(
            stack_ptr: *mut Value,
            sp: *mut usize,
            locals_ptr: *mut Value,
            ip_ptr: *mut usize,
        ) -> i32;

        let entry: JitEntry = unsafe { std::mem::transmute(entry_ptr) };
        let frame = self
            .frames
            .last_mut()
            .ok_or(VmError::RuntimeError("No active frame".to_string()))?;

        unsafe {
            let res_code = entry(
                self.stack.as_mut_ptr(),
                &mut self.sp as *mut usize,
                frame.locals.as_mut_ptr(),
                &mut frame.ip as *mut usize,
            );

            if res_code == 0 {
                if self.sp > 0 {
                    self.sp -= 1;
                    Ok(Some(self.stack[self.sp]))
                } else {
                    Ok(Some(Value::null()))
                }
            } else if res_code == 1 || res_code == 2 {
                // 1: Success but no value (e.g. tail call or special exit)
                // 2: OSR Exit (return to interpreter)
                Ok(None)
            } else {
                Err(VmError::RuntimeError(format!(
                    "JIT execution failed or requested deopt with code {}",
                    res_code
                )))
            }
        }
    }

    pub fn run_loop(&mut self) -> Result<Value, VmError> {
        let mut loop_count = 0u64;
        loop {
            loop_count += 1;
            if loop_count > 10_000_000 {
                let err = VmError::RuntimeError(
                    "Maximum instruction limit exceeded (potential infinite loop)".to_string(),
                );
                self.print_traceback(&err);
                return Err(err);
            }

            let (cur_ip, module_idx) = {
                let f = match self.frames.last() {
                    Some(f) => f,
                    None => break,
                };
                if f.ip >= f.instrs.len() {
                    break;
                }
                (f.ip, f.module_idx)
            };

            let ins = self.frames.last().unwrap().instrs[cur_ip].clone();

            #[cfg(debug_assertions)]
            println!("VM: [{:04}] {:?} (stack size: {})", cur_ip, ins, self.sp);

            let next_ip = self.dispatch_instruction(ins, cur_ip, module_idx)?;

            if let Some(f) = self.frames.last_mut() {
                if let Some(new_ip) = next_ip {
                    f.ip = new_ip;
                } else {
                    f.ip += 1;
                }
            } else {
                // Return called and it was the last frame
                break;
            }
        }

        if self.sp > 0 {
            self.pop()
        } else {
            Ok(Value::null())
        }
    }

    fn dispatch_instruction(
        &mut self,
        ins: Instruction,
        cur_ip: usize,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        match ins {
            Instruction::Nop => Ok(None),
            ins if ins.is_bigint_op() => {
                self.execute_bigint_op(ins)?;
                Ok(None)
            }
            ins if ins.is_i32_op() => {
                self.execute_i32_op(ins)?;
                Ok(None)
            }
            ins if ins.is_i64_op() => {
                self.execute_i64_op(ins)?;
                Ok(None)
            }
            ins if ins.is_float_op() => {
                self.execute_float_op(ins)?;
                Ok(None)
            }
            ins if ins.is_string_op() => {
                self.execute_string_op(ins)?;
                Ok(None)
            }
            ins if ins.is_stack_op() => {
                self.execute_stack_op(ins, module_idx)?;
                Ok(None)
            }
            ins if ins.is_control_op() => self.execute_control_op(ins, cur_ip),
            ins if ins.is_closure_op() => {
                self.execute_closure_op(ins, module_idx)?;
                Ok(None)
            }
            ins if ins.is_call_op() => self.execute_call_op(ins, module_idx),
            _ => Err(VmError::InvalidOpcode),
        }
    }
}

impl Instruction {
    pub fn is_stack_op(&self) -> bool {
        matches!(
            self,
            Instruction::Push(_)
                | Instruction::Pop
                | Instruction::Dup(_)
                | Instruction::Swap(_)
                | Instruction::LoadLocal(_)
                | Instruction::StoreLocal(_)
                | Instruction::LoadGlobal(_)
                | Instruction::StoreGlobal(_)
        )
    }

    pub fn is_control_op(&self) -> bool {
        matches!(
            self,
            Instruction::Jump(_) | Instruction::JumpIfFalse(_) | Instruction::Return
        )
    }

    pub fn is_closure_op(&self) -> bool {
        matches!(
            self,
            Instruction::MakeClosure(_, _)
                | Instruction::LoadUpvalue(_)
                | Instruction::StoreUpvalue(_)
        )
    }

    pub fn is_call_op(&self) -> bool {
        matches!(
            self,
            Instruction::Call(_, _)
                | Instruction::CallClosure(_)
                | Instruction::CallSymbol(_, _)
                | Instruction::InvokeMethod(_, _)
        )
    }
    pub fn is_bigint_op(&self) -> bool {
        matches!(
            self,
            Instruction::BigIntConst { .. }
                | Instruction::BigIntAdd
                | Instruction::BigIntSub
                | Instruction::BigIntMul
                | Instruction::BigIntDiv
                | Instruction::BigIntMod
                | Instruction::BigIntNeg
                | Instruction::BigIntEq
                | Instruction::BigIntNe
                | Instruction::BigIntLt
                | Instruction::BigIntLe
                | Instruction::BigIntGt
                | Instruction::BigIntGe
                | Instruction::BigIntToI64
                | Instruction::BigIntFromI64
                | Instruction::BigIntToString
        )
    }

    pub fn is_i32_op(&self) -> bool {
        matches!(
            self,
            Instruction::I32Const(_)
                | Instruction::I32Add
                | Instruction::I32Sub
                | Instruction::I32Mul
                | Instruction::I32DivS
                | Instruction::I32DivU
                | Instruction::I32RemS
                | Instruction::I32RemU
                | Instruction::I32Neg
                | Instruction::I32Eq
                | Instruction::I32Ne
                | Instruction::I32LtS
                | Instruction::I32LtU
                | Instruction::I32LeS
                | Instruction::I32LeU
                | Instruction::I32GtS
                | Instruction::I32GtU
                | Instruction::I32GeS
                | Instruction::I32GeU
                | Instruction::I32ToF32S
                | Instruction::I32ToF32U
                | Instruction::I32ToF64S
                | Instruction::I32ToF64U
                | Instruction::I32Extend64S
                | Instruction::I32Extend64U
                | Instruction::I32Trunc64SLow
                | Instruction::I32Trunc64S
                | Instruction::I32Trunc64U
        )
    }

    pub fn is_i64_op(&self) -> bool {
        matches!(
            self,
            Instruction::I64Const(_)
                | Instruction::I64Add
                | Instruction::I64Sub
                | Instruction::I64Mul
                | Instruction::I64DivS
                | Instruction::I64DivU
                | Instruction::I64RemS
                | Instruction::I64RemU
                | Instruction::I64Neg
                | Instruction::I64Eq
                | Instruction::I64Ne
                | Instruction::I64LtS
                | Instruction::I64LtU
                | Instruction::I64LeS
                | Instruction::I64LeU
                | Instruction::I64GtS
                | Instruction::I64GtU
                | Instruction::I64GeS
                | Instruction::I64GeU
                | Instruction::I64ToF32S
                | Instruction::I64ToF32U
                | Instruction::I64ToF64S
                | Instruction::I64ToF64U
        )
    }

    pub fn is_float_op(&self) -> bool {
        matches!(
            self,
            Instruction::F32Const(_)
                | Instruction::F32Add
                | Instruction::F32Sub
                | Instruction::F32Mul
                | Instruction::F32Div
                | Instruction::F32Neg
                | Instruction::F32Eq
                | Instruction::F32Ne
                | Instruction::F32Lt
                | Instruction::F32Le
                | Instruction::F32Gt
                | Instruction::F32Ge
                | Instruction::F32ToI32S
                | Instruction::F32ToI32U
                | Instruction::F32ToI64S
                | Instruction::F32ToI64U
                | Instruction::F32ToF64
                | Instruction::F64Const(_)
                | Instruction::F64Add
                | Instruction::F64Sub
                | Instruction::F64Mul
                | Instruction::F64Div
                | Instruction::F64Neg
                | Instruction::F64Eq
                | Instruction::F64Ne
                | Instruction::F64Lt
                | Instruction::F64Le
                | Instruction::F64Gt
                | Instruction::F64Ge
                | Instruction::F64ToI32S
                | Instruction::F64ToI32U
                | Instruction::F64ToI64S
                | Instruction::F64ToI64U
                | Instruction::F64ToF32
        )
    }

    pub fn is_string_op(&self) -> bool {
        matches!(
            self,
            Instruction::StringConst(_)
                | Instruction::StringConcat
                | Instruction::StringLenBytes
                | Instruction::StringLenChars
                | Instruction::StringEq
                | Instruction::StringNe
                | Instruction::StringLt
                | Instruction::StringLe
        )
    }
}
