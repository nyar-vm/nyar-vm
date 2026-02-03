use crate::bytecode::instruction::Instruction;
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
        println!("Backend: lowering tree");
        let mut code = Vec::new();
        match tree {
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
            IKunTree::Constant(v) => {
                code.extend_from_slice(&Instruction::I64Const(*v).encode());
            }
            IKunTree::StringConstant(s) => {
                code.extend_from_slice(&Instruction::StringConst(s.clone()).encode());
            }
            IKunTree::Symbol(s) => {
                code.push(Opcode::LoadGlobal as u8);
                let idx = self.add_constant(Constant::QualifiedName(QualifiedName::from(s.as_str())));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::Return(val) => {
                if let Some(code_tail) = self.lower_tail_call(val)? {
                    code.extend(code_tail);
                } else {
                    code.extend(self.lower_tree(val)?);
                    code.push(Opcode::Return as u8);
                }
            }
            IKunTree::Seq(items) => {
                for item in items {
                    code.extend(self.lower_tree(item)?);
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
                        ("io", "print") | ("", "print") => 1,
                        ("io", "println") | ("", "println") => 2,
                        ("std", "exit") | ("", "exit") => 3,
                        ("time", "now") | ("", "get_time") => 4,
                        ("time", "sleep") | ("", "sleep") => 5,
                        ("ops", "add") | ("", "native_add") => 6,
                        ("std", "panic") | ("", "panic") => 7,
                        ("math", "sin") | ("", "sin") => 8,
                        ("math", "sqrt") | ("", "sqrt") => 9,
                        ("mem", "alloc") | ("", "alloc") => 10,
                        ("math", "abs") | ("", "abs") => 11,
                        ("math", "cos") | ("", "cos") => 12,
                        ("math", "tan") | ("", "tan") => 13,
                        ("ops", "bit_and") | ("", "bit_and") => 14,
                        ("ops", "bit_or") | ("", "bit_or") => 15,
                        ("ops", "bit_xor") | ("", "bit_xor") => 16,
                        ("ops", "bit_not") | ("", "bit_not") => 17,
                        ("ops", "bit_shl") | ("", "bit_shl") => 18,
                        ("ops", "bit_shr") | ("", "bit_shr") => 19,
                        ("mem", "free") => 20,
                        ("mem", "realloc") => 21,
                        ("mem", "set") => 22,
                        ("mem", "copy") => 23,
                        ("str", "len") => 24,
                        ("str", "cmp") => 25,
                        ("math", "rand") => 26,
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
                                let idx = self.add_constant(Constant::QualifiedName(QualifiedName::from(name.as_str())));
                                code.push(Opcode::StoreGlobal as u8);
                                code.extend_from_slice(&(idx as u16).to_le_bytes());
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
                    "and" => {
                        // short-circuit: a && b
                        // eval a
                        code.extend(self.lower_tree(&args[0])?);
                        // dup
                        code.extend_from_slice(&Instruction::Dup(0).encode());
                        // jump if false to end
                        let jump_placeholder = code.len();
                        code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                        // pop (a)
                        code.extend_from_slice(&Instruction::Pop.encode());
                        // eval b
                        code.extend(self.lower_tree(&args[1])?);
                        // label end:
                        let end_pos = code.len();
                        let offset = (end_pos as isize - jump_placeholder as isize) as i16;
                        let jump_instr = Instruction::JumpIfFalse(offset).encode();
                        code[jump_placeholder..jump_placeholder + jump_instr.len()].copy_from_slice(&jump_instr);
                    }
                    "or" => {
                        // short-circuit: a || b
                        // eval a
                        code.extend(self.lower_tree(&args[0])?);
                        // dup
                        code.extend_from_slice(&Instruction::Dup(0).encode());
                        // jump if true to end
                        let jump_placeholder = code.len();
                        code.extend_from_slice(&Instruction::JumpIfTrue(0).encode());
                        // pop (a)
                        code.extend_from_slice(&Instruction::Pop.encode());
                        // eval b
                        code.extend(self.lower_tree(&args[1])?);
                        // label end:
                        let end_pos = code.len();
                        let offset = (end_pos as isize - jump_placeholder as isize) as i16;
                        let jump_instr = Instruction::JumpIfTrue(offset).encode();
                        code[jump_placeholder..jump_placeholder + jump_instr.len()].copy_from_slice(&jump_instr);
                    }
                    "bit_and" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64And.encode());
                    }
                    "bit_or" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Or.encode());
                    }
                    "bit_xor" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Xor.encode());
                    }
                    "shl" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Shl.encode());
                    }
                    "shr" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64ShrS.encode());
                    }
                    "ushr" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64ShrU.encode());
                    }
                    "neg" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend_from_slice(&Instruction::I64Neg.encode());
                    }
                    "not" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend_from_slice(&Instruction::I64Not.encode());
                    }
                    "bit_not" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend_from_slice(&Instruction::I64Not.encode());
                    }
                    "break" => {
                        // For now just halt or nop, or implement proper jump
                        code.push(Opcode::Halt as u8);
                    }
                    "continue" => {
                        code.push(Opcode::Nop as u8);
                    }
                    _ => {
                        // Handle other extensions or fallback
                    }
                }
            }
            _ => {}
        }
        Ok(code)
    }

    fn add_constant(&mut self, c: Constant) -> u16 {
        if let Some(pos) = self.module.constants.iter().position(|x| x == &c) {
            pos as u16
        } else {
            let idx = self.module.constants.len();
            if idx >= u16::MAX as usize {
                panic!("Constant pool overflow");
            }
            let idx = idx as u16;
            self.module.constants.push(c);
            idx
        }
    }

    fn add_class(&mut self, name: String, fields: Vec<String>) -> u16 {
        let qn = QualifiedName::from(name.as_str());
        if let Some(pos) = self.module.classes.iter().position(|x| x.name == qn) {
            pos as u16
        } else {
            let idx = self.module.classes.len();
            if idx >= u16::MAX as usize {
                panic!("Class pool overflow");
            }
            let idx = idx as u16;
            self.module.classes.push(ClassInfo { name: qn, fields });
            idx
        }
    }
    pub fn finish(self) -> NyarcModule {
        self.module
    }

    fn lower_tail_call(&mut self, tree: &IKunTree) -> Result<Option<Vec<u8>>, NyarError> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Extension(name, args) if name == "call" => {
                if args.len() == 3 {
                    // [target, name, args]
                    code.extend(self.lower_tree(&args[0])?); // callee
                    if let IKunTree::Symbol(name) = &args[1] {
                        let name_idx = self.add_constant(Constant::String(name.clone()));
                        code.push(Opcode::LoadGlobal as u8);
                        code.extend_from_slice(&(name_idx as u16).to_le_bytes());
                    }
                    if let IKunTree::Seq(call_args) = &args[2] {
                        for arg in call_args {
                            code.extend(self.lower_tree(arg)?);
                        }
                        // Currently we don't have InvokeMethodTail, so we just use regular call for methods
                        // Or we could implement it. But let's stick to simple tail calls for now.
                        return Ok(None);
                    }
                } else if args.len() == 2 {
                    // [name, args]
                    if let IKunTree::Symbol(name) = &args[0] {
                        let name_idx = self.add_constant(Constant::String(name.clone()));
                        // To do a tail call, we first need to load the closure
                        code.push(Opcode::LoadGlobal as u8);
                        code.extend_from_slice(&(name_idx as u16).to_le_bytes());

                        if let IKunTree::Seq(call_args) = &args[1] {
                            for arg in call_args {
                                code.extend(self.lower_tree(arg)?);
                            }
                            code.extend_from_slice(&Instruction::TailCall(call_args.len() as u8).encode());
                            return Ok(Some(code));
                        }
                    }
                }
            }
            IKunTree::CrossLangCall {
                language,
                module_path,
                function_name,
                arguments,
            } if language == "nyar" => {
                // Similar logic for nyar cross-lang calls
                let name = format!("{}:{}:{}", language, module_path, function_name);
                let name_idx = self.add_constant(Constant::String(name));
                code.push(Opcode::LoadGlobal as u8);
                code.extend_from_slice(&(name_idx as u16).to_le_bytes());

                for arg in arguments {
                    code.extend(self.lower_tree(arg)?);
                }
                code.extend_from_slice(&Instruction::TailCall(arguments.len() as u8).encode());
                return Ok(Some(code));
            }
            _ => {}
        }
        Ok(None)
    }
}

impl Backend for NyarBackend {
    fn name(&self) -> &str {
        "nyar"
    }

    fn get_model(&self) -> &dyn chomsky_cost::CostModel {
        &chomsky_cost::DEFAULT_COST_MODEL
    }

    fn generate(&self, tree: &IKunTree) -> Result<BackendArtifact, chomsky_types::ChomskyError> {
        let mut backend = NyarBackend::new();
        backend
            .lower_tree(tree)
            .map_err(|e| chomsky_types::ChomskyError::backend_error(format!("{:?}", e)))?;
        let module = backend.finish();

        let json = serde_json::to_string_pretty(&module)
            .map_err(|e| chomsky_types::ChomskyError::backend_error(e.to_string()))?;
        Ok(BackendArtifact::Source(json))
    }
}
