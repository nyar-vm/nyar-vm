#![doc = include_str!("readme.md")]

pub mod call;
pub mod closure;
pub mod control;
pub mod effects;
pub mod metaprogramming;
pub mod object;
pub mod stack;
pub mod traits;

use crate::bytecode::instruction::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::NyarError;
use nyar_types::QualifiedName;

impl NyarVM {
    pub fn execute(&mut self, module_idx: usize, chunk_idx: usize) -> Result<Value, NyarError> {
        if let Some(jit) = self.jit.clone() {
            if let Some(res) = jit.try_execute(self, module_idx, chunk_idx) {
                return res;
            }
        }
        println!("VM: Executing module {}, chunk {}", module_idx, chunk_idx);
        let instrs = self.get_chunk_instructions(module_idx, chunk_idx)?;

        let locals_count = 32;
        let frame = crate::vm::value::Frame {
            instrs,
            ip: 0,
            locals: vec![Value::null(); locals_count],
            upvalues: vec![None; locals_count],
            closure: Value::null(),
            module_idx,
            chunk_idx: Some(chunk_idx),
            location: Default::default(),
        };

        self.frames.push(frame);
        self.run_loop()
    }

    pub fn execute_symbol(&mut self, name: &QualifiedName, args: Vec<Value>) -> Result<Value, NyarError> {
        let entry = self.env.symbol_table.get(name).map(|r| *r.value());
        if let Some((m_idx, chunk_idx)) = entry {
            if let Some(jit) = self.jit.clone() {
                if let Some(res) = jit.try_execute(self, m_idx, chunk_idx as usize) {
                    return res;
                }
            }
            let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;

            let mut locals = args;
            let locals_count = 32;
            if locals.len() < locals_count {
                locals.resize(locals_count, Value::null());
            }

            let frame = crate::vm::value::Frame {
                instrs,
                ip: 0,
                locals,
                upvalues: vec![None; locals_count],
                closure: Value::null(),
                module_idx: m_idx,
                chunk_idx: Some(chunk_idx as usize),
                location: Default::default(),
            };

            self.frames.push(frame);
            self.run_loop()
        } else {
            Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name.clone())))
        }
    }

    pub fn get_chunk_instructions(
        &self,
        module_idx: usize,
        chunk_idx: usize,
    ) -> Result<std::sync::Arc<Vec<(Instruction, u32)>>, NyarError> {
        if module_idx >= self.env.modules.len() {
            return Err(self.error(nyar_types::VmErrorKind::ModuleNotFound(module_idx)));
        }
        let module = self.get_module(module_idx);
        if chunk_idx >= module.chunks.len() {
            return Err(self.error(nyar_types::VmErrorKind::ChunkNotFound {
                module: module_idx,
                chunk: chunk_idx,
            }));
        }
        let chunk = &module.chunks[chunk_idx];

        if let Some(instrs) = chunk.decoded.get() {
            return Ok(instrs.clone());
        }

        let mut decoder = crate::bytecode::decoder::Decoder::new(&chunk.code);
        let mut instructions = Vec::new();
        while decoder.position() < chunk.code.len() as u64 {
            let pos = decoder.position() as u32;
            if let Ok(ins) = decoder.next_result() {
                instructions.push((ins, pos));
            } else {
                break;
            }
        }

        let instrs = std::sync::Arc::new(instructions);
        let _ = chunk.decoded.set(instrs.clone());
        Ok(instrs)
    }

    pub fn execute_jit_at(&mut self, entry_ptr: *const u8) -> Result<Option<Value>, NyarError> {
        type JitEntry = unsafe extern "win64" fn(
            stack_ptr: *mut Value,
            sp: *mut usize,
            locals_ptr: *mut Value,
            upvalues_ptr: *mut Option<crate::vm::value::Upvalue>,
            ip_ptr: *mut usize,
            closure: Value,
            vm_ptr: *mut NyarVM,
        ) -> i32;

        let entry: JitEntry = unsafe { std::mem::transmute(entry_ptr) };
        if self.frames.is_empty() {
            return Err(self.error(nyar_types::VmErrorKind::NoActiveFrame));
        }
        let frame = self.frames.last_mut().unwrap();

        unsafe {
            let res_code = entry(
                self.stack.as_mut_ptr(),
                &mut self.sp as *mut usize,
                frame.locals.as_mut_ptr(),
                frame.upvalues.as_mut_ptr(),
                &mut frame.ip as *mut usize,
                frame.closure,
                self as *mut NyarVM,
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
                Err(self.error(nyar_types::VmErrorKind::RuntimeError("Generic runtime error".to_string())))
            }
        }
    }

    pub fn run_loop(&mut self) -> Result<Value, NyarError> {
        let mut loop_count = 0u64;
        
        // Use StackRootGuard to register VM as a root
        use nyar_gc::stack::StackRootGuard;
        let _vm_root = unsafe { StackRootGuard::<'static, NyarVM>::from_raw(self as *const NyarVM) };
        
        // Set current GC for write barriers
        let _gc_guard = crate::vm::core::CURRENT_GC.with(|curr| {
            let mut curr = curr.borrow_mut();
            let old = curr.take();
            *curr = Some(self.gc.clone());
            old
        });

        let res = loop {
            loop_count += 1;

            if loop_count % 1024 == 0 {
                if loop_count > 10_000_000 {
                    let err = self.error(nyar_types::VmErrorKind::LimitExceeded);
                    self.print_traceback(&err);
                    break Err(err);
                }
            }

            match self.execute_step() {
                Ok(Some(())) => continue,
                Ok(None) => break Ok(()),
                Err(e) => break Err(e),
            }
        };

        // Restore old GC
        crate::vm::core::CURRENT_GC.with(|curr| {
            *curr.borrow_mut() = _gc_guard;
        });

        res?;

        if self.sp > 0 {
            self.pop()
        } else {
            Ok(Value::null())
        }
    }

    pub fn execute_step(&mut self) -> Result<Option<()>, NyarError> {
        self.step_count = self.step_count.wrapping_add(1);
        if self.step_count % 1024 == 0 {
            let allocated = self.gc.allocated_bytes.load(std::sync::atomic::Ordering::Relaxed);
            let threshold = self.gc.threshold.load(std::sync::atomic::Ordering::Relaxed);

            if allocated >= threshold
                || nyar_gc::runtime::GC_STOP_THE_WORLD.load(std::sync::atomic::Ordering::Acquire)
            {
                self.gc.flush_thread_local();
                // Perform an incremental GC step
                unsafe {
                    use nyar_gc::Trace;
                    self.gc.step(1024, |ctx| {
                        nyar_gc::stack::scan_all_thread_roots(ctx);
                        self.trace(ctx);
                    });
                }
            }
        }

        let (cur_ip, module_idx, chunk_idx) = {
            let f = match self.frames.last() {
                Some(f) => f,
                None => return Ok(None),
            };
            if f.ip >= f.instrs.len() {
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(()));
            }
            (f.ip, f.module_idx, f.chunk_idx)
        };

        // Update location before dispatch
        if let Some(c_idx) = chunk_idx {
            let line_offset = {
                let module = self.get_module(module_idx);
                let chunk = &module.chunks[c_idx];
                let byte_offset = self.frames.last().unwrap().instrs[cur_ip].1;
                // Find the line info for the current IP
                // lines is Vec<(offset, line)>
                let mut line_offset = 0;
                for &(offset, line) in &chunk.lines {
                    if byte_offset >= offset {
                        line_offset = line;
                    } else {
                        break;
                    }
                }
                line_offset
            };
            if let Some(f) = self.frames.last_mut() {
                f.location = nyar_types::SourceLocation::new(module_idx as u32, line_offset);
            }
        }

        let ins = self.frames.last().unwrap().instrs[cur_ip].0.clone();

        #[cfg(debug_assertions)]
        {
            let log_msg = format!("VM: [{:04}] {:?} (stack size: {}) at {}", cur_ip, ins, self.sp, self.frames.last().unwrap().location);
            println!("{}", log_msg);
            self.trace_log.lock().unwrap().push(log_msg);
        }

        let mut pushed_frame = false;
        let next_ip = {
            let frame_count_before = self.frames.len();
            let res = self.dispatch_instruction(ins, cur_ip, module_idx)?;
            let frame_count_after = self.frames.len();

            if frame_count_after > frame_count_before {
                // A new frame was pushed.
                // Increment IP of the PREVIOUS frame so it continues after the call.
                if let Some(prev_f) = self.frames.get_mut(frame_count_before - 1) {
                    prev_f.ip += 1;
                }
                pushed_frame = true;
            }
            res
        };

        if let Some(f) = self.frames.last_mut() {
            if let Some(new_ip) = next_ip {
                f.ip = new_ip;
            } else if !pushed_frame {
                f.ip += 1;
            }
            Ok(Some(()))
        } else {
            // Return called and it was the last frame
            Ok(None)
        }
    }

    pub fn dispatch_instruction(
        &mut self,
        ins: Instruction,
        cur_ip: usize,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        match ins {
            Instruction::Nop => Ok(None),
            // I32 operations
            Instruction::I32Const(v) => self.execute_i32_const(v),
            Instruction::I32Add => self.execute_i32_add(),
            Instruction::I32Sub => self.execute_i32_sub(),
            Instruction::I32Mul => self.execute_i32_mul(),
            Instruction::I32DivS => self.execute_i32_div_s(),
            Instruction::I32DivU => self.execute_i32_div_u(),
            Instruction::I32RemS => self.execute_i32_rem_s(),
            Instruction::I32RemU => self.execute_i32_rem_u(),
            Instruction::I32And => self.execute_i32_and(),
            Instruction::I32Or => self.execute_i32_or(),
            Instruction::I32Xor => self.execute_i32_xor(),
            Instruction::I32Shl => self.execute_i32_shl(),
            Instruction::I32ShrS => self.execute_i32_shr_s(),
            Instruction::I32ShrU => self.execute_i32_shr_u(),
            Instruction::I32Not => self.execute_i32_not(),
            Instruction::I32Neg => self.execute_i32_neg(),
            Instruction::I32Eq => self.execute_i32_eq(),
            Instruction::I32Ne => self.execute_i32_ne(),
            Instruction::I32LtS => self.execute_i32_lt_s(),
            Instruction::I32LtU => self.execute_i32_lt_u(),
            Instruction::I32LeS => self.execute_i32_le_s(),
            Instruction::I32LeU => self.execute_i32_le_u(),
            Instruction::I32GtS => self.execute_i32_gt_s(),
            Instruction::I32GtU => self.execute_i32_gt_u(),
            Instruction::I32GeS => self.execute_i32_ge_s(),
            Instruction::I32GeU => self.execute_i32_ge_u(),
            Instruction::I32ToF32S => self.execute_i32_to_f32_s(),
            Instruction::I32ToF32U => self.execute_i32_to_f32_u(),
            Instruction::I32ToF64S => self.execute_i32_to_f64_s(),
            Instruction::I32ToF64U => self.execute_i32_to_f64_u(),
            Instruction::I32AddSatS => self.execute_i32_add_sat_s(),
            Instruction::I32AddSatU => self.execute_i32_add_sat_u(),
            Instruction::I32SubSatS => self.execute_i32_sub_sat_s(),
            Instruction::I32SubSatU => self.execute_i32_sub_sat_u(),
            Instruction::I32Extend64S => self.execute_i32_extend64_s(),
            Instruction::I32Extend64U => self.execute_i32_extend64_u(),
            Instruction::I32Trunc64SLow => self.execute_i32_trunc64_s_low(),
            Instruction::I32Trunc64S => self.execute_i32_trunc64_s(),
            Instruction::I32Trunc64U => self.execute_i32_trunc64_u(),
            // I64 operations
            Instruction::I64Const(v) => self.execute_i64_const(v),
            Instruction::I64Add => self.execute_i64_add(),
            Instruction::I64Sub => self.execute_i64_sub(),
            Instruction::I64Mul => self.execute_i64_mul(),
            Instruction::I64DivS => self.execute_i64_div_s(),
            Instruction::I64DivU => self.execute_i64_div_u(),
            Instruction::I64RemS => self.execute_i64_rem_s(),
            Instruction::I64RemU => self.execute_i64_rem_u(),
            Instruction::I64And => self.execute_i64_and(),
            Instruction::I64Or => self.execute_i64_or(),
            Instruction::I64Xor => self.execute_i64_xor(),
            Instruction::I64Shl => self.execute_i64_shl(),
            Instruction::I64ShrS => self.execute_i64_shr_s(),
            Instruction::I64ShrU => self.execute_i64_shr_u(),
            Instruction::I64Not => self.execute_i64_not(),
            Instruction::I64Neg => self.execute_i64_neg(),
            Instruction::I64Eq => self.execute_i64_eq(),
            Instruction::I64Ne => self.execute_i64_ne(),
            Instruction::I64LtS => self.execute_i64_lt_s(),
            Instruction::I64LtU => self.execute_i64_lt_u(),
            Instruction::I64LeS => self.execute_i64_le_s(),
            Instruction::I64LeU => self.execute_i64_le_u(),
            Instruction::I64GtS => self.execute_i64_gt_s(),
            Instruction::I64GtU => self.execute_i64_gt_u(),
            Instruction::I64GeS => self.execute_i64_ge_s(),
            Instruction::I64GeU => self.execute_i64_ge_u(),
            Instruction::I64ToF32S => self.execute_i64_to_f32_s(),
            Instruction::I64ToF32U => self.execute_i64_to_f32_u(),
            Instruction::I64ToF64S => self.execute_i64_to_f64_s(),
            Instruction::I64ToF64U => self.execute_i64_to_f64_u(),
            Instruction::I64AddSatS => self.execute_i64_add_sat_s(),
            Instruction::I64AddSatU => self.execute_i64_add_sat_u(),
            // Float operations
            Instruction::F32Const(v) => self.execute_f32_const(v),
            Instruction::F32Add => self.execute_f32_add(),
            Instruction::F32Sub => self.execute_f32_sub(),
            Instruction::F32Mul => self.execute_f32_mul(),
            Instruction::F32Div => self.execute_f32_div(),
            Instruction::F32Neg => self.execute_f32_neg(),
            Instruction::F32Eq => self.execute_f32_eq(),
            Instruction::F32Ne => self.execute_f32_ne(),
            Instruction::F32Lt => self.execute_f32_lt(),
            Instruction::F32Le => self.execute_f32_le(),
            Instruction::F32Gt => self.execute_f32_gt(),
            Instruction::F32Ge => self.execute_f32_ge(),
            Instruction::F32ToI32S => self.execute_f32_to_i32_s(),
            Instruction::F32ToI32U => self.execute_f32_to_i32_u(),
            Instruction::F32ToI64S => self.execute_f32_to_i64_s(),
            Instruction::F32ToI64U => self.execute_f32_to_i64_u(),
            Instruction::F32ToF64 => self.execute_f32_to_f64(),
            Instruction::F64Const(v) => self.execute_f64_const(v),
            Instruction::F64Add => self.execute_f64_add(),
            Instruction::F64Sub => self.execute_f64_sub(),
            Instruction::F64Mul => self.execute_f64_mul(),
            Instruction::F64Div => self.execute_f64_div(),
            Instruction::F64Neg => self.execute_f64_neg(),
            Instruction::F64Eq => self.execute_f64_eq(),
            Instruction::F64Ne => self.execute_f64_ne(),
            Instruction::F64Lt => self.execute_f64_lt(),
            Instruction::F64Le => self.execute_f64_le(),
            Instruction::F64Gt => self.execute_f64_gt(),
            Instruction::F64Ge => self.execute_f64_ge(),
            Instruction::F64ToI32S => self.execute_f64_to_i32_s(),
            Instruction::F64ToI32U => self.execute_f64_to_i32_u(),
            Instruction::F64ToI64S => self.execute_f64_to_i64_s(),
            Instruction::F64ToI64U => self.execute_f64_to_i64_u(),
            Instruction::F64ToF32 => self.execute_f64_to_f32(),
            // BigInt operations
            Instruction::BigIntConst { sign, bytes } => self.execute_bigint_const(sign, bytes),
            Instruction::BigIntAdd => self.execute_bigint_add(),
            Instruction::BigIntSub => self.execute_bigint_sub(),
            Instruction::BigIntMul => self.execute_bigint_mul(),
            Instruction::BigIntDiv => self.execute_bigint_div(),
            Instruction::BigIntMod => self.execute_bigint_mod(),
            Instruction::BigIntNeg => self.execute_bigint_neg(),
            Instruction::BigIntEq => self.execute_bigint_eq(),
            Instruction::BigIntNe => self.execute_bigint_ne(),
            Instruction::BigIntLt => self.execute_bigint_lt(),
            Instruction::BigIntLe => self.execute_bigint_le(),
            Instruction::BigIntGt => self.execute_bigint_gt(),
            Instruction::BigIntGe => self.execute_bigint_ge(),
            Instruction::BigIntToI64 => self.execute_bigint_to_i64(),
            Instruction::BigIntFromI64 => self.execute_bigint_from_i64(),
            Instruction::BigIntToString => self.execute_bigint_to_string(),
            // String operations
            Instruction::StringConst(s) => self.execute_string_const(s),
            Instruction::StringConcat => self.execute_string_concat(),
            Instruction::StringLenBytes => self.execute_string_len_bytes(),
            Instruction::StringLenChars => self.execute_string_len_chars(),
            Instruction::StringEq => self.execute_string_eq(),
            Instruction::StringNe => self.execute_string_ne(),
            Instruction::StringLt => self.execute_string_lt(),
            Instruction::StringLe => self.execute_string_le(),
            Instruction::StringGt => self.execute_string_gt(),
            Instruction::StringGe => self.execute_string_ge(),
            Instruction::StringSubstr => self.execute_string_substr(),
            // Stack operations
            Instruction::Push(idx) => self.execute_push(idx, module_idx),
            Instruction::Pop => self.execute_pop_stack(),
            Instruction::Dup(d) => self.execute_dup(d.into()),
            Instruction::Swap(d) => self.execute_swap(d.into()),
            Instruction::LoadLocal(idx) => self.execute_load_local(idx.into()),
            Instruction::StoreLocal(idx) => self.execute_store_local(idx.into()),
            Instruction::LoadGlobal(idx) => self.execute_load_global(idx, module_idx),
            Instruction::StoreGlobal(idx) => self.execute_store_global(idx, module_idx),
            // Control operations
            Instruction::Jump(off) => self.execute_jump(off, cur_ip),
            Instruction::JumpIfFalse(off) => self.execute_jump_if_false(off, cur_ip),
            Instruction::JumpIfTrue(off) => self.execute_jump_if_true(off, cur_ip),
            Instruction::JumpIfNull(off) => self.execute_jump_if_null(off, cur_ip),
            Instruction::Return => self.execute_return(),
            // Closure operations
            Instruction::MakeClosure(idx, upvalues) => {
                self.execute_make_closure(idx, upvalues, module_idx)
            }
            Instruction::LoadUpvalue(idx) => self.execute_load_upvalue(idx.into()),
            Instruction::StoreUpvalue(idx) => self.execute_store_upvalue(idx.into()),
            Instruction::CloseUpvalues => self.execute_close_upvalues(),
            // Object operations
            Instruction::NewObject(idx) => self.execute_new_object(idx),
            Instruction::GetField(idx) => self.execute_get_field(idx),
            Instruction::SetField(idx) => self.execute_set_field(idx),
            Instruction::NewArray(len) => self.execute_new_array(len.into()),
            Instruction::GetElement => self.execute_get_element(),
            Instruction::SetElement => self.execute_set_element(),
            Instruction::NewDynObject => self.execute_new_dyn_object(),
            Instruction::NewList(len) => self.execute_new_list(len.into()),
            Instruction::MakeTuple(len) => self.execute_new_tuple(len.into()),
            Instruction::HasKey => self.execute_has_key(),
            Instruction::RemoveKey => self.execute_remove_key(),
            Instruction::PushElementRight => self.execute_push_element_right(),
            Instruction::PopElementRight => self.execute_pop_element_right(),
            Instruction::PushElementLeft => self.execute_push_element_left(),
            Instruction::PopElementLeft => self.execute_pop_element_left(),
            Instruction::SizeOf => self.execute_size_of(),
            Instruction::TypeOf => self.execute_type_of(),
            Instruction::InstanceOf(idx) => self.execute_instance_of(idx),
            Instruction::MatchVariant(idx) => self.execute_match_variant(idx),
            Instruction::CheckCast(idx) => self.execute_check_cast(idx),
            Instruction::Cast(idx) => self.execute_cast(idx),
            // Effects operations
            Instruction::Perform(idx, argc) => self.execute_perform(idx, argc.into(), module_idx),
            Instruction::WithHandler(idx) => self.execute_with_handler(idx, module_idx),
            Instruction::ResumeWith => self.execute_resume_with(),
            Instruction::CaptureCont => self.execute_capture_cont(),
            Instruction::Await => self.execute_await(),
            Instruction::BlockOn => self.execute_block_on(),
            Instruction::MatchEffect(idx) => self.execute_match_effect(idx, module_idx),
            // Trait operations
            Instruction::GetWitnessTable(t_idx, i_idx) => {
                self.execute_get_witness_table(t_idx, i_idx, module_idx)
            }
            Instruction::WitnessMethod(idx) => self.execute_witness_method(idx),
            // Existential operations
            Instruction::OpenExistential => self.execute_open_existential(),
            Instruction::CloseExistential => self.execute_close_existential(),
            // Metaprogramming operations
            Instruction::Quote(idx) => self.execute_quote(idx),
            Instruction::Splice => self.execute_splice(),
            Instruction::Eval(argc) => self.execute_eval(argc.into()),
            Instruction::ExpandMacro(idx, argc) => self.execute_expand_macro(idx, argc.into(), module_idx),
            // Call operations
            Instruction::Call(idx, argc) => self.execute_call(idx, argc.into(), module_idx),
            Instruction::CallClosure(argc) => self.execute_call_closure(argc.into()),
            Instruction::CallSymbol(idx, argc) => {
                self.execute_call_symbol(idx, argc.into(), module_idx)
            }
            Instruction::CallDynamic(idx, argc) => {
                self.execute_call_dynamic(idx, argc.into(), module_idx)
            }
            Instruction::CallVirtual(idx, argc) => {
                self.execute_call_virtual(idx, argc.into(), module_idx)
            }
            Instruction::TailCall(idx, argc) => self.execute_tail_call(idx, argc, module_idx),
            Instruction::TailCallClosure(argc) => self.execute_tail_call_closure(argc),
            Instruction::FFICall(idx, argc) => self.execute_ffi_call(idx, argc.into(), module_idx),
            Instruction::InvokeMethod(idx, argc) => {
                self.execute_invoke_method(idx, argc.into(), module_idx)
            }
            Instruction::Initiate(v) => self.execute_initiate(v),
            Instruction::Finalize => self.execute_finalize(),
            Instruction::Halt => Err(self.error(nyar_types::VmErrorKind::Halt)),
        }
    }
    
}
