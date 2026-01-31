use crate::bytecode::instruction::Instruction;
use crate::bytecode::format::Constant;
use crate::vm::core::{Frame, NyarVM};
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_call_op(
        &mut self,
        ins: Instruction,
        module_idx: usize,
    ) -> Result<Option<usize>, VmError> {
        match ins {
            Instruction::Call(chunk_idx, argc) => {
                let instrs = self.get_chunk_instructions(module_idx, chunk_idx as usize)?;
                let locals_count =
                    self.modules[module_idx].chunks[chunk_idx as usize].locals as usize;

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
                    closure: Value::null(),
                    module_idx,
                    chunk_idx: Some(chunk_idx as usize),
                };

                self.frames.push(new_frame);
                Ok(Some(0))
            }
            Instruction::CallClosure(argc) => {
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
                    closure: callee,
                    module_idx: c_module_idx,
                    chunk_idx: Some(c_chunk_idx),
                };

                self.frames.push(new_frame);
                Ok(Some(0))
            }
            Instruction::CallSymbol(name_idx, argc) => {
                let name = match self.modules[module_idx].constants.get(name_idx as usize) {
                    Some(Constant::String(s)) => s.as_str(),
                    _ => return Err(VmError::IndexOutOfBounds),
                };

                if let Some(&(m_idx, chunk_idx)) = self.symbol_table.get(name) {
                    let instrs = self.get_chunk_instructions(m_idx, chunk_idx as usize)?;
                    let locals_count =
                        self.modules[m_idx].chunks[chunk_idx as usize].locals as usize;

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
                        closure: Value::null(),
                        module_idx: m_idx,
                        chunk_idx: Some(chunk_idx as usize),
                    };

                    self.frames.push(new_frame);
                    Ok(Some(0))
                } else {
                    return Err(VmError::RuntimeError(format!("Symbol not found: {}", name)));
                }
            }
            Instruction::InvokeMethod(name_idx, argc) => {
                let mut args = Vec::with_capacity(argc as usize);
                for _ in 0..argc {
                    args.push(self.pop()?);
                }
                args.reverse();

                let receiver = self.pop()?;
                let name = match self.modules[module_idx].constants.get(name_idx as usize) {
                    Some(Constant::String(s)) => s.as_str(),
                    _ => return Err(VmError::InvalidOpcode),
                };

                if !receiver.is_object() {
                    self.invoke_primitive_method(receiver, name, args)?;
                } else {
                    // Object method invocation logic...
                    // For now, let's just push null as a placeholder if not handled
                    self.push(Value::null());
                }
                Ok(None)
            }
            _ => Err(VmError::InvalidOpcode),
        }
    }

    fn invoke_primitive_method(
        &mut self,
        receiver: Value,
        name: &str,
        args: Vec<Value>,
    ) -> Result<(), VmError> {
        match name {
            "println" => {
                for arg in args {
                    self.print_line(&arg.to_string());
                }
                self.push(Value::null());
            }
            "add" => {
                if args.len() == 1 {
                    let rhs = args[0];
                    if let (Some(l), Some(r)) = (receiver.try_as_int(), rhs.try_as_int()) {
                        self.push(Value::int(l + r));
                    } else if let (Some(l), Some(r)) = (receiver.try_as_float(), rhs.try_as_float())
                    {
                        self.push(Value::float(l + r));
                    } else if let (Some(l), Some(r)) = (receiver.try_as_str(), rhs.try_as_str()) {
                        let mut s = l.to_string();
                        s.push_str(r);
                        self.push(Value::string(s, &self.gc));
                    } else {
                        self.push(Value::null());
                    }
                } else {
                    self.push(Value::null());
                }
            }
            _ => {
                self.push(Value::null());
            }
        }
        Ok(())
    }
}
