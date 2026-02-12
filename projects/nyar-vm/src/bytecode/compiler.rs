use crate::bytecode::instruction::Instruction;
use crate::bytecode::opcode::{NyarBuiltin, Opcode};
use crate::bytecode::format::{Chunk, ClassInfo, Constant, ExportInfo, NyarcModule};
use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use chomsky_types::LineMap;
use nyar_types::{NyarError, QualifiedName, SourceLocation};

pub struct NyarBackend {
    module: NyarcModule,
    locals: Vec<String>,
    classes: std::collections::HashMap<String, Vec<String>>,
    current_class: Option<String>,
    current_location: SourceLocation,
    current_lines: Vec<(u32, u32)>,
    line_maps: std::collections::HashMap<u32, LineMap>,
}

impl NyarBackend {
    pub fn new() -> Self {
        Self {
            module: NyarcModule::default(),
            locals: Vec::new(),
            classes: std::collections::HashMap::new(),
            current_class: None,
            current_location: SourceLocation::default(),
            current_lines: Vec::new(),
            line_maps: std::collections::HashMap::new(),
        }
    }

    pub fn add_line_map(&mut self, source_id: u32, content: &str) {
        self.line_maps.insert(source_id, LineMap::new(content));
    }

    fn add_local(&mut self, name: String) -> u8 {
        if let Some(idx) = self.locals.iter().position(|l| l == &name) {
            println!("DEBUG: Local {} already exists at index {}", name, idx);
            return idx as u8;
        }
        let idx = self.locals.len() as u8;
        println!("DEBUG: Adding local {} at index {}", name, idx);
        self.locals.push(name);
        idx
    }

    fn find_local(&self, name: &str) -> Option<u8> {
        let res = self.locals.iter().position(|l| l == name).map(|i| i as u8);
        println!("DEBUG: Finding local {}: {:?}", name, res);
        res
    }

    fn add_constant(&mut self, constant: Constant) -> u16 {
        if let Some(idx) = self.module.constants.iter().position(|c| c == &constant) {
            return idx as u16;
        }
        let idx = self.module.constants.len() as u16;
        self.module.constants.push(constant);
        idx
    }

    pub fn emit(&mut self, instr: Instruction, code: &mut Vec<u8>) {
        let offset = code.len() as u32;
        let line = self.current_location.offset;
        if self.current_lines.is_empty() || self.current_lines.last().unwrap().1 != line {
            self.current_lines.push((offset, line));
        }
        code.extend_from_slice(&instr.encode());
    }

    fn add_class(&mut self, name: String, fields: Vec<String>) {
        self.classes.insert(name.clone(), fields.clone());
        let qn = QualifiedName::from(name.as_str());
        if !self.module.classes.iter().any(|c| c.name == qn) {
            self.module.classes.push(ClassInfo {
                name: qn,
                fields,
                location: SourceLocation::default(),
            });
        }
    }

    fn find_class_index(&self, name: &str) -> Option<u16> {
        let qn = QualifiedName::from(name);
        self.module.classes.iter().position(|c| c.name == qn).map(|i| i as u16)
    }

    fn try_fold_const(&self, tree: &IKunTree) -> Option<Constant> {
        match tree {
            IKunTree::Constant(i) => Some(Constant::Int(*i)),
            IKunTree::FloatConstant(f) => Some(Constant::Float(f64::from_bits(*f))),
            IKunTree::StringConstant(s) => Some(Constant::String(s.clone())),
            _ => None,
        }
    }

    fn scan_fields(&mut self, tree: &IKunTree, fields: &mut Vec<String>) {
        match tree {
            IKunTree::Source(_, body) => self.scan_fields(body, fields),
            IKunTree::Seq(items) | IKunTree::Module(_, items) => {
                for item in items {
                    self.scan_fields(item, fields);
                }
            }
            IKunTree::Lambda(_, body)
            | IKunTree::Export(_, body)
            | IKunTree::Return(body)
            | IKunTree::Meta(body)
            | IKunTree::Trap(body)
            | IKunTree::SoALayout(body)
            | IKunTree::AoSLayout(body)
            | IKunTree::Tiled(_, body)
            | IKunTree::Unrolled(_, body)
            | IKunTree::Vectorized(_, body)
            | IKunTree::Reg(body)
            | IKunTree::ResourceClone(body)
            | IKunTree::ResourceDrop(body) => {
                self.scan_fields(body, fields);
            }
            IKunTree::StateUpdate(_, value)
            | IKunTree::Map(_, value)
            | IKunTree::Filter(_, value)
            | IKunTree::Repeat(_, value)
            | IKunTree::Compose(_, value)
            | IKunTree::WithContext(_, value)
            | IKunTree::WithConstraint(_, value)
            | IKunTree::Pipe(_, value)
            | IKunTree::GpuMap(_, value)
            | IKunTree::CpuMap(_, value)
            | IKunTree::SoAMap(_, value) => {
                self.scan_fields(value, fields);
            }
            IKunTree::Choice(cond, t, f) | IKunTree::Reduce(cond, t, f) => {
                self.scan_fields(cond, fields);
                self.scan_fields(t, fields);
                self.scan_fields(f, fields);
            }
            IKunTree::LifeCycle(setup, cleanup) => {
                self.scan_fields(setup, fields);
                self.scan_fields(cleanup, fields);
            }
            IKunTree::TiledMap(_, f, x)
            | IKunTree::VectorizedMap(_, f, x)
            | IKunTree::UnrolledMap(_, f, x) => {
                self.scan_fields(f, fields);
                self.scan_fields(x, fields);
            }
            IKunTree::Apply(callee, args) => {
                self.scan_fields(callee, fields);
                for arg in args {
                    self.scan_fields(arg, fields);
                }
            }
            IKunTree::Closure(body, captured) => {
                self.scan_fields(body, fields);
                for item in captured {
                    self.scan_fields(item, fields);
                }
            }
            IKunTree::Extension(name, args) => {
                if name == "set_field" {
                    // args: [obj, name, value]
                    if let IKunTree::Symbol(field_name) = &args[1] {
                        if !fields.contains(field_name) {
                            fields.push(field_name.clone());
                        }
                    }
                    if args.len() > 2 {
                        self.scan_fields(&args[2], fields);
                    }
                } else if name == "assign" {
                    // args: [target, value]
                    if args.len() > 1 {
                        self.scan_fields(&args[1], fields);
                    }
                } else {
                    for arg in args {
                        self.scan_fields(arg, fields);
                    }
                }
            }
            IKunTree::CrossLangCall { arguments, .. } => {
                for arg in arguments {
                    self.scan_fields(arg, fields);
                }
            }
            _ => {}
        }
    }

    pub fn lower_tree(&mut self, tree: &IKunTree) -> Result<Vec<u8>, NyarError> {
        println!("DEBUG: lower_tree: {:?}", tree);
        let mut code = Vec::new();
        match tree {
            IKunTree::Source(loc, body) => {
                let prev_loc = self.current_location;
                let line = if let Some(map) = self.line_maps.get(&loc.source_id) {
                    map.lookup(loc.span.start).0
                } else {
                    loc.span.start
                };
                self.current_location = SourceLocation {
                    source_id: loc.source_id as u32,
                    offset: line as u32,
                };
                let res = self.lower_tree(body)?;
                self.current_location = prev_loc;
                return Ok(res);
            }
            IKunTree::Symbol(name) => {
                if let Some(idx) = self.find_local(name) {
                    println!("DEBUG: LoadLocal {} at index {}", name, idx);
                    self.emit(Instruction::LoadLocal(idx), &mut code);
                } else {
                    let final_name = if let Some(class_name) = &self.current_class {
                        format!("{}::{}", class_name, name)
                    } else {
                        name.clone()
                    };
                    let idx = self.add_constant(Constant::String(final_name.clone()));
                    println!("DEBUG: LoadGlobal {} constant index {}", final_name, idx);
                    self.emit(Instruction::LoadGlobal(idx), &mut code);
                }
            }
            IKunTree::Apply(callee, args) => {
                let mut current_callee = callee.as_ref();
                while let IKunTree::Source(_, inner) = current_callee {
                    current_callee = inner.as_ref();
                }

                if let IKunTree::Symbol(name) = current_callee {
                    if let Some(class_idx) = self.find_class_index(name) {
                        self.emit(Instruction::NewObject(class_idx), &mut code);
                        for arg in args {
                            code.extend(self.lower_tree(arg)?);
                        }
                        self.emit(Instruction::Initiate(args.len() as u8), &mut code);
                        return Ok(code);
                    }
                }
                for arg in args {
                    code.extend(self.lower_tree(arg)?);
                }
                if let IKunTree::Symbol(name) = current_callee {
                    let final_name = if let Some(class_name) = &self.current_class {
                        format!("{}::{}", class_name, name)
                    } else {
                        name.clone()
                    };
                    let idx = self.add_constant(Constant::String(final_name));
                    self.emit(Instruction::CallSymbol(idx, args.len() as u8), &mut code);
                } else {
                    code.extend(self.lower_tree(callee)?);
                    self.emit(Instruction::CallClosure(args.len() as u8), &mut code);
                }
            }
            IKunTree::Lambda(params, body) => {
                let prev_locals = self.locals.clone();
                let prev_lines = std::mem::take(&mut self.current_lines);
                self.locals.clear();
                for param in params {
                    self.add_local(param.clone());
                }
                let mut body_code = self.lower_tree(body)?;
                if body_code.last() != Some(&(Opcode::Return as u8)) {
                    self.emit(Instruction::Return, &mut body_code);
                }
                let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);

                let chunk_idx = self.module.chunks.len();
                if chunk_idx >= u16::MAX as usize {
                    return Err(NyarError::new(
                        0x1007,
                        nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                        nyar_types::SourceLocation::default(),
                    ));
                }
                let chunk_idx = chunk_idx as u16;
                let num_locals = self.locals.len() as u16;
                self.module.chunks.push(Chunk {
                    locals: num_locals.max(32),
                    upvalues: 0,
                    max_stack: 64,
                    code: body_code,
                    handlers: vec![],
                    lines: final_lines,
                    decoded: std::sync::OnceLock::new(),
                    hotness: std::sync::atomic::AtomicU32::new(0),
                });
                self.locals = prev_locals;
                self.emit(Instruction::MakeClosure(chunk_idx, vec![]), &mut code);
            }
            IKunTree::StateUpdate(target, value) => {
                if let IKunTree::Symbol(name) = &**target {
                    println!("DEBUG: Compiling StateUpdate for {}, current_class: {:?}", name, self.current_class);
                    code.extend(self.lower_tree(value)?);
                    // StateUpdate 应该返回被赋的值
                    self.emit(Instruction::Dup(0), &mut code);

                    let final_name = if let Some(class_name) = &self.current_class {
                        let res = format!("{}::{}", class_name, name);
                        println!("DEBUG: Prefixing name {} -> {}", name, res);
                        res
                    } else {
                        name.clone()
                    };

                    if let Some(idx) = self.find_local(name) {
                        self.emit(Instruction::StoreLocal(idx), &mut code);
                    } else {
                        let idx = self.add_constant(Constant::String(final_name));
                        self.emit(Instruction::StoreGlobal(idx), &mut code);
                    }
                }
            }
            IKunTree::Seq(items) => {
                if items.is_empty() {
                    self.emit(Instruction::I64Const(0), &mut code);
                } else {
                    for (i, item) in items.iter().enumerate() {
                        code.extend(self.lower_tree(item)?);
                        // 如果不是最后一个元素，且该元素产生了值（在栈上留下了东西），我们需要将其弹出
                        // 但在目前的极简实现中，我们假设每个表达式都留下一个值
                        if i < items.len() - 1 {
                            self.emit(Instruction::Pop, &mut code);
                        }
                    }
                }
            }
            IKunTree::Constant(v) => {
                self.emit(Instruction::I64Const(*v), &mut code);
            }
            IKunTree::FloatConstant(v) => {
                self.emit(Instruction::F64Const(f64::from_bits(*v)), &mut code);
            }
            IKunTree::BooleanConstant(v) => {
                let idx = self.add_constant(Constant::Int(if *v { 1 } else { 0 }));
                self.emit(Instruction::Push(idx), &mut code);
            }
            IKunTree::StringConstant(v) => {
                self.emit(Instruction::StringConst(v.clone()), &mut code);
            }
            IKunTree::Module(name, items) => {
                println!("DEBUG: Lowering Module {}: {:#?}", name, items);
                let prev_locals = self.locals.clone();
                let prev_lines = std::mem::take(&mut self.current_lines);
                self.locals.clear();
                let mut module_code = Vec::new();
                for item in items {
                    module_code.extend(self.lower_tree(item)?);
                }
                println!("DEBUG: Module code generated: {:02X?}", module_code);
                if module_code.last() != Some(&(Opcode::Return as u8)) {
                    self.emit(Instruction::Return, &mut module_code);
                }
                let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
                let num_locals = self.locals.len() as u16;
                self.module.chunks.push(Chunk {
                    locals: num_locals.max(32),
                    upvalues: 0,
                    max_stack: 64,
                    code: module_code,
                    handlers: vec![],
                    lines: final_lines,
                    decoded: std::sync::OnceLock::new(),
                    hotness: std::sync::atomic::AtomicU32::new(0),
                });
                self.locals = prev_locals;
            }
            IKunTree::Export(name, body) => {
                let final_name = if let Some(class_name) = &self.current_class {
                    format!("{}::{}", class_name, name)
                } else {
                    name.clone()
                };

                let mut current = body.as_ref();
                while let IKunTree::Source(_, inner) = current {
                    current = inner.as_ref();
                }

                if let IKunTree::Lambda(params, body) = current {
                    let prev_locals = self.locals.clone();
                    let prev_lines = std::mem::take(&mut self.current_lines);
                    self.locals.clear();
                    for param in params {
                        self.add_local(param.clone());
                    }
                    let mut body_code = self.lower_tree(body)?;
                    // Ensure Return at the end
                    if body_code.last() != Some(&(Opcode::Return as u8)) {
                        self.emit(Instruction::Return, &mut body_code);
                    }
                    let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);

                    let chunk_idx = self.module.chunks.len();
                    if chunk_idx >= u16::MAX as usize {
                        return Err(NyarError::new(
                            0x1007,
                            nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                            nyar_types::SourceLocation::default(),
                        ));
                    }
                    let chunk_idx = chunk_idx as u16;
                    let num_locals = self.locals.len() as u16;
                    self.module.chunks.push(Chunk {
                        locals: num_locals.max(32),
                        upvalues: 0,
                        max_stack: 64,
                        code: body_code,
                        handlers: vec![],
                        lines: final_lines,
                        decoded: std::sync::OnceLock::new(),
                        hotness: std::sync::atomic::AtomicU32::new(0),
                    });
                    self.module.exports.push(ExportInfo {
                        symbol: QualifiedName::from(final_name.as_str()),
                        chunk_idx,
                    });
                    self.locals = prev_locals;
                } else {
                    // If it's not a lambda, it might be a module-level constant or expression
                    let code_piece = self.lower_tree(body)?;
                    // We don't have a good way to "export" an expression result yet,
                    // but we can at least emit the code.
                    code.extend(code_piece);
                }
            }
            IKunTree::Return(val) => {
                if let Some(code_tail) = self.lower_tail_call(val)? {
                    code.extend(code_tail);
                } else {
                    code.extend(self.lower_tree(val)?);
                    self.emit(Instruction::Return, &mut code);
                }
            }
            IKunTree::Choice(cond, then_branch, else_branch) => {
                // 1. Evaluate condition
                code.extend(self.lower_tree(cond)?);
                
                // 2. Placeholder for JumpIfFalse to else_branch
                let jump_false_placeholder = code.len();
                self.emit(Instruction::JumpIfFalse(0), &mut code);
                
                // 3. Evaluate then_branch
                let then_code = self.lower_tree(then_branch)?;
                code.extend(then_code);
                
                // 4. Placeholder for Jump to end
                let jump_end_placeholder = code.len();
                self.emit(Instruction::Jump(0), &mut code);
                
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
                let mut arg_code = Vec::new();
                for arg in args {
                    arg_code.extend(self.lower_tree(arg)?);
                }
                code.extend(arg_code);
                println!("DEBUG: Current code before FFICall: {:02X?}", code);
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
                self.emit(Instruction::FFICall(name_idx, args.len() as u8), &mut code);
            }
            IKunTree::Extension(name, args) => {
                println!("Backend: Extension {}, args len {}", name, args.len());
                match name.as_str() {
                    "local_variable" => {
                        // [name, type, init]
                        println!("DEBUG: local_variable args: {:?}", args);
                        if let IKunTree::StringConstant(name) = &args[0] {
                            if let Some(init) = args.get(2) {
                                code.extend(self.lower_tree(init)?);
                            } else {
                                self.emit(Instruction::I64Const(0), &mut code);
                            }
                            // local_variable 应该返回被赋的值
                            self.emit(Instruction::Dup(0), &mut code);
                            let local_idx = self.add_local(name.clone());
                            self.emit(Instruction::StoreLocal(local_idx), &mut code);
                        }
                    }
                    "class_def" => {
                        // args: [name (Symbol), bases (Extension "bases"), body (Seq)]
                        if let IKunTree::Symbol(class_name) = &args[0] {
                            let mut fields = Vec::new();
                            // 预扫描 body 寻找字段定义
                if let IKunTree::Seq(body) = &args[2] {
                    for item in body {
                        match item {
                            IKunTree::StateUpdate(target, _) => {
                                if let IKunTree::Symbol(name) = &**target {
                                    if !fields.contains(name) {
                                        fields.push(name.clone());
                                    }
                                }
                            }
                            IKunTree::Extension(ext_name, ext_args) if ext_name == "assign" => {
                                if let IKunTree::Symbol(name) = &ext_args[0] {
                                    if !fields.contains(name) {
                                        fields.push(name.clone());
                                    }
                                }
                            }
                            _ => {
                                // 递归扫描 lambda 体内的 set_field
                                self.scan_fields(item, &mut fields);
                            }
                        }
                    }
                }
                            // 同时将这些字段名加入常量池
                            for field in &fields {
                                self.add_constant(Constant::String(field.clone()));
                            }

                            self.add_class(class_name.clone(), fields);

                            // 设置当前类上下文
                            let prev_class = self.current_class.take();
                            self.current_class = Some(class_name.clone());

                            // 执行 body 来定义方法
                            if let IKunTree::Seq(body) = &args[2] {
                                for item in body {
                                    match item {
                                        IKunTree::Export(name, lambda) => {
                                            let final_name = if let Some(class_name) = &self.current_class {
                                                format!("{}::{}", class_name, name)
                                            } else {
                                                name.clone()
                                            };
                                            println!("DEBUG: Exporting class method: {}", final_name);
                                            if let IKunTree::Lambda(params, body) = &**lambda {
                                                let prev_locals = self.locals.clone();
                                                let prev_lines = std::mem::take(&mut self.current_lines);
                                                self.locals.clear();
                                                for param in params {
                                                    self.add_local(param.clone());
                                                }
                                                let mut body_code = self.lower_tree(body)?;
                                                if body_code.last() != Some(&(Opcode::Return as u8)) {
                                                    self.emit(Instruction::Return, &mut body_code);
                                                }
                                                let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
                                                let chunk_idx = self.module.chunks.len() as u16;
                                                self.module.chunks.push(Chunk {
                                                    locals: (self.locals.len() as u16).max(32),
                                                    upvalues: 0,
                                                    max_stack: 64,
                                                    code: body_code,
                                                    handlers: vec![],
                                                    lines: final_lines,
                                                    decoded: std::sync::OnceLock::new(),
                                                    hotness: std::sync::atomic::AtomicU32::new(0),
                                                });
                                                self.module.exports.push(ExportInfo {
                                                    symbol: QualifiedName::from(final_name.as_str()),
                                                    chunk_idx,
                                                });
                                                // 同时也将方法名作为一个全局变量，其值为一个闭包
                                                let qn = QualifiedName::from(final_name.as_str());
                                                let qn_idx = self.add_constant(Constant::QualifiedName(qn));
                                                self.emit(Instruction::MakeClosure(chunk_idx, vec![]), &mut code);
                                                self.emit(Instruction::StoreGlobal(qn_idx), &mut code);
                                                self.emit(Instruction::Pop, &mut code);

                                                self.locals = prev_locals;
                                            }
                                        }
                                        IKunTree::StateUpdate(target, value) => {
                                            if let IKunTree::Symbol(name) = &**target {
                                                let mut method_lambda = None;
                                                if let IKunTree::Lambda(_, _) = &**value {
                                                    method_lambda = Some(&**value);
                                                } else if let IKunTree::Extension(ext_name, ext_args) = &**value {
                                                    // Remove python_function support
                                                    if ext_name == "function_decl" {
                                                        method_lambda = Some(&ext_args[1]);
                                                    }
                                                }

                                                if let Some(IKunTree::Lambda(params, body)) = method_lambda {
                                                    let final_name = if let Some(class_name) = &self.current_class {
                                                        format!("{}::{}", class_name, name)
                                                    } else {
                                                        name.clone()
                                                    };
                                                    println!("DEBUG: Exporting class method via StateUpdate: {}", final_name);
                                                    let prev_locals = self.locals.clone();
                                                    let prev_lines = std::mem::take(&mut self.current_lines);
                                                    self.locals.clear();
                                                    for param in params {
                                                        self.add_local(param.clone());
                                                    }
                                                    let mut body_code = self.lower_tree(body)?;
                                                    if body_code.last() != Some(&(Opcode::Return as u8)) {
                                                        self.emit(Instruction::Return, &mut body_code);
                                                    }
                                                    let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
                                                    let chunk_idx = self.module.chunks.len() as u16;
                                                    self.module.chunks.push(Chunk {
                                                        locals: (self.locals.len() as u16).max(32),
                                                        upvalues: 0,
                                                        max_stack: 64,
                                                        code: body_code,
                                                        handlers: vec![],
                                                        lines: final_lines,
                                                        decoded: std::sync::OnceLock::new(),
                                                        hotness: std::sync::atomic::AtomicU32::new(0),
                                                    });
                                                    self.module.exports.push(ExportInfo {
                                                        symbol: QualifiedName::from(final_name.as_str()),
                                                        chunk_idx,
                                                    });
                                                    // 同时也将方法名作为一个全局变量，其值为一个闭包
                                                    let qn = QualifiedName::from(final_name.as_str());
                                                    let qn_idx = self.add_constant(Constant::QualifiedName(qn));
                                                    self.emit(Instruction::MakeClosure(chunk_idx, vec![]), &mut code);
                                                    self.emit(Instruction::Dup(0), &mut code);
                                                    self.emit(Instruction::StoreGlobal(qn_idx), &mut code);

                                                    self.locals = prev_locals;
                                                } else {
                                                    code.extend(self.lower_tree(item)?);
                                                }
                                            }
                                        }
                                        IKunTree::Extension(ext_name, ext_args) if ext_name == "assign" => {
                                            if let IKunTree::Symbol(name) = &ext_args[0] {
                                                let value = &ext_args[1];
                                                let mut method_lambda = None;
                                                if let IKunTree::Lambda(_, _) = value {
                                                    method_lambda = Some(value);
                                                } else if let IKunTree::Extension(inner_ext_name, inner_ext_args) = value {
                                                    if inner_ext_name == "function_decl" {
                                                        method_lambda = Some(&inner_ext_args[1]);
                                                    }
                                                }

                                                if let Some(IKunTree::Lambda(params, body)) = method_lambda {
                                                    let final_name = if let Some(class_name) = &self.current_class {
                                                        format!("{}::{}", class_name, name)
                                                    } else {
                                                        name.clone()
                                                    };
                                                    println!("DEBUG: Exporting class method via assign extension: {}", final_name);
                                                    let prev_locals = self.locals.clone();
                                                    let prev_lines = std::mem::take(&mut self.current_lines);
                                                    self.locals.clear();
                                                    for param in params {
                                                        self.add_local(param.clone());
                                                    }
                                                    let mut body_code = self.lower_tree(body)?;
                                                    if body_code.last() != Some(&(Opcode::Return as u8)) {
                                                        self.emit(Instruction::Return, &mut body_code);
                                                    }
                                                    let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
                                                    let chunk_idx = self.module.chunks.len() as u16;
                                                    self.module.chunks.push(Chunk {
                                                        locals: (self.locals.len() as u16).max(32),
                                                        upvalues: 0,
                                                        max_stack: 64,
                                                        code: body_code,
                                                        handlers: vec![],
                                                        lines: final_lines,
                                                        decoded: std::sync::OnceLock::new(),
                                                        hotness: std::sync::atomic::AtomicU32::new(0),
                                                    });
                                                    self.module.exports.push(ExportInfo {
                                                        symbol: QualifiedName::from(final_name.as_str()),
                                                        chunk_idx,
                                                    });
                                                    // 同时也将方法名作为一个全局变量，其值为一个闭包
                                                    let qn = QualifiedName::from(final_name.as_str());
                                                    let qn_idx = self.add_constant(Constant::QualifiedName(qn));
                                                    self.emit(Instruction::MakeClosure(chunk_idx, vec![]), &mut code);
                                                    self.emit(Instruction::Dup(0), &mut code);
                                                    self.emit(Instruction::StoreGlobal(qn_idx), &mut code);

                                                    self.locals = prev_locals;
                                                } else {
                                                    code.extend(self.lower_tree(item)?);
                                                }
                                            }
                                        }
                                        _ => {
                                            code.extend(self.lower_tree(item)?);
                                        }
                                    }
                                }
                            }

                            // 恢复类上下文
                            self.current_class = prev_class;

                            // 弹出 body 的返回值（Seq 的最后一个值）
                            self.emit(Instruction::Pop, &mut code);

                            // 创建类对象
                            self.emit(Instruction::StringConst(class_name.clone()), &mut code);
                        }
                    }
                    "function_decl" => {
                        // args: [name, lambda, ...]
                        code.extend(self.lower_tree(&args[1])?);
                    }
                    "invoke_method" => {
                        // args: [obj, method_name (Symbol), args (Extension "args")]
                        if let (IKunTree::Symbol(method_name), IKunTree::Extension(_, method_args)) = (&args[1], &args[2]) {
                            // 1. Push arguments
                            for arg in method_args {
                                code.extend(self.lower_tree(arg)?);
                            }
                            // 2. Push object (receiver)
                            code.extend(self.lower_tree(&args[0])?);
                            // 3. Invoke method
                            let idx = self.add_constant(Constant::String(method_name.clone()));
                            self.emit(Instruction::InvokeMethod(idx, method_args.len() as u8), &mut code);
                        }
                    }
                    "get_field" => {
                        // args: [obj, field_name (Symbol)]
                        code.extend(self.lower_tree(&args[0])?);
                        if let IKunTree::Symbol(field_name) = &args[1] {
                            let idx = self.add_constant(Constant::String(field_name.clone()));
                            println!("DEBUG: GetField {} constant index {}", field_name, idx);
                            self.emit(Instruction::GetField(idx), &mut code);
                        }
                    }
                    "set_field" => {
                        // args: [obj, field_name (Symbol), value]
                        code.extend(self.lower_tree(&args[0])?); // obj
                        code.extend(self.lower_tree(&args[2])?); // value
                        if let IKunTree::Symbol(field_name) = &args[1] {
                            let idx = self.add_constant(Constant::String(field_name.clone()));
                            println!("DEBUG: SetField {} constant index {}", field_name, idx);
                            self.emit(Instruction::SetField(idx), &mut code);
                        }
                    }
                    "list" => {
                        // 处理空列表或其他
                        self.emit(Instruction::I64Const(0), &mut code);
                    }
                    "none" => {
                        self.emit(Instruction::I64Const(0), &mut code);
                    }
                    "assign" => {
                        if args.len() == 2 {
                            // [name, value]
                            code.extend(self.lower_tree(&args[1])?);
                            // assign 应该返回被赋的值
                            self.emit(Instruction::Dup(0), &mut code);
                            if let IKunTree::Symbol(name) = &args[0] {
                                if let Some(idx) = self.find_local(name) {
                                    self.emit(Instruction::StoreLocal(idx), &mut code);
                                } else {
                                    let final_name = if let Some(class_name) = &self.current_class {
                                        format!("{}::{}", class_name, name)
                                    } else {
                                        name.clone()
                                    };
                                    let idx = self.add_constant(Constant::String(final_name));
                                    self.emit(Instruction::StoreGlobal(idx), &mut code);
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
                                self.emit(Instruction::Return, &mut code);
                            }
                        } else {
                            self.emit(Instruction::Return, &mut code);
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
                                }
                                
                                self.add_class(class_name.clone(), fields);
                                
                                // 设置当前类上下文
                                let prev_class = self.current_class.take();
                                self.current_class = Some(class_name.clone());

                                for member in members {
                                    code.extend(self.lower_tree(member)?);
                                    // 弹出成员定义的返回值（如果有的话）
                                    self.emit(Instruction::Pop, &mut code);
                                }
                                
                                // 恢复类上下文
                                self.current_class = prev_class;
                            }
                            
                            // 创建类对象（或者是返回类名）
                            self.emit(Instruction::StringConst(class_name.clone()), &mut code);
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

                            let mut arg_code = Vec::new();
                            let mut argc = 0;
                            if let IKunTree::Seq(params) = &args[1] {
                                argc = params.len() as u8;
                                for param in params {
                                    arg_code.extend(self.lower_tree(param)?);
                                }
                            }
                            self.emit(Instruction::NewObject(idx), &mut code);
                            code.extend(arg_code);
                            self.emit(Instruction::Initiate(argc), &mut code);
                            return Ok(code);
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
                                let prev_lines = std::mem::take(&mut self.current_lines);
                                self.locals.clear();
                                // Add parameters to locals if present
                                if args.len() >= 4 {
                                    if let IKunTree::Seq(params) = &args[2] {
                                        for param in params {
                                            if let IKunTree::Extension(ext_name, ext_args) = param {
                                                if ext_name == "parameter" {
                                                    println!("DEBUG: parameter args: {:?}", ext_args);
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

                                let mut body_code = self.lower_tree(body)?;
                                if body_code.last() != Some(&(Opcode::Return as u8)) {
                                    self.emit(Instruction::Return, &mut body_code);
                                }
                                let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
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
                                    code: body_code,
                                    handlers: vec![],
                                    lines: final_lines,
                                    decoded: std::sync::OnceLock::new(),
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
                            let prev_lines = std::mem::take(&mut self.current_lines);
                            self.locals.clear();
                            if let IKunTree::Seq(params) = &args[0] {
                                for param in params {
                                    if let IKunTree::Symbol(pname) = param {
                                        self.add_local(pname.clone());
                                    }
                                }
                            }
                            let mut body_code = self.lower_tree(body)?;
                            if body_code.last() != Some(&(Opcode::Return as u8)) {
                                self.emit(Instruction::Return, &mut body_code);
                            }
                            let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
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
                                code: body_code,
                                handlers: vec![],
                                lines: final_lines,
                                decoded: std::sync::OnceLock::new(),
                                hotness: std::sync::atomic::AtomicU32::new(0),
                            });
                            self.emit(Instruction::MakeClosure(chunk_idx, vec![]), &mut code);
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
                                    self.emit(Instruction::InvokeMethod(name_idx, call_args.len() as u8), &mut code);
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
                                    self.emit(Instruction::Call(name_idx, call_args.len() as u8), &mut code);
                                }
                            }
                        }
                    }
                    "import" => {
                        if let IKunTree::StringConstant(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            self.emit(Instruction::Call(idx, 0), &mut code);
                        }
                    }
                    "import_from" => {
                        if args.len() == 2 {
                            if let (IKunTree::StringConstant(module), IKunTree::StringConstant(member)) = (&args[0], &args[1]) {
                                let mod_idx = self.add_constant(Constant::String(module.clone()));
                                self.emit(Instruction::Call(mod_idx, 0), &mut code);
                                let mem_idx = self.add_constant(Constant::String(member.clone()));
                                self.emit(Instruction::GetField(mem_idx), &mut code);
                            }
                        }
                    }
                    "add" | "sub" | "mul" | "div" | "rem" => {
                        if let (Some(l), Some(r)) =
                            (self.try_fold_const(&args[0]), self.try_fold_const(&args[1]))
                        {
                            match (l, r) {
                                (Constant::Int(l), Constant::Int(r)) => {
                                    let result = match name.as_str() {
                                        "add" => l.checked_add(r),
                                        "sub" => l.checked_sub(r),
                                        "mul" => l.checked_mul(r),
                                        "div" => if r != 0 { l.checked_div(r) } else { None },
                                        "rem" => if r != 0 { l.checked_rem(r) } else { None },
                                        _ => unreachable!(),
                                    };
                                    if let Some(res) = result {
                                        self.emit(Instruction::I64Const(res), &mut code);
                                    } else {
                                        code.extend(self.lower_tree(&args[0])?);
                                        code.extend(self.lower_tree(&args[1])?);
                                        let name_idx = self.add_constant(Constant::String(name.clone()));
                                        self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                                    }
                                }
                                (Constant::Float(l), Constant::Float(r)) => {
                                    let result = match name.as_str() {
                                        "add" => l + r,
                                        "sub" => l - r,
                                        "mul" => l * r,
                                        "div" => l / r,
                                        "rem" => l % r,
                                        _ => unreachable!(),
                                    };
                                    self.emit(Instruction::F64Const(result), &mut code);
                                }
                                _ => {
                                    code.extend(self.lower_tree(&args[0])?);
                                    code.extend(self.lower_tree(&args[1])?);
                                    let name_idx = self.add_constant(Constant::String(name.clone()));
                                    self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                                }
                            }
                        } else {
                            code.extend(self.lower_tree(&args[0])?);
                            code.extend(self.lower_tree(&args[1])?);
                            let name_idx = self.add_constant(Constant::String(name.clone()));
                            self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                        }
                    }
                    "bit_and" | "bit_or" | "bit_xor" | "bit_shl" | "bit_shr" => {
                        if let (Some(l), Some(r)) =
                            (self.try_fold_const(&args[0]), self.try_fold_const(&args[1]))
                        {
                            if let (Constant::Int(l), Constant::Int(r)) = (l, r) {
                                let result = match name.as_str() {
                                    "bit_and" => Some(l & r),
                                    "bit_or" => Some(l | r),
                                    "bit_xor" => Some(l ^ r),
                                    "bit_shl" => l.checked_shl(r as u32),
                                    "bit_shr" => l.checked_shr(r as u32),
                                    _ => unreachable!(),
                                };
                                if let Some(res) = result {
                                    self.emit(Instruction::I64Const(res), &mut code);
                                } else {
                                    code.extend(self.lower_tree(&args[0])?);
                                    code.extend(self.lower_tree(&args[1])?);
                                    let name_idx = self.add_constant(Constant::String(name.clone()));
                                    self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                                }
                            } else {
                                code.extend(self.lower_tree(&args[0])?);
                                code.extend(self.lower_tree(&args[1])?);
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                            }
                        } else {
                            code.extend(self.lower_tree(&args[0])?);
                            code.extend(self.lower_tree(&args[1])?);
                            let name_idx = self.add_constant(Constant::String(name.clone()));
                            self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                        }
                    }
                    "bit_not" => {
                        if let Some(Constant::Int(l)) = self.try_fold_const(&args[0]) {
                            self.emit(Instruction::I64Const(!l), &mut code);
                        } else {
                            code.extend(self.lower_tree(&args[0])?);
                            let name_idx = self.add_constant(Constant::String("bit_not".to_string()));
                            self.emit(Instruction::InvokeMethod(name_idx, 0), &mut code);
                        }
                    }
                    "neg" => {
                        if let Some(c) = self.try_fold_const(&args[0]) {
                            match c {
                                Constant::Int(l) => self.emit(Instruction::I64Const(-l), &mut code),
                                Constant::Float(l) => self.emit(Instruction::F64Const(-l), &mut code),
                                _ => {
                                    code.extend(self.lower_tree(&args[0])?);
                                    let name_idx = self.add_constant(Constant::String("neg".to_string()));
                                    self.emit(Instruction::InvokeMethod(name_idx, 0), &mut code);
                                }
                            }
                        } else {
                            code.extend(self.lower_tree(&args[0])?);
                            let name_idx = self.add_constant(Constant::String("neg".to_string()));
                            self.emit(Instruction::InvokeMethod(name_idx, 0), &mut code);
                        }
                    }
                    "not" => {
                        if let Some(c) = self.try_fold_const(&args[0]) {
                            match c {
                                Constant::Int(l) => {
                                    self.emit(Instruction::I64Const(if l == 0 { 1 } else { 0 }), &mut code)
                                }
                                _ => {
                                    code.extend(self.lower_tree(&args[0])?);
                                    let name_idx = self.add_constant(Constant::String("not".to_string()));
                                    self.emit(Instruction::InvokeMethod(name_idx, 0), &mut code);
                                }
                            }
                        } else {
                            code.extend(self.lower_tree(&args[0])?);
                            let name_idx = self.add_constant(Constant::String("not".to_string()));
                            self.emit(Instruction::InvokeMethod(name_idx, 0), &mut code);
                        }
                    }
                    "and" => {
                        // a && b
                        // eval a
                        code.extend(self.lower_tree(&args[0])?);
                        // dup for jump
                        self.emit(Instruction::Dup(0), &mut code);
                        // if false, jump to end (keep false on stack)
                        let placeholder = code.len();
                        self.emit(Instruction::JumpIfFalse(0), &mut code);
                        // if true, pop a and eval b
                        self.emit(Instruction::Pop, &mut code);
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
                        self.emit(Instruction::Dup(0), &mut code);
                        // if true, jump to end (keep true on stack)
                        let placeholder = code.len();
                        self.emit(Instruction::JumpIfTrue(0), &mut code);
                        // if false, pop a and eval b
                        self.emit(Instruction::Pop, &mut code);
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
                            self.add_constant(Constant::String("vm:throw".to_string()));
                        self.emit(Instruction::Perform(name_idx, 2), &mut code);
                    }
                    "assert" => {
                        // [test, msg]
                        code.extend(self.lower_tree(&args[0])?);
                        let placeholder = code.len();
                        self.emit(Instruction::JumpIfTrue(0), &mut code);
                        code.extend(self.lower_tree(&args[1])?);
                        let name_idx = self
                            .add_constant(Constant::String("std:AssertionError".to_string()));
                        self.emit(Instruction::Perform(name_idx, 1), &mut code);
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
                            let prev_lines = std::mem::take(&mut self.current_lines);
                            // Finally handler catches everything, runs finalbody, then re-performs
                            // Handler receives: [effect_obj, args_list, continuation]
                            
                            // Load effect_obj and args_list to re-raise later
                            self.emit(Instruction::LoadLocal(0), &mut finally_handler_code);
                            self.emit(Instruction::LoadLocal(1), &mut finally_handler_code);
                            
                            // Run finalbody
                            finally_handler_code.extend(self.lower_tree(finalbody)?);
                            
                            // Re-raise
                            // We need an instruction that can raise with dynamic name and args list
                            // Nyar VM might need a dynamic raise. For now, let's assume it's vm:throw
                            let re_perform_idx = self.add_constant(Constant::String("vm:throw".to_string()));
                            self.emit(Instruction::Perform(re_perform_idx, 2), &mut finally_handler_code);
                            self.emit(Instruction::Return, &mut finally_handler_code);

                            let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
                            let finally_chunk_idx = self.module.chunks.len() as u16;
                            self.module.chunks.push(Chunk {
                                locals: 3,
                                upvalues: 0,
                                max_stack: 64,
                                code: finally_handler_code,
                                handlers: vec![],
                                lines: final_lines,
                                decoded: std::sync::OnceLock::new(),
                                hotness: std::sync::atomic::AtomicU32::new(0),
                            });
                            self.emit(Instruction::WithHandler(finally_chunk_idx), &mut code);
                        }

                        // 2. Wrap body in except handler
                        let mut except_handler_code = Vec::new();
                        let prev_lines = std::mem::take(&mut self.current_lines);
                        let raise_name_idx = self.add_constant(Constant::String("vm:throw".to_string()));
                        self.emit(Instruction::MatchEffect(raise_name_idx), &mut except_handler_code);
                        
                        let next_handler_placeholder = except_handler_code.len();
                        self.emit(Instruction::JumpIfFalse(0), &mut except_handler_code);

                        // Match! [exc, cause] are on stack
                        except_handler_code.extend(self.lower_tree(handlers)?);
                        
                        // If no except matched, re-raise
                        let re_perform_idx = self.add_constant(Constant::String("vm:throw".to_string()));
                        self.emit(Instruction::Perform(re_perform_idx, 2), &mut except_handler_code);

                        let handler_end = except_handler_code.len();
                        let off = (handler_end as isize - next_handler_placeholder as isize) as i16;
                        let instr = Instruction::JumpIfFalse(off).encode();
                        except_handler_code[next_handler_placeholder..next_handler_placeholder + instr.len()].copy_from_slice(&instr);
                        self.emit(Instruction::Return, &mut except_handler_code);

                        let final_lines = std::mem::replace(&mut self.current_lines, prev_lines);
                        let except_chunk_idx = self.module.chunks.len() as u16;
                        self.module.chunks.push(Chunk {
                            locals: 3,
                            upvalues: 0,
                            max_stack: 64,
                            code: except_handler_code,
                            handlers: vec![],
                            lines: final_lines,
                            decoded: std::sync::OnceLock::new(),
                            hotness: std::sync::atomic::AtomicU32::new(0),
                        });

                        self.emit(Instruction::WithHandler(except_chunk_idx), &mut code);
                        code.extend(self.lower_tree(body)?);
                        code.extend(self.lower_tree(orelse)?);
                        
                        // End of WithHandler(except)
                        self.emit(Instruction::Pop, &mut code); // Pop the handler if finished normally? 
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
                            self.emit(Instruction::Dup(1), &mut code); // Dup exc
                            code.extend(self.lower_tree(&args[0])?); // type
                            self.emit(Instruction::InstanceOf(0), &mut code);
                            
                            let placeholder = code.len();
                            self.emit(Instruction::JumpIfFalse(0), &mut code);
                            jump_placeholder = Some(placeholder);
                        }
                        
                        // 2. Assign name if present
                        if let IKunTree::Symbol(name) = &args[1] {
                             let name_idx = self.add_constant(Constant::String(name.clone()));
                             self.emit(Instruction::Dup(1), &mut code); // Dup exc
                             self.emit(Instruction::StoreGlobal(name_idx), &mut code);
                        }
                        
                        // 3. Execute body
                        code.extend(self.lower_tree(&args[2])?);
                        
                        // 4. Return from handler (handled!)
                        self.emit(Instruction::Return, &mut code);

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
                                        self.emit(Instruction::Dup(0), &mut code);
                                        self.emit(Instruction::StoreGlobal(temp_idx), &mut code);
                                        
                                        // Call __enter__
                                        let enter_idx = self.add_constant(Constant::String("__enter__".to_string()));
                                        self.emit(Instruction::InvokeMethod(enter_idx, 0), &mut code);
                                        
                                        if let IKunTree::Symbol(var_name) = &item_args[1] {
                                            let var_idx = self.add_constant(Constant::String(var_name.clone()));
                                            self.emit(Instruction::StoreGlobal(var_idx), &mut code);
                                        } else {
                                            self.emit(Instruction::Pop, &mut code);
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
                                self.emit(Instruction::LoadGlobal(temp_idx), &mut exit_code);
                                let exit_idx = self.add_constant(Constant::String("__exit__".to_string()));
                                // Call __exit__(None, None, None)
                                let none_idx = self.add_constant(Constant::String("None".to_string()));
                                self.emit(Instruction::Push(none_idx), &mut exit_code);
                                self.emit(Instruction::Push(none_idx), &mut exit_code);
                                self.emit(Instruction::Push(none_idx), &mut exit_code);
                                self.emit(Instruction::InvokeMethod(exit_idx, 3), &mut exit_code);
                                self.emit(Instruction::Pop, &mut exit_code);
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
                        self.emit(Instruction::JumpIfFalse(0), &mut code);
                        
                        code.extend(self.lower_tree(&args[1])?);
                        let jump_end_placeholder = code.len();
                        self.emit(Instruction::Jump(0), &mut code);
                        
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
                        self.emit(Instruction::SizeOf, &mut code);
                    }
                    "cast" => {
                        // (type)expr
                        // args[0] is target type (symbol or string), args[1] is expr
                        code.extend(self.lower_tree(&args[1])?);
                        if let IKunTree::Symbol(type_name) = &args[0] {
                            let idx = self.add_constant(Constant::String(type_name.clone()));
                            self.emit(Instruction::Cast(idx), &mut code);
                        } else if let IKunTree::StringConstant(type_name) = &args[0] {
                            let idx = self.add_constant(Constant::String(type_name.clone()));
                            self.emit(Instruction::Cast(idx), &mut code);
                        }
                    }
                    "inc_pre" | "inc_post" | "dec_pre" | "dec_post" => {
                        if let IKunTree::Symbol(name) = &args[0] {
                            let idx = self.add_constant(Constant::String(name.clone()));
                            self.emit(Instruction::LoadGlobal(idx), &mut code);
                            
                            // Determine method and stack behavior
                            let method = match name.as_str() {
                                "inc_pre" | "inc_post" => "add",
                                "dec_pre" | "dec_post" => "sub",
                                _ => unreachable!(),
                            };
                            let is_pre = name.starts_with("inc_pre") || name.starts_with("dec_pre");
                            
                            if !is_pre {
                                self.emit(Instruction::Dup(0), &mut code);
                            }
                            
                            self.emit(Instruction::I64Const(1), &mut code);
                            let method_idx = self.add_constant(Constant::String(method.to_string()));
                            self.emit(Instruction::InvokeMethod(method_idx, 1), &mut code);
                            
                            if is_pre {
                                self.emit(Instruction::Dup(0), &mut code);
                            }
                            
                            self.emit(Instruction::StoreGlobal(idx), &mut code);
                            self.emit(Instruction::Pop, &mut code);
                        }
                    }
                    "eq" | "ne" | "lt" | "le" | "gt" | "ge" => {
                        if let (Some(l), Some(r)) =
                            (self.try_fold_const(&args[0]), self.try_fold_const(&args[1]))
                        {
                            let result = match (l, r) {
                                (Constant::Int(l), Constant::Int(r)) => match name.as_str() {
                                    "eq" => l == r,
                                    "ne" => l != r,
                                    "lt" => l < r,
                                    "le" => l <= r,
                                    "gt" => l > r,
                                    "ge" => l >= r,
                                    _ => unreachable!(),
                                },
                                (Constant::Float(l), Constant::Float(r)) => match name.as_str() {
                                    "eq" => l == r,
                                    "ne" => l != r,
                                    "lt" => l < r,
                                    "le" => l <= r,
                                    "gt" => l > r,
                                    "ge" => l >= r,
                                    _ => unreachable!(),
                                },
                                (Constant::String(l), Constant::String(r)) => match name.as_str() {
                                    "eq" => l == r,
                                    "ne" => l != r,
                                    "lt" => l < r,
                                    "le" => l <= r,
                                    "gt" => l > r,
                                    "ge" => l >= r,
                                    _ => unreachable!(),
                                },
                                _ => {
                                    code.extend(self.lower_tree(&args[0])?);
                                    code.extend(self.lower_tree(&args[1])?);
                                    let name_idx = self.add_constant(Constant::String(name.clone()));
                                    self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                                    return Ok(code);
                                }
                            };
                            self.emit(Instruction::I64Const(if result { 1 } else { 0 }), &mut code);
                        } else {
                            code.extend(self.lower_tree(&args[0])?);
                            code.extend(self.lower_tree(&args[1])?);
                            let name_idx = self.add_constant(Constant::String(name.clone()));
                            self.emit(Instruction::InvokeMethod(name_idx, 1), &mut code);
                        }
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
                    self.emit(Instruction::TailCall(idx, args.len() as u8), &mut code);
                } else {
                    code.extend(self.lower_tree(callee)?);
                    self.emit(Instruction::TailCallClosure(args.len() as u8), &mut code);
                }
                Ok(Some(code))
            }
            _ => Ok(None),
        }
    }

    pub fn compile(&mut self, tree: &IKunTree) -> Result<NyarcModule, NyarError> {
        let code = self.lower_tree(tree)?;
        let mut final_code = code;
        if final_code.last() != Some(&(Opcode::Return as u8)) {
            self.emit(Instruction::Return, &mut final_code);
        }
        let lines = std::mem::take(&mut self.current_lines);
        self.module.chunks.push(Chunk {
            locals: 32,
            upvalues: 0,
            max_stack: 64,
            code: final_code,
            handlers: vec![],
            lines,
            decoded: Default::default(),
            hotness: std::sync::atomic::AtomicU32::new(0),
        });
        Ok(self.module.clone())
    }

    pub fn finish(mut self) -> NyarcModule {
        if self.module.chunks.is_empty() {
            println!("DEBUG: No chunks in module, adding an empty return chunk");
            self.module.chunks.push(Chunk {
                locals: 32,
                upvalues: 0,
                max_stack: 64,
                code: vec![Opcode::Return as u8],
                handlers: vec![],
                lines: vec![],
                decoded: std::sync::OnceLock::new(),
                hotness: std::sync::atomic::AtomicU32::new(0),
            });
        }
        self.module
    }

    pub fn finish_with_code(mut self, code: Vec<u8>) -> NyarcModule {
        self.module.chunks.push(Chunk {
            locals: 32,
            upvalues: 0,
            max_stack: 64,
            code,
            handlers: vec![],
            lines: vec![],
            decoded: std::sync::OnceLock::new(),
            hotness: std::sync::atomic::AtomicU32::new(0),
        });
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
                this.emit(Instruction::Return, &mut final_code);
            }
            let lines = std::mem::take(&mut this.current_lines);
            this.module.chunks.push(Chunk {
                locals: 32,
                upvalues: 0,
                max_stack: 64,
                code: final_code,
                handlers: vec![],
                lines,
                decoded: Default::default(),
                hotness: std::sync::atomic::AtomicU32::new(0),
            });
        }
        Ok(BackendArtifact::Binary(this.module.encode()))
    }
}
