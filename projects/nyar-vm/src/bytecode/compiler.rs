use crate::bytecode::instruction::Instruction;
use crate::runtime::NyarBuiltin;
use crate::bytecode::format::{Chunk, ClassInfo, Constant, ExportInfo, NyarcModule};
use crate::bytecode::opcode::Opcode;
use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use nyar_types::{NyarError, QualifiedName};

pub struct NyarBackend {
    module: NyarcModule,
}

impl NyarBackend {
    pub fn new() -> Self {
        Self {
            module: NyarcModule::default(),
        }
    }

    pub fn lower_tree(&mut self, tree: &IKunTree) -> Result<Vec<u8>, NyarError> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Symbol(name) => {
                let idx = self.add_constant(Constant::String(name.clone()));
                code.extend_from_slice(&Instruction::LoadGlobal(idx).encode());
            }
            IKunTree::StateUpdate(target, value) => {
                if let IKunTree::Symbol(name) = &**target {
                    code.extend(self.lower_tree(value)?);
                    let idx = self.add_constant(Constant::String(name.clone()));
                    code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                }
            }
            IKunTree::Apply(callee, args) => {
                for arg in args {
                    code.extend(self.lower_tree(arg)?);
                }
                if let IKunTree::Symbol(name) = &**callee {
                    let idx = self.add_constant(Constant::String(name.clone()));
                    code.extend_from_slice(&Instruction::Call(idx, args.len() as u8).encode());
                } else {
                    code.extend(self.lower_tree(callee)?);
                    code.extend_from_slice(&Instruction::CallClosure(args.len() as u8).encode());
                }
            }
            IKunTree::Lambda(params, body) => {
                let body_code = self.lower_tree(body)?;
                let mut final_code = body_code;
                if final_code.last() != Some(&(Opcode::Return as u8)) {
                    final_code.push(Opcode::Return as u8);
                }
                let chunk_idx = self.module.chunks.len();
                if chunk_idx >= u16::MAX as usize {
                    return Err(NyarError::new(
                        0x1007,
                        nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                        nyar_types::SourceLocation::default(),
                    ));
                }
                let chunk_idx = chunk_idx as u16;
                self.module.chunks.push(Chunk {
                    locals: params.len() as u16,
                    upvalues: 0,
                    max_stack: 64,
                    code: final_code,
                    handlers: vec![],
                    lines: vec![],
                    decoded: None,
                    hotness: std::sync::atomic::AtomicU32::new(0),
                });
                code.extend_from_slice(&Instruction::MakeClosure(chunk_idx, vec![]).encode());
            }
            IKunTree::Seq(items) => {
                for item in items {
                    code.extend(self.lower_tree(item)?);
                }
            }
            IKunTree::Constant(v) => {
                code.extend_from_slice(&Instruction::I64Const(*v).encode());
            }
            IKunTree::FloatConstant(v) => {
                code.extend_from_slice(&Instruction::F64Const(f64::from_bits(*v)).encode());
            }
            IKunTree::BooleanConstant(v) => {
                let idx = self.add_constant(Constant::Int(if *v { 1 } else { 0 }));
                code.extend_from_slice(&Instruction::Push(idx).encode());
            }
            IKunTree::StringConstant(v) => {
                code.extend_from_slice(&Instruction::StringConst(v.clone()).encode());
            }
            IKunTree::Module(_name, items) => {
                let mut module_code = Vec::new();
                for item in items {
                    module_code.extend(self.lower_tree(item)?);
                }
                if !module_code.is_empty() {
                    if module_code.last() != Some(&(Opcode::Return as u8)) {
                        module_code.push(Opcode::Return as u8);
                    }
                    self.module.chunks.push(Chunk {
                        locals: 32,
                        upvalues: 0,
                        max_stack: 64,
                        code: module_code,
                        handlers: vec![],
                        lines: vec![],
                        decoded: None,
                        hotness: std::sync::atomic::AtomicU32::new(0),
                    });
                }
            }
            IKunTree::Export(name, body) => {
                if let IKunTree::Lambda(_params, body) = &**body {
                    let body_code = self.lower_tree(body)?;
                    let mut final_code = body_code;
                    // Ensure Return at the end
                    if final_code.last() != Some(&(Opcode::Return as u8)) {
                        final_code.push(Opcode::Return as u8);
                    }

                    let chunk_idx = self.module.chunks.len();
                    if chunk_idx >= u16::MAX as usize {
                        return Err(NyarError::new(
                            0x1007,
                            nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                            nyar_types::SourceLocation::default(),
                        ));
                    }
                    let chunk_idx = chunk_idx as u16;
                    self.module.chunks.push(Chunk {
                        locals: 32,
                        upvalues: 0,
                        max_stack: 64,
                        code: final_code,
                        handlers: vec![],
                        lines: vec![],
                        decoded: None,
                        hotness: std::sync::atomic::AtomicU32::new(0),
                    });
                    self.module.exports.push(ExportInfo {
                        symbol: QualifiedName::from(name.as_str()),
                        chunk_idx,
                    });
                }
            }
            IKunTree::Return(val) => {
                if let Some(code_tail) = self.lower_tail_call(val)? {
                    code.extend(code_tail);
                } else {
                    code.extend(self.lower_tree(val)?);
                    code.push(Opcode::Return as u8);
                }
            }
            IKunTree::Choice(cond, then_branch, else_branch) => {
                // 1. Evaluate condition
                code.extend(self.lower_tree(cond)?);
                
                // 2. Placeholder for JumpIfFalse to else_branch
                let jump_false_placeholder = code.len();
                code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                
                // 3. Evaluate then_branch
                let then_code = self.lower_tree(then_branch)?;
                code.extend(then_code);
                
                // 4. Placeholder for Jump to end
                let jump_end_placeholder = code.len();
                code.extend_from_slice(&Instruction::Jump(0).encode());
                
                // 5. Fill JumpIfFalse target
                let else_start = code.len();
                let else_offset = (else_start as isize - jump_false_placeholder as isize) as i16;
                let jump_false_instr = Instruction::JumpIfFalse(else_offset).encode();
                code[jump_false_placeholder..jump_false_placeholder + jump_false_instr.len()].copy_from_slice(&jump_false_instr);
                
                // 6. Evaluate else_branch
                let else_code = self.lower_tree(else_branch)?;
                code.extend(else_code);
                
                // 7. Fill Jump target
                let end_pos = code.len();
                let end_offset = (end_pos as isize - jump_end_placeholder as isize) as i16;
                let jump_end_instr = Instruction::Jump(end_offset).encode();
                code[jump_end_placeholder..jump_end_placeholder + jump_end_instr.len()].copy_from_slice(&jump_end_instr);
            }
            IKunTree::CrossLangCall {
                language: lang,
                module_path: group,
                function_name: func,
                arguments: args,
            } => {
                for arg in args {
                    code.extend(self.lower_tree(arg)?);
                }
                let name = if lang == "nyar" {
                    // Try to map to intrinsic ID
                    let id = match (group.as_str(), func.as_str()) {
                        ("io", "print") | ("", "print") => NyarBuiltin::Print as u32,
                        ("io", "println") | ("", "println") => NyarBuiltin::Println as u32,
                        ("std", "exit") | ("", "exit") => NyarBuiltin::Exit as u32,
                        ("time", "now") | ("", "get_time") => NyarBuiltin::GetTime as u32,
                        ("time", "sleep") | ("", "sleep") => NyarBuiltin::Sleep as u32,
                        ("ops", "add") | ("", "native_add") => NyarBuiltin::NativeAdd as u32,
                        ("std", "panic") | ("", "panic") => NyarBuiltin::Panic as u32,
                        ("math", "sin") | ("", "sin") => NyarBuiltin::MathSin as u32,
                        ("math", "sqrt") | ("", "sqrt") => NyarBuiltin::MathSqrt as u32,
                        ("mem", "alloc") | ("", "alloc") => NyarBuiltin::MemAlloc as u32,
                        ("math", "abs") | ("", "abs") => NyarBuiltin::MathAbs as u32,
                        ("math", "cos") | ("", "cos") => NyarBuiltin::MathCos as u32,
                        ("math", "tan") | ("", "tan") => NyarBuiltin::MathTan as u32,
                        ("ops", "bit_and") | ("", "bit_and") => NyarBuiltin::BitAnd as u32,
                        ("ops", "bit_or") | ("", "bit_or") => NyarBuiltin::BitOr as u32,
                        ("ops", "bit_xor") | ("", "bit_xor") => NyarBuiltin::BitXor as u32,
                        ("ops", "bit_not") | ("", "bit_not") => NyarBuiltin::BitNot as u32,
                        ("ops", "bit_shl") | ("", "bit_shl") => NyarBuiltin::BitShl as u32,
                        ("ops", "bit_shr") | ("", "bit_shr") => NyarBuiltin::BitShr as u32,
                        ("mem", "free") => NyarBuiltin::MemFree as u32,
                        ("mem", "realloc") => NyarBuiltin::MemRealloc as u32,
                        ("mem", "set") => NyarBuiltin::MemSet as u32,
                        ("mem", "copy") => NyarBuiltin::MemCopy as u32,
                        ("str", "len") => NyarBuiltin::StrLen as u32,
                        ("str", "cmp") => NyarBuiltin::StrCmp as u32,
                        ("math", "rand") => NyarBuiltin::MathRand as u32,
                        _ => 0,
                    };
                    if id > 0 {
                        format!("$intrinsic:{}", id)
                    } else {
                        format!("{}:{}:{}", lang, group, func)
                    }
                } else {
                    format!("{}:{}:{}", lang, group, func)
                };
                let name_idx = self.add_constant(Constant::String(name));
                code.extend_from_slice(
                    &Instruction::FFICall(name_idx, args.len() as u8).encode(),
                );
            }
            IKunTree::Extension(name, args) => {
                println!("Backend: Extension {}, args len {}", name, args.len());
                match name.as_str() {
                    "return" => {
                        if let Some(val) = args.first() {
                            if let Some(code_tail) = self.lower_tail_call(val)? {
                                code.extend(code_tail);
                            } else {
                                code.extend(self.lower_tree(val)?);
                                code.push(Opcode::Return as u8);
                            }
                        } else {
                            code.push(Opcode::Return as u8);
                        }
                    }
                    "class" => {
                        if let IKunTree::StringConstant(class_name) = &args[0] {
                            let mut fields = Vec::new();
                            if let IKunTree::Seq(members) = &args[1] {
                                for member in members {
                                    if let IKunTree::Extension(ext_name, ext_args) = member {
                                        if ext_name == "field" {
                                            if let IKunTree::StringConstant(field_name) = &ext_args[0] {
                                                fields.push(field_name.clone());
                                            }
                                        }
                                    }
                                    self.lower_tree(member)?;
                                }
                            }
                            self.add_class(class_name.clone(), fields);
                        }
                    }
                    "new" => {
                        if let IKunTree::StringConstant(class_name) = &args[0] {
                            let qn = QualifiedName::from(class_name.as_str());
                            let idx = self.module.classes.iter().position(|c| c.name == qn).map(|i| i as u16).ok_or_else(|| {
                                NyarError::new(
                                    0x1002,
                                    nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::IndexOutOfBounds(0)),
                                    nyar_types::SourceLocation::default(),
                                )
                            })?;

                            if let IKunTree::Seq(params) = &args[1] {
                                for param in params {
                                    code.extend(self.lower_tree(param)?);
                                }
                            }
                            code.extend_from_slice(&Instruction::NewObject(idx).encode());
                        }
                    }
                    "method" => {
                        if args.len() >= 3 {
                            let name_idx = 0;
                            let body_idx = args.len() - 1; // Last arg is body
                            if let (IKunTree::StringConstant(name), body) =
                                (&args[name_idx], &args[body_idx])
                            {
                                let body_code = self.lower_tree(body)?;
                                let mut final_code = body_code;
                                if final_code.last() != Some(&(Opcode::Return as u8)) {
                                    final_code.push(Opcode::Return as u8);
                                }
                                let chunk_idx = self.module.chunks.len();
                                if chunk_idx >= u16::MAX as usize {
                                    return Err(NyarError::new(
                                        0x1007,
                                        nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                                        nyar_types::SourceLocation::default(),
                                    ));
                                }
                                let chunk_idx = chunk_idx as u16;
                                self.module.chunks.push(Chunk {
                                    locals: 32,
                                    upvalues: 0,
                                    max_stack: 64,
                                    code: final_code,
                                    handlers: vec![],
                                    lines: vec![],
                                    decoded: None,
                                    hotness: std::sync::atomic::AtomicU32::new(0),
                                });
                                self.module.exports.push(ExportInfo {
                                    symbol: QualifiedName::from(name.as_str()),
                                    chunk_idx,
                                });
                            }
                        }
                    }
                    "lambda" => {
                        if args.len() >= 2 {
                            // [params, body]
                            let body = &args[1];
                            let body_code = self.lower_tree(body)?;
                            let mut final_code = body_code;
                            if final_code.last() != Some(&(Opcode::Return as u8)) {
                                final_code.push(Opcode::Return as u8);
                            }
                            let chunk_idx = self.module.chunks.len();
                            if chunk_idx >= u16::MAX as usize {
                                return Err(NyarError::new(
                                    0x1007,
                                    nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                                    nyar_types::SourceLocation::default(),
                                ));
                            }
                            let chunk_idx = chunk_idx as u16;
                            self.module.chunks.push(Chunk {
                                locals: 32,
                                upvalues: 0,
                                max_stack: 64,
                                code: final_code,
                                handlers: vec![],
                                lines: vec![],
                                decoded: None,
                                hotness: std::sync::atomic::AtomicU32::new(0),
                            });
                            code.extend_from_slice(&Instruction::MakeClosure(chunk_idx, vec![]).encode());
                        }
                    }
                    "call" => {
                        if args.len() == 3 {
                            // [target, name, args]
                            code.extend(self.lower_tree(&args[0])?); // target
                            if let IKunTree::Symbol(name) = &args[1] {
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                if let IKunTree::Seq(call_args) = &args[2] {
                                    for arg in call_args {
                                        code.extend(self.lower_tree(arg)?);
                                    }
                                    code.extend_from_slice(
                                        &Instruction::InvokeMethod(name_idx, call_args.len() as u8)
                                            .encode(),
                                    );
                                }
                            }
                        } else if args.len() == 2 {
                            // [name, args]
                            if let IKunTree::Symbol(name) = &args[0] {
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                if let IKunTree::Seq(call_args) = &args[1] {
                                    for arg in call_args {
                                        code.extend(self.lower_tree(arg)?);
                                    }
                                    code.extend_from_slice(
                                        &Instruction::Call(name_idx, call_args.len() as u8).encode(),
                                    );
                                }
                            }
                        }
                    }
                    "get_field" => {
                        if args.len() == 2 {
                            // [target, name]
                            code.extend(self.lower_tree(&args[0])?);
                            if let IKunTree::Symbol(name) = &args[1] {
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                code.extend_from_slice(&Instruction::GetField(name_idx).encode());
                            }
                        }
                    }
                    "assign" => {
                        if args.len() == 2 {
                            // [name, value]
                            code.extend(self.lower_tree(&args[1])?);
                            if let IKunTree::Symbol(name) = &args[0] {
                                let idx = self.add_constant(Constant::String(name.clone()));
                                code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                            }
                        }
                    }
                    "add" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Add.encode());
                    }
                    "sub" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Sub.encode());
                    }
                    "mul" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Mul.encode());
                    }
                    "div" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64DivS.encode());
                    }
                    "rem" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64RemS.encode());
                    }
                    "eq" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Eq.encode());
                    }
                    "ne" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Ne.encode());
                    }
                    "lt" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64LtS.encode());
                    }
                    "le" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64LeS.encode());
                    }
                    "gt" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64GtS.encode());
                    }
                    "ge" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64GeS.encode());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(code)
    }

    fn lower_tail_call(&mut self, tree: &IKunTree) -> Result<Option<Vec<u8>>, NyarError> {
        match tree {
            IKunTree::Apply(callee, args) => {
                let mut code = Vec::new();
                for arg in args {
                    code.extend(self.lower_tree(arg)?);
                }
                if let IKunTree::Symbol(name) = &**callee {
                    let idx = self.add_constant(Constant::String(name.clone()));
                    code.extend_from_slice(&Instruction::TailCall(idx, args.len() as u8).encode());
                } else {
                    code.extend(self.lower_tree(callee)?);
                    code.extend_from_slice(&Instruction::TailCallClosure(args.len() as u8).encode());
                }
                Ok(Some(code))
            }
            _ => Ok(None),
        }
    }

    fn add_constant(&mut self, constant: Constant) -> u16 {
        if let Some(pos) = self.module.constants.iter().position(|c| c == &constant) {
            pos as u16
        } else {
            let pos = self.module.constants.len() as u16;
            self.module.constants.push(constant);
            pos
        }
    }

    fn add_class(&mut self, name: String, fields: Vec<String>) {
        let qn = QualifiedName::from(name.as_str());
        if !self.module.classes.iter().any(|c| c.name == qn) {
            self.module.classes.push(ClassInfo {
                name: qn,
                fields,
            });
        }
    }

    pub fn finish(self) -> NyarcModule {
        self.module
    }
}
