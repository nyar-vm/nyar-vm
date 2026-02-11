use crate::bytecode::format::Constant;
use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Frame, BigInt};
use num_bigint::BigInt as NativeBigInt;
use nyar_types::{NyarError, QualifiedName};

impl NyarVM {
    #[inline(always)]
    pub fn execute_call(
        &mut self,
        chunk_idx: u16,
        argc: u16,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let instrs = self.get_chunk_instructions(module_idx, chunk_idx as usize)?;
        let locals_count = {
            let module = self.get_module(module_idx);
            module.chunks[chunk_idx as usize].locals as usize
        };

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
            location: Default::default(),
        };

        self.frames.push(new_frame);
        Ok(Some(0))
    }

    #[inline(always)]
    pub fn execute_call_closure(&mut self, argc: u16) -> Result<Option<usize>, NyarError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let callee = self.pop()?;

        let (instrs, locals_count, c_module_idx, c_chunk_idx) =
            if let Some(closure) = callee.try_as_closure() {
                let chunk_idx = closure.func;
                let module_idx = closure.module_idx;
                let instrs = self.get_chunk_instructions(module_idx, chunk_idx)?;
                let locals_count = {
                    let module = self.get_module(module_idx);
                    module.chunks[chunk_idx].locals as usize
                };
                (instrs, locals_count, module_idx, chunk_idx)
            } else {
                return Err(self.error(nyar_types::VmErrorKind::RuntimeError("callee is not a closure".to_string())));
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
            location: Default::default(),
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
    ) -> Result<Option<usize>, NyarError> {
        let name = {
            let module = self.get_module(module_idx);
            match module.constants.get(name_idx as usize) {
                Some(Constant::QualifiedName(qn)) => qn.clone(),
                Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
                _ => {
                    return Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(name_idx as usize)))
                }
            }
        };

        let symbol = self.env.symbol_table.get(&name).map(|r| *r.value());
        if let Some((m_idx, chunk_idx)) = symbol {
            let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;
            let locals_count = {
                let module = self.get_module(m_idx);
                module.chunks[chunk_idx as usize].locals as usize
            };

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
                location: Default::default(),
            };

            self.frames.push(new_frame);
            Ok(Some(0))
        } else {
            // Check in Runtime registry
            let name_str = name.to_string();
            if let Some(func) = self.runtime.resolve(&name_str) {
                let mut args = Vec::with_capacity(argc as usize);
                for _ in 0..argc {
                    args.push(self.pop()?);
                }
                args.reverse();

                let result = func(self, &args)?;
                self.push(result)?;
                return Ok(None);
            }

            let callee = self.env.builtins.get(&name).map(|v| *v.value());
            if let Some(val) = callee {
                if let Some(closure) = val.try_as_closure() {
                    let m_idx = closure.module_idx;
                    let chunk_idx = closure.func;
                    let instrs = self.get_chunk_instructions(m_idx, chunk_idx)?;
                    let locals_count = {
                        let module = self.get_module(m_idx);
                        module.chunks[chunk_idx].locals as usize
                    };

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
                        closure: val,
                        module_idx: m_idx,
                        chunk_idx: Some(chunk_idx),
                        location: Default::default(),
                    };

                    self.frames.push(new_frame);
                    Ok(Some(0))
                } else {
                    Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name)))
                }
            } else {
                Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name)))
            }
        }
    }

    #[inline(always)]
    pub fn execute_invoke_method(
        &mut self,
        name_idx: u16,
        argc: u16,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let receiver = self.pop()?;

        let name = {
            let module = self.get_module(module_idx);
            match module.constants.get(name_idx as usize) {
                Some(Constant::QualifiedName(qn)) => qn.clone(),
                Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
                _ => return Err(self.error(nyar_types::VmErrorKind::InvalidOpcode(0x15))), // Opcode for INVOKE_METHOD
            }
        };

        if !receiver.is_object() {
            self.invoke_primitive_method(receiver, &name, args)?;
        } else {
            let obj = unsafe { receiver.as_object() };
            let class_info = {
                let module = self.get_module(obj.module_idx);
                module.classes.get(obj.class_idx as usize)
                    .ok_or_else(|| self.error(nyar_types::VmErrorKind::IndexOutOfBounds(obj.class_idx as usize)))?.clone()
            };

            // Try to find method "ClassName::MethodName"
            let mut method_name = class_info.name.clone();
            for part in &name.parts {
                method_name.push(part.clone());
            }

            #[cfg(debug_assertions)]
            println!("DEBUG: InvokeMethod searching for {} in symbol_table and builtins", method_name);

            let entry = self.env.symbol_table.get(&method_name).map(|r| *r.value());
            if let Some((m_idx, chunk_idx)) = entry {
                // Found class method, call it with receiver as first argument
                let mut final_args = Vec::with_capacity(args.len() + 1);
                final_args.push(receiver);
                final_args.extend(args);

                let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;
                let locals_count = {
                    let module = self.get_module(m_idx);
                    module.chunks[chunk_idx as usize].locals as usize
                };

                if final_args.len() < locals_count {
                    final_args.resize(locals_count, Value::null());
                }

                let new_frame = Frame {
                    instrs,
                    ip: 0,
                    locals: final_args,
                    upvalues: vec![None; locals_count],
                    closure: Value::null(),
                    module_idx: m_idx,
                    chunk_idx: Some(chunk_idx as usize),
                    location: Default::default(),
                };

                self.frames.push(new_frame);
                return Ok(Some(0));
            }
            let builtin_method = self.env.builtins.get(&method_name).map(|v| *v.value());
            if let Some(callee) = builtin_method {
                // Found method in builtins (e.g. a closure defined in a class body)
                if callee.is_closure() {
                    let mut final_args = Vec::with_capacity(args.len() + 1);
                    final_args.push(receiver);
                    final_args.extend(args);

                    let closure = unsafe { callee.as_closure() };
                    let m_idx = closure.module_idx;
                    let c_idx = closure.func;
                    let instrs = self.get_chunk_instructions(m_idx, c_idx)?;
                    let locals_count = {
                        let module = self.get_module(m_idx);
                        module.chunks[c_idx].locals as usize
                    };

                    if final_args.len() < locals_count {
                        final_args.resize(locals_count, Value::null());
                    }

                    let new_frame = Frame {
                        instrs,
                        ip: 0,
                        locals: final_args,
                        upvalues: vec![None; locals_count],
                        closure: callee,
                        module_idx: m_idx,
                        chunk_idx: Some(c_idx),
                        location: Default::default(),
                    };

                    self.frames.push(new_frame);
                    return Ok(Some(0));
                }
            }

            #[cfg(debug_assertions)]
            {
                println!("DEBUG: Method {} not found. Available builtins:", method_name);
                for entry in self.env.builtins.iter() {
                    println!("  - {}", entry.key());
                }
            }

            // Method not found on class, maybe it's a dynamic property that is a closure?
            // Try searching for just "MethodName" in class fields
            let field_idx = class_info.fields.iter().position(|f| f == name.parts.last().unwrap())
                .ok_or_else(|| self.error(nyar_types::VmErrorKind::SymbolNotFound(method_name.clone())))?;

            if field_idx < obj.fields.len() {
                let callee = obj.fields[field_idx];
                if callee.is_closure() {
                    // It's a closure stored in a field, call it with arguments (including self)
                    let mut final_args = Vec::with_capacity(args.len() + 1);
                    final_args.push(receiver);
                    final_args.extend(args);

                    let closure = unsafe { callee.as_closure() };
                    let m_idx = closure.module_idx;
                    let c_idx = closure.func;
                    let instrs = self.get_chunk_instructions(m_idx, c_idx)?;
                    let locals_count = {
                        let module = self.get_module(m_idx);
                        module.chunks[c_idx].locals as usize
                    };

                    if final_args.len() < locals_count {
                        final_args.resize(locals_count, Value::null());
                    }

                    let new_frame = Frame {
                        instrs,
                        ip: 0,
                        locals: final_args,
                        upvalues: vec![None; locals_count],
                        closure: callee,
                        module_idx: m_idx,
                        chunk_idx: Some(c_idx),
                        location: Default::default(),
                    };

                    self.frames.push(new_frame);
                    return Ok(Some(0));
                }
            }
            return Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(method_name)));
        }
        Ok(None)
    }

    fn invoke_primitive_method(
        &mut self,
        receiver: Value,
        name: &QualifiedName,
        args: Vec<Value>,
    ) -> Result<(), NyarError> {
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
                        if let Some(res) = l.checked_add(r) {
                            self.push(Value::int(res))?;
                        } else {
                            self.push(Value::bigint(BigInt(NativeBigInt::from(l) + NativeBigInt::from(r)), &self.gc))?;
                        }
                    } else if receiver.is_f32() && rhs.is_f32() {
                        self.push(Value::f32(receiver.as_f32() + rhs.as_f32()))?;
                    } else if (receiver.is_f64() || receiver.is_int() || receiver.is_f32())
                        && (rhs.is_f64() || rhs.is_int() || rhs.is_f32())
                    {
                        self.push(Value::float(receiver.to_f64() + rhs.to_f64()))?;
                    } else if receiver.is_string() && rhs.is_string() {
                        if let (Some(l), Some(r)) = (receiver.try_as_str(), rhs.try_as_str()) {
                            let mut s = l.to_string();
                            s.push_str(r);
                            self.push(Value::string(s, &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            self.push(Value::bigint(BigInt(l + &r), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "sub" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        if let Some(res) = l.checked_sub(r) {
                            self.push(Value::int(res))?;
                        } else {
                            self.push(Value::bigint(BigInt(NativeBigInt::from(l) - NativeBigInt::from(r)), &self.gc))?;
                        }
                    } else if receiver.is_f32() && rhs.is_f32() {
                        self.push(Value::f32(receiver.as_f32() - rhs.as_f32()))?;
                    } else if (receiver.is_f64() || receiver.is_int() || receiver.is_f32())
                        && (rhs.is_f64() || rhs.is_int() || rhs.is_f32())
                    {
                        self.push(Value::float(receiver.to_f64() - rhs.to_f64()))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            self.push(Value::bigint(BigInt(l - &r), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "mul" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        if let Some(res) = l.checked_mul(r) {
                            self.push(Value::int(res))?;
                        } else {
                            self.push(Value::bigint(BigInt(NativeBigInt::from(l) * NativeBigInt::from(r)), &self.gc))?;
                        }
                    } else if receiver.is_f32() && rhs.is_f32() {
                        self.push(Value::f32(receiver.as_f32() * rhs.as_f32()))?;
                    } else if (receiver.is_f64() || receiver.is_int() || receiver.is_f32())
                        && (rhs.is_f64() || rhs.is_int() || rhs.is_f32())
                    {
                        self.push(Value::float(receiver.to_f64() * rhs.to_f64()))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            self.push(Value::bigint(BigInt(l * &r), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "div" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        if r != 0 {
                            self.push(Value::int(l / r))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else if receiver.is_f32() && rhs.is_f32() {
                        self.push(Value::f32(receiver.as_f32() / rhs.as_f32()))?;
                    } else if (receiver.is_f64() || receiver.is_int() || receiver.is_f32())
                        && (rhs.is_f64() || rhs.is_int() || rhs.is_f32())
                    {
                        self.push(Value::float(receiver.to_f64() / rhs.to_f64()))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            if r != NativeBigInt::from(0) {
                                self.push(Value::bigint(BigInt(l / &r), &self.gc))?;
                            } else {
                                self.push(Value::null())?;
                            }
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "rem" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        if r != 0 {
                            self.push(Value::int(l % r))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else if receiver.is_f32() && rhs.is_f32() {
                        self.push(Value::f32(receiver.as_f32() % rhs.as_f32()))?;
                    } else if (receiver.is_f64() || receiver.is_int() || receiver.is_f32())
                        && (rhs.is_f64() || rhs.is_int() || rhs.is_f32())
                    {
                        self.push(Value::float(receiver.to_f64() % rhs.to_f64()))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            if r != NativeBigInt::from(0) {
                                self.push(Value::bigint(BigInt(l % &r), &self.gc))?;
                            } else {
                                self.push(Value::null())?;
                            }
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "bit_and" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        self.push(Value::int(l & r))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            self.push(Value::bigint(BigInt(l & &r), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "bit_or" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        self.push(Value::int(l | r))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            self.push(Value::bigint(BigInt(l | &r), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "bit_xor" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        self.push(Value::int(l ^ r))?;
                    } else if (receiver.is_bigint() || receiver.is_int()) && (rhs.is_bigint() || rhs.is_int()) {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        let r = if rhs.is_bigint() {
                            rhs.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(rhs.as_int()))
                        };
                        if let (Some(l), Some(r)) = (l, r) {
                            self.push(Value::bigint(BigInt(l ^ &r), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "bit_not" => {
                if args.is_empty() {
                    if let Some(l) = receiver.try_as_int() {
                        self.push(Value::int(!l))?;
                    } else if let Some(l) = receiver.try_as_bigint() {
                        self.push(Value::bigint(BigInt(!l.0.clone()), &self.gc))?;
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "bit_shl" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        if let Some(res) = l.checked_shl(r as u32) {
                            self.push(Value::int(res))?;
                        } else {
                            let l = NativeBigInt::from(l);
                            let shift = r;
                            if shift >= 0 {
                                self.push(Value::bigint(BigInt(l << (shift as usize)), &self.gc))?;
                            } else {
                                self.push(Value::bigint(BigInt(l >> ((-shift) as usize)), &self.gc))?;
                            }
                        }
                    } else if (receiver.is_bigint() || receiver.is_int()) && rhs.is_int() {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        if let Some(l) = l {
                            let shift = rhs.as_int();
                            if shift >= 0 {
                                self.push(Value::bigint(BigInt(l << (shift as usize)), &self.gc))?;
                            } else {
                                self.push(Value::bigint(BigInt(l >> ((-shift) as usize)), &self.gc))?;
                            }
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "bit_shr" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        if let Some(res) = l.checked_shr(r as u32) {
                            self.push(Value::int(res))?;
                        } else {
                            let l = NativeBigInt::from(l);
                            let shift = r;
                            if shift >= 0 {
                                self.push(Value::bigint(BigInt(l >> (shift as usize)), &self.gc))?;
                            } else {
                                self.push(Value::bigint(BigInt(l << ((-shift) as usize)), &self.gc))?;
                            }
                        }
                    } else if (receiver.is_bigint() || receiver.is_int()) && rhs.is_int() {
                        let l = if receiver.is_bigint() {
                            receiver.try_as_bigint().map(|b| b.0.clone())
                        } else {
                            Some(NativeBigInt::from(receiver.as_int()))
                        };
                        if let Some(l) = l {
                            let shift = rhs.as_int();
                            if shift >= 0 {
                                self.push(Value::bigint(BigInt(l >> (shift as usize)), &self.gc))?;
                            } else {
                                self.push(Value::bigint(BigInt(l << ((-shift) as usize)), &self.gc))?;
                            }
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "neg" => {
                if args.is_empty() {
                    if let Some(l) = receiver.try_as_int() {
                        self.push(Value::int(-l))?;
                    } else if receiver.is_f32() {
                        self.push(Value::f32(-receiver.as_f32()))?;
                    } else if receiver.is_f64() {
                        self.push(Value::float(-receiver.as_f64()))?;
                    } else if receiver.is_bigint() {
                        if let Some(l) = receiver.try_as_bigint() {
                            self.push(Value::bigint(BigInt(-l.0.clone()), &self.gc))?;
                        } else {
                            self.push(Value::null())?;
                        }
                    } else {
                        self.push(Value::null())?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "not" => {
                if args.is_empty() {
                    if let Some(l) = receiver.try_as_int() {
                        self.push(Value::bool(l == 0))?;
                    } else {
                        self.push(Value::bool(receiver.is_null()))?;
                    }
                } else {
                    self.push(Value::null())?;
                }
            }
            "eq" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    self.push(Value::bool(self.compare_values(receiver, rhs, "eq")))?;
                } else {
                    self.push(Value::bool(false))?;
                }
            }
            "ne" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    self.push(Value::bool(self.compare_values(receiver, rhs, "ne")))?;
                } else {
                    self.push(Value::bool(true))?;
                }
            }
            "lt" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    self.push(Value::bool(self.compare_values(receiver, rhs, "lt")))?;
                } else {
                    self.push(Value::bool(false))?;
                }
            }
            "le" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    self.push(Value::bool(self.compare_values(receiver, rhs, "le")))?;
                } else {
                    self.push(Value::bool(false))?;
                }
            }
            "gt" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    self.push(Value::bool(self.compare_values(receiver, rhs, "gt")))?;
                } else {
                    self.push(Value::bool(false))?;
                }
            }
            "ge" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    self.push(Value::bool(self.compare_values(receiver, rhs, "ge")))?;
                } else {
                    self.push(Value::bool(false))?;
                }
            }
            _ => {
                self.push(Value::null())?;
            }
        }
        Ok(())
    }

    fn compare_values(&self, lhs: Value, rhs: Value, op: &str) -> bool {
        if lhs.is_int() && rhs.is_int() {
            let l = lhs.as_int();
            let r = rhs.as_int();
            return match op {
                "eq" => l == r,
                "ne" => l != r,
                "lt" => l < r,
                "le" => l <= r,
                "gt" => l > r,
                "ge" => l >= r,
                _ => false,
            };
        } else if (lhs.is_int() || lhs.is_f64() || lhs.is_f32())
            && (rhs.is_int() || rhs.is_f64() || rhs.is_f32())
        {
            let l = if lhs.is_int() {
                lhs.as_int() as f64
            } else if lhs.is_f32() {
                lhs.as_f32() as f64
            } else {
                lhs.as_f64()
            };
            let r = if rhs.is_int() {
                rhs.as_int() as f64
            } else if rhs.is_f32() {
                rhs.as_f32() as f64
            } else {
                rhs.as_f64()
            };
            return match op {
                "eq" => l == r,
                "ne" => l != r,
                "lt" => l < r,
                "le" => l <= r,
                "gt" => l > r,
                "ge" => l >= r,
                _ => false,
            };
        } else if lhs.is_string() && rhs.is_string() {
            if let (Some(l), Some(r)) = (lhs.try_as_str(), rhs.try_as_str()) {
                return match op {
                    "eq" => l == r,
                    "ne" => l != r,
                    "lt" => l < r,
                    "le" => l <= r,
                    "gt" => l > r,
                    "ge" => l >= r,
                    _ => false,
                };
            }
            return false;
        } else if lhs.is_bigint() || rhs.is_bigint() {
            // BigInt vs BigInt, or BigInt vs Int
            let l = if lhs.is_bigint() {
                lhs.try_as_bigint().map(|b| b.0.clone())
            } else if lhs.is_int() {
                Some(NativeBigInt::from(lhs.as_int()))
            } else {
                None
            };
            
            let r = if rhs.is_bigint() {
                rhs.try_as_bigint().map(|b| b.0.clone())
            } else if rhs.is_int() {
                Some(NativeBigInt::from(rhs.as_int()))
            } else {
                None
            };

            if let (Some(l), Some(r)) = (l, r) {
                return match op {
                    "eq" => l == r,
                    "ne" => l != r,
                    "lt" => l < r,
                    "le" => l <= r,
                    "gt" => l > r,
                    "ge" => l >= r,
                    _ => false,
                };
            }
            // If one side is float and other is bigint, we don't handle it yet (fall through to default)
            return match op {
                "eq" => false,
                "ne" => true,
                _ => false,
            };
        } else {
            return match op {
                "eq" => false,
                "ne" => true,
                _ => false,
            };
        }
    }

    #[inline(always)]
    pub fn execute_tail_call_closure(&mut self, argc: u8) -> Result<Option<usize>, NyarError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let callee = self.pop()?;

        let (instrs, locals_count, c_module_idx, c_chunk_idx) =
            if let Some(closure) = callee.try_as_closure() {
                let chunk_idx = closure.func;
                let module_idx = closure.module_idx;
                let instrs = self.get_chunk_instructions(module_idx, chunk_idx)?;
                let locals_count = {
                    let module = self.get_module(module_idx);
                    module.chunks[chunk_idx].locals as usize
                };
                (instrs, locals_count, module_idx, chunk_idx)
            } else {
                return Err(self.error(nyar_types::VmErrorKind::InvalidOpcode(0x16))); // Opcode for TAIL_CALL_CLOSURE
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
    pub fn execute_tail_call(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let name = {
            let module = self.get_module(module_idx);
            match module.constants.get(idx as usize) {
                Some(Constant::QualifiedName(qn)) => qn.clone(),
                Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
                _ => return Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize))),
            }
        };

        let symbol = self.env.symbol_table.get(&name).map(|r| *r.value());
        if let Some((m_idx, chunk_idx)) = symbol {
            let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;
            let locals_count = {
                let module = self.get_module(m_idx);
                module.chunks[chunk_idx as usize].locals as usize
            };

            let mut args = Vec::with_capacity(argc as usize);
            for _ in 0..argc {
                args.push(self.pop()?);
            }
            args.reverse();

            if args.len() < locals_count {
                args.resize(locals_count, Value::null());
            }

            // Reuse the current frame
            if let Some(frame) = self.frames.last_mut() {
                frame.instrs = instrs;
                frame.ip = 0;
                frame.locals = args;
                frame.closure = Value::null();
                frame.module_idx = m_idx;
                frame.chunk_idx = Some(chunk_idx as usize);
            }

            Ok(Some(0))
        } else {
            Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name)))
        }
    }

    #[inline(always)]
    pub fn execute_call_virtual(
        &mut self,
        idx: u16,
        argc: u8,
        _module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let trait_val = self.pop()?;

        let trait_obj = trait_val.try_as_trait_object().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x17)))?; // Opcode for CALL_VIRTUAL
        
        let witness_val = trait_obj.witness;
        let witness = unsafe { witness_val.as_witness_table() };
        
        let chunk_idx = witness.methods.get(idx as usize).ok_or_else(|| self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize)))?;
        let target_module_idx = witness.module_idx;

        // The first argument to a trait method is usually the data (self)
        let mut final_args = Vec::with_capacity(argc as usize + 1);
        final_args.push(trait_obj.data);
        final_args.extend(args);

        let instrs = self.get_chunk_instructions(target_module_idx, *chunk_idx as usize)?;
        let locals_count = {
            let module = self.get_module(target_module_idx);
            module.chunks[*chunk_idx as usize].locals as usize
        };

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
            location: Default::default(),
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
    ) -> Result<Option<usize>, NyarError> {
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
            let name = QualifiedName::from(name_str);
            self.invoke_primitive_method(receiver, &name, args)?;
        } else {
            return Err(self.error(nyar_types::VmErrorKind::InvalidOpcode(0x18))); // Opcode for CALL_DYNAMIC
        }
        
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_ffi_call(
        &mut self,
        idx: u16,
        argc: u8,
        module_idx: usize,
    ) -> Result<Option<usize>, NyarError> {
        let name_qn = {
            let module = self.get_module(module_idx);
            match module.constants.get(idx as usize) {
                Some(Constant::QualifiedName(qn)) => qn.clone(),
                Some(Constant::String(s)) => QualifiedName::from(s.as_str()),
                _ => return Err(self.error(nyar_types::VmErrorKind::IndexOutOfBounds(idx as usize))),
            }
        };
        let name = name_qn.to_string();

        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // Check in Runtime registry
        if let Some(func) = self.runtime.resolve(&name) {
            let result = func(self, &args)?;
            self.push(result)?;
        } else if name.starts_with("$intrinsic:") {
            // Check if it's an encoded intrinsic call
            if let Ok(id) = name.trim_start_matches("$intrinsic:").parse::<u32>() {
                if let Some(func) = self.runtime.get_intrinsic(id) {
                    let result = func(self, &args)?;
                    self.push(result)?;
                } else {
                    return Err(self.error(nyar_types::VmErrorKind::RuntimeError(format!(
                        "Intrinsic not found: ID={}",
                        id
                    ))));
                }
            } else {
                return Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name_qn)));
            }
        } else {
            // If not found in FFI, maybe it's a builtin?
            if let Some(_val) = self.env.builtins.get(&name_qn).map(|v| *v.value()) {
                // If it's a closure/function, we should probably call it, 
                // but FFICall usually implies direct native call.
                // For now, return error if not a native function.
                return Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name_qn)));
            }
            return Err(self.error(nyar_types::VmErrorKind::SymbolNotFound(name_qn)));
        }

        Ok(None)
    }
}
