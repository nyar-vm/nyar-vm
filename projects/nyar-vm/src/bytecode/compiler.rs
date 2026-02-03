use crate::bytecode::instruction::Instruction;
use crate::runtime::NyarBuiltin;
use crate::bytecode::format::{Chunk, ClassInfo, Constant, ExportInfo, NyarcModule};
use crate::bytecode::opcode::Opcode;
use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use nyar_types::{NyarError, QualifiedName};

pub struct NyarBackend {
    module: NyarcModule,
    locals: Vec<String>,
}

impl NyarBackend {
    pub fn new() -> Self {
        Self {
            module: NyarcModule::default(),
            locals: Vec::new(),
        }
    }

    fn add_local(&mut self, name: String) -> u8 {
        if let Some(idx) = self.locals.iter().position(|l| l == &name) {
            return idx as u8;
        }
        let idx = self.locals.len() as u8;
        self.locals.push(name);
        idx
    }

    fn find_local(&self, name: &str) -> Option<u8> {
        self.locals.iter().position(|l| l == name).map(|i| i as u8)
    }

    pub fn lower_tree(&mut self, tree: &IKunTree) -> Result<Vec<u8>, NyarError> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Symbol(name) => {
                if let Some(idx) = self.find_local(name) {
                    code.extend_from_slice(&Instruction::LoadLocal(idx).encode());
                } else {
                    let idx = self.add_constant(Constant::String(name.clone()));
                    code.extend_from_slice(&Instruction::LoadGlobal(idx).encode());
                }
            }
            IKunTree::StateUpdate(target, value) => {
                if let IKunTree::Symbol(name) = &**target {
                    code.extend(self.lower_tree(value)?);
                    if let Some(idx) = self.find_local(name) {
                        code.extend_from_slice(&Instruction::StoreLocal(idx).encode());
                    } else {
                        let idx = self.add_constant(Constant::String(name.clone()));
                        code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                    }
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
                println!("DEBUG: Module code generated: {:02X?}", module_code);
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
                        println!("DEBUG: Mapping {}:{}:{} to intrinsic:{}", lang, group, func, id);
                        format!("$intrinsic:{}", id)
                    } else {
                        format!("{}:{}:{}", lang, group, func)
                    }
                } else {
                    format!("{}:{}:{}", lang, group, func)
                };
                let name_idx = self.add_constant(Constant::String(name.clone()));
                println!("DEBUG: Added FFICall constant: {} at index {}", name, name_idx);
                code.extend_from_slice(
                    &Instruction::FFICall(name_idx, args.len() as u8).encode(),
                );
            }
            IKunTree::Extension(name, args) => {
                println!("Backend: Extension {}, args len {}", name, args.len());
                match name.as_str() {
                    "local_variable" => {
                        // [name, type, init]
                        if let IKunTree::StringConstant(name) = &args[0] {
                            if let Some(init) = args.get(2) {
                                code.extend(self.lower_tree(init)?);
                            } else {
                                code.extend_from_slice(&Instruction::I64Const(0).encode());
                            }
                            let local_idx = self.add_local(name.clone());
                            code.extend_from_slice(&Instruction::StoreLocal(local_idx).encode());
                        }
                    }
                    "assign" => {
                        if args.len() == 2 {
                            // [name, value]
                            code.extend(self.lower_tree(&args[1])?);
                            if let IKunTree::Symbol(name) = &args[0] {
                                if let Some(idx) = self.find_local(name) {
                                    code.extend_from_slice(&Instruction::StoreLocal(idx).encode());
                                } else {
                                    let idx = self.add_constant(Constant::String(name.clone()));
                                    code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                                }
                            }
                        }
                    }
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
                            let members_idx = args.len() - 1;
                            if let IKunTree::Seq(members) = &args[members_idx] {
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
                                let prev_locals = self.locals.clone();
                                self.locals.clear();
                                // Add parameters to locals if present
                                if args.len() >= 4 {
                                    if let IKunTree::Seq(params) = &args[2] {
                                        for param in params {
                                            if let IKunTree::Extension(ext_name, ext_args) = param {
                                                if ext_name == "parameter" {
                                                    if let IKunTree::StringConstant(pname) =
                                                        &ext_args[0]
                                                    {
                                                        self.add_local(pname.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                let body_code = self.lower_tree(body)?;
                                let mut final_code = body_code;
                                if final_code.last() != Some(&(Opcode::Return as u8)) {
                                    final_code.push(Opcode::Return as u8);
                                }
                                let chunk_idx = self.module.chunks.len();
                                if chunk_idx >= u16::MAX as usize {
                                    return Err(NyarError::new(
                                        0x1007,
                                        nyar_types::NyarErrorKind::Vm(
                                            nyar_types::VmErrorKind::LimitExceeded,
                                        ),
                                        nyar_types::SourceLocation::default(),
                                    ));
                                }
                                let chunk_idx = chunk_idx as u16;
                                let num_locals = self.locals.len() as u16;
                                self.module.chunks.push(Chunk {
                                    locals: num_locals.max(32),
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
                                self.locals = prev_locals;
                            }
                        }
                    }
                    "lambda" => {
                        if args.len() >= 2 {
                            // [params, body]
                            let body = &args[1];
                            let prev_locals = self.locals.clone();
                            self.locals.clear();
                            if let IKunTree::Seq(params) = &args[0] {
                                for param in params {
                                    if let IKunTree::Symbol(pname) = param {
                                        self.add_local(pname.clone());
                                    }
                                }
                            }
                            let body_code = self.lower_tree(body)?;
                            let mut final_code = body_code;
                            if final_code.last() != Some(&(Opcode::Return as u8)) {
                                final_code.push(Opcode::Return as u8);
                            }
                            let chunk_idx = self.module.chunks.len();
                            if chunk_idx >= u16::MAX as usize {
                                return Err(NyarError::new(
                                    0x1007,
                                    nyar_types::NyarErrorKind::Vm(
                                        nyar_types::VmErrorKind::LimitExceeded,
                                    ),
                                    nyar_types::SourceLocation::default(),
                                ));
                            }
                            let chunk_idx = chunk_idx as u16;
                            let num_locals = self.locals.len() as u16;
                            self.module.chunks.push(Chunk {
                                locals: num_locals.max(32),
                                upvalues: 0,
                                max_stack: 64,
                                code: final_code,
                                handlers: vec![],
                                lines: vec![],
                                decoded: None,
                                hotness: std::sync::atomic::AtomicU32::new(0),
                            });
                            code.extend_from_slice(
                                &Instruction::MakeClosure(chunk_idx, vec![]).encode(),
                            );
                            self.locals = prev_locals;
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
                    "import" => {
                        if let IKunTree::StringConstant(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            code.extend_from_slice(&Instruction::Call(idx, 0).encode());
                        }
                    }
                    "import_from" => {
                        if args.len() == 2 {
                            if let (IKunTree::StringConstant(module), IKunTree::StringConstant(member)) = (&args[0], &args[1]) {
                                let mod_idx = self.add_constant(Constant::String(module.clone()));
                                code.extend_from_slice(&Instruction::Call(mod_idx, 0).encode());
                                let mem_idx = self.add_constant(Constant::String(member.clone()));
                                code.extend_from_slice(&Instruction::GetField(mem_idx).encode());
                            }
                        }
                    }
                    "none" => {}
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
                    "bit_not" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend_from_slice(&Instruction::I64Not.encode());
                    }
                    "bit_shl" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64Shl.encode());
                    }
                    "bit_shr" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend_from_slice(&Instruction::I64ShrS.encode());
                    }
                    "and" => {
                        // a && b
                        // eval a
                        code.extend(self.lower_tree(&args[0])?);
                        // dup for jump
                        code.extend_from_slice(&Instruction::Dup(0).encode());
                        // if false, jump to end (keep false on stack)
                        let placeholder = code.len();
                        code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                        // if true, pop a and eval b
                        code.extend_from_slice(&Instruction::Pop.encode());
                        code.extend(self.lower_tree(&args[1])?);
                        // label end
                        let end_pos = code.len();
                        let off = (end_pos as isize - placeholder as isize) as i16;
                        let instr = Instruction::JumpIfFalse(off).encode();
                        code[placeholder..placeholder + instr.len()].copy_from_slice(&instr);
                    }
                    "or" => {
                        // a || b
                        // eval a
                        code.extend(self.lower_tree(&args[0])?);
                        // dup for jump
                        code.extend_from_slice(&Instruction::Dup(0).encode());
                        // if true, jump to end (keep true on stack)
                        let placeholder = code.len();
                        code.extend_from_slice(&Instruction::JumpIfTrue(0).encode());
                        // if false, pop a and eval b
                        code.extend_from_slice(&Instruction::Pop.encode());
                        code.extend(self.lower_tree(&args[1])?);
                        // label end
                        let end_pos = code.len();
                        let off = (end_pos as isize - placeholder as isize) as i16;
                        let instr = Instruction::JumpIfTrue(off).encode();
                        code[placeholder..placeholder + instr.len()].copy_from_slice(&instr);
                    }
                    "raise" => {
                        // [exc, cause]
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend(self.lower_tree(&args[1])?);
                        let name_idx =
                            self.add_constant(Constant::String("python:raise".to_string()));
                        code.extend_from_slice(&Instruction::Perform(name_idx, 2).encode());
                    }
                    "assert" => {
                        // [test, msg]
                        code.extend(self.lower_tree(&args[0])?);
                        let placeholder = code.len();
                        code.extend_from_slice(&Instruction::JumpIfTrue(0).encode());
                        code.extend(self.lower_tree(&args[1])?);
                        let name_idx = self
                            .add_constant(Constant::String("python:AssertionError".to_string()));
                        code.extend_from_slice(&Instruction::Perform(name_idx, 1).encode());
                        let end_pos = code.len();
                        let off = (end_pos as isize - placeholder as isize) as i16;
                        let instr = Instruction::JumpIfTrue(off).encode();
                        code[placeholder..placeholder + instr.len()].copy_from_slice(&instr);
                    }
                    "try" => {
                        // [body, handlers, orelse, finalbody]
                        let body = &args[0];
                        let handlers = &args[1];
                        let orelse = &args[2];
                        let finalbody = &args[3];

                        // 1. If there's a finalbody, wrap everything in a finally-like handler
                        let has_finally = !matches!(&finalbody, IKunTree::Seq(items) if items.is_empty()) && !matches!(&finalbody, IKunTree::Extension(name, _) if name == "none");
                        
                        if has_finally {
                            let mut finally_handler_code = Vec::new();
                            // Finally handler catches everything, runs finalbody, then re-performs
                            // Handler receives: [effect_obj, args_list, continuation]
                            
                            // Load effect_obj and args_list to re-perform later
                            finally_handler_code.extend_from_slice(&Instruction::LoadLocal(0).encode());
                            finally_handler_code.extend_from_slice(&Instruction::LoadLocal(1).encode());
                            
                            // Run finalbody
                            finally_handler_code.extend(self.lower_tree(finalbody)?);
                            
                            // Re-perform
                            // We need an instruction that can perform with dynamic name and args list
                            // Nyar VM might need a dynamic perform. For now, let's assume it's python:raise
                            let re_perform_idx = self.add_constant(Constant::String("python:raise".to_string()));
                            finally_handler_code.extend_from_slice(&Instruction::Perform(re_perform_idx, 2).encode());
                            finally_handler_code.push(Opcode::Return as u8);

                            let finally_chunk_idx = self.module.chunks.len() as u16;
                            self.module.chunks.push(Chunk {
                                locals: 3,
                                upvalues: 0,
                                max_stack: 64,
                                code: finally_handler_code,
                                handlers: vec![],
                                lines: vec![],
                                decoded: None,
                                hotness: std::sync::atomic::AtomicU32::new(0),
                            });
                            code.extend_from_slice(&Instruction::WithHandler(finally_chunk_idx).encode());
                        }

                        // 2. Wrap body in except handler
                        let mut except_handler_code = Vec::new();
                        let raise_name_idx = self.add_constant(Constant::String("python:raise".to_string()));
                        except_handler_code.extend_from_slice(&Instruction::MatchEffect(raise_name_idx).encode());
                        
                        let next_handler_placeholder = except_handler_code.len();
                        except_handler_code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());

                        // Match! [exc, cause] are on stack
                        except_handler_code.extend(self.lower_tree(handlers)?);
                        
                        // If no except matched, re-perform
                        let re_perform_idx = self.add_constant(Constant::String("python:raise".to_string()));
                        except_handler_code.extend_from_slice(&Instruction::Perform(re_perform_idx, 2).encode());

                        let handler_end = except_handler_code.len();
                        let off = (handler_end as isize - next_handler_placeholder as isize) as i16;
                        let instr = Instruction::JumpIfFalse(off).encode();
                        except_handler_code[next_handler_placeholder..next_handler_placeholder + instr.len()].copy_from_slice(&instr);
                        except_handler_code.push(Opcode::Return as u8);

                        let except_chunk_idx = self.module.chunks.len() as u16;
                        self.module.chunks.push(Chunk {
                            locals: 3,
                            upvalues: 0,
                            max_stack: 64,
                            code: except_handler_code,
                            handlers: vec![],
                            lines: vec![],
                            decoded: None,
                            hotness: std::sync::atomic::AtomicU32::new(0),
                        });

                        code.extend_from_slice(&Instruction::WithHandler(except_chunk_idx).encode());
                        code.extend(self.lower_tree(body)?);
                        code.extend(self.lower_tree(orelse)?);
                        
                        // End of WithHandler(except)
                        code.push(Opcode::Pop as u8); // Pop the handler if finished normally? 
                        // Actually WithHandler might need an explicit end or it ends with the scope.
                        
                        if has_finally {
                            code.extend(self.lower_tree(finalbody)?);
                        }
                    }
                    "except" => {
                        // [type, name, body]
                        // Currently on stack: [exc, cause]
                        
                        // 1. Check type if provided
                        let has_type = !matches!(&args[0], IKunTree::Extension(name, _) if name == "none");
                        let mut jump_placeholder = None;
                        
                        if has_type {
                            code.extend_from_slice(&Instruction::Dup(1).encode()); // Dup exc
                            code.extend(self.lower_tree(&args[0])?); // type
                            code.extend_from_slice(&Instruction::InstanceOf(0).encode());
                            
                            let placeholder = code.len();
                            code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                            jump_placeholder = Some(placeholder);
                        }
                        
                        // 2. Assign name if present
                        if let IKunTree::Symbol(name) = &args[1] {
                             let name_idx = self.add_constant(Constant::String(name.clone()));
                             code.extend_from_slice(&Instruction::Dup(1).encode()); // Dup exc
                             code.extend_from_slice(&Instruction::StoreGlobal(name_idx).encode());
                        }
                        
                        // 3. Execute body
                        code.extend(self.lower_tree(&args[2])?);
                        
                        // 4. Return from handler (handled!)
                        code.push(Opcode::Return as u8);

                        if let Some(placeholder) = jump_placeholder {
                            let end_pos = code.len();
                            let off = (end_pos as isize - placeholder as isize) as i16;
                            let instr = Instruction::JumpIfFalse(off).encode();
                            code[placeholder..placeholder + instr.len()].copy_from_slice(&instr);
                        }
                    }
                    "with" => {
                        // [items, body]
                        if let IKunTree::Seq(items) = &args[0] {
                            for (i, item) in items.iter().enumerate() {
                                if let IKunTree::Extension(name, item_args) = item {
                                    if name == "with_item" {
                                        // [ctx_expr, var_node]
                                        code.extend(self.lower_tree(&item_args[0])?);
                                        // Store context object in a temporary global/local for __exit__
                                        let temp_name = format!("$with_ctx_{}", i);
                                        let temp_idx = self.add_constant(Constant::String(temp_name));
                                        code.extend_from_slice(&Instruction::Dup(0).encode());
                                        code.extend_from_slice(&Instruction::StoreGlobal(temp_idx).encode());
                                        
                                        // Call __enter__
                                        let enter_idx = self.add_constant(Constant::String("__enter__".to_string()));
                                        code.extend_from_slice(&Instruction::InvokeMethod(enter_idx, 0).encode());
                                        
                                        if let IKunTree::Symbol(var_name) = &item_args[1] {
                                            let var_idx = self.add_constant(Constant::String(var_name.clone()));
                                            code.extend_from_slice(&Instruction::StoreGlobal(var_idx).encode());
                                        } else {
                                            code.extend_from_slice(&Instruction::Pop.encode());
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Wrap body in a finally-like block to call __exit__
                        let mut exit_code = Vec::new();
                        if let IKunTree::Seq(items) = &args[0] {
                            for (i, _) in items.iter().enumerate().rev() {
                                let temp_name = format!("$with_ctx_{}", i);
                                let temp_idx = self.add_constant(Constant::String(temp_name));
                                exit_code.extend_from_slice(&Instruction::LoadGlobal(temp_idx).encode());
                                let exit_idx = self.add_constant(Constant::String("__exit__".to_string()));
                                // Call __exit__(None, None, None)
                                exit_code.extend_from_slice(&Instruction::Push(self.add_constant(Constant::String("None".to_string()))).encode());
                                exit_code.extend_from_slice(&Instruction::Push(self.add_constant(Constant::String("None".to_string()))).encode());
                                exit_code.extend_from_slice(&Instruction::Push(self.add_constant(Constant::String("None".to_string()))).encode());
                                exit_code.extend_from_slice(&Instruction::InvokeMethod(exit_idx, 3).encode());
                                exit_code.extend_from_slice(&Instruction::Pop.encode());
                            }
                        }

                        // For simplicity, we don't handle exceptions in 'with' perfectly yet,
                        // but we run exit_code after body.
                        code.extend(self.lower_tree(&args[1])?);
                        code.extend(exit_code);
                    }
                    "choice" => {
                        // condition ? then : else
                        // args[0] = cond, args[1] = then, args[2] = else
                        code.extend(self.lower_tree(&args[0])?);
                        
                        let jump_false_placeholder = code.len();
                        code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                        
                        code.extend(self.lower_tree(&args[1])?);
                        let jump_end_placeholder = code.len();
                        code.extend_from_slice(&Instruction::Jump(0).encode());
                        
                        let else_start = code.len();
                        let else_offset = (else_start as isize - jump_false_placeholder as isize) as i16;
                        let jump_false_instr = Instruction::JumpIfFalse(else_offset).encode();
                        code[jump_false_placeholder..jump_false_placeholder + jump_false_instr.len()].copy_from_slice(&jump_false_instr);
                        
                        code.extend(self.lower_tree(&args[2])?);
                        let end_pos = code.len();
                        let end_offset = (end_pos as isize - jump_end_placeholder as isize) as i16;
                        let jump_end_instr = Instruction::Jump(end_offset).encode();
                        code[jump_end_placeholder..jump_end_placeholder + jump_end_instr.len()].copy_from_slice(&jump_end_instr);
                    }
                    "sizeof" => {
                        code.extend(self.lower_tree(&args[0])?);
                        code.extend_from_slice(&Instruction::SizeOf.encode());
                    }
                    "cast" => {
                        // (type)expr
                        // args[0] is target type (symbol or string), args[1] is expr
                        code.extend(self.lower_tree(&args[1])?);
                        if let IKunTree::Symbol(type_name) = &args[0] {
                            let idx = self.add_constant(Constant::String(type_name.clone()));
                            code.extend_from_slice(&Instruction::Cast(idx).encode());
                        } else if let IKunTree::StringConstant(type_name) = &args[0] {
                            let idx = self.add_constant(Constant::String(type_name.clone()));
                            code.extend_from_slice(&Instruction::Cast(idx).encode());
                        }
                    }
                    "inc_pre" => {
                        if let IKunTree::Symbol(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            code.extend_from_slice(&Instruction::LoadGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::I64Const(1).encode());
                            code.extend_from_slice(&Instruction::I64Add.encode());
                            code.extend_from_slice(&Instruction::Dup(0).encode());
                            code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::Pop.encode());
                        }
                    }
                    "inc_post" => {
                        if let IKunTree::Symbol(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            code.extend_from_slice(&Instruction::LoadGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::Dup(0).encode());
                            code.extend_from_slice(&Instruction::I64Const(1).encode());
                            code.extend_from_slice(&Instruction::I64Add.encode());
                            code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::Pop.encode());
                        }
                    }
                    "dec_pre" => {
                        if let IKunTree::Symbol(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            code.extend_from_slice(&Instruction::LoadGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::I64Const(1).encode());
                            code.extend_from_slice(&Instruction::I64Sub.encode());
                            code.extend_from_slice(&Instruction::Dup(0).encode());
                            code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::Pop.encode());
                        }
                    }
                    "dec_post" => {
                        if let IKunTree::Symbol(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            code.extend_from_slice(&Instruction::LoadGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::Dup(0).encode());
                            code.extend_from_slice(&Instruction::I64Const(1).encode());
                            code.extend_from_slice(&Instruction::I64Sub.encode());
                            code.extend_from_slice(&Instruction::StoreGlobal(idx).encode());
                            code.extend_from_slice(&Instruction::Pop.encode());
                        }
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

impl Backend for NyarBackend {
    fn name(&self) -> &str {
        "nyar-vm"
    }

    fn generate(&self, tree: &IKunTree) -> chomsky_types::ChomskyResult<BackendArtifact> {
        let mut this = NyarBackend::new();
        let code = this.lower_tree(tree).map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("{:?}", e))
        })?;
        if !code.is_empty() {
            let mut final_code = code;
            if final_code.last() != Some(&(Opcode::Return as u8)) {
                final_code.push(Opcode::Return as u8);
            }
            this.module.chunks.push(Chunk {
                locals: 32,
                upvalues: 0,
                max_stack: 64,
                code: final_code,
                handlers: vec![],
                lines: vec![],
                decoded: None,
                hotness: std::sync::atomic::AtomicU32::new(0),
            });
        }
        Ok(BackendArtifact::Binary(this.module.encode()))
    }
}
