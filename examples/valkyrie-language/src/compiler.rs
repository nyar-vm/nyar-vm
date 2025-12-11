use crate::ast::{Expr, Stmt};
use crate::lexer::Error;
use nyar_vm::bytecode::format::{Chunk, ClassInfo, TraitInfo, ImplInfo, Constant, NyarcModule};
use nyar_vm::bytecode::opcode::Opcode;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

struct Compiler {
    constants: Vec<Constant>,
    chunks: Vec<Chunk>,
    // Global function registry: name -> chunk_index
    functions: HashMap<String, u16>,
    classes: Vec<ClassInfo>,
    class_map: HashMap<String, u16>,
    traits: Vec<TraitInfo>,
    trait_map: HashMap<String, u16>,
    impls: Vec<ImplInfo>,
}

struct FunctionContext {
    code: Vec<u8>,
    locals: Vec<String>,
    args: Vec<String>,
    upvalues: Vec<(bool, u8)>, // (is_local, index)
}

impl Compiler {
    fn new() -> Self {
        Self {
            constants: Vec::new(),
            chunks: Vec::new(),
            functions: HashMap::new(),
            classes: Vec::new(),
            class_map: HashMap::new(),
            traits: Vec::new(),
            trait_map: HashMap::new(),
            impls: Vec::new(),
        }
    }

    fn add_constant(&mut self, c: Constant) -> u16 {
        if let Some((i, _)) = self.constants.iter().enumerate().find(|(_, v)| *v == &c) {
            i as u16
        } else {
            let i = self.constants.len() as u16;
            self.constants.push(c);
            i
        }
    }

    fn add_string(&mut self, s: &str) -> u16 {
        self.add_constant(Constant::String(s.to_string()))
    }
}

impl FunctionContext {
    fn new(args: Vec<String>) -> Self {
        Self {
            code: Vec::new(),
            locals: args, // Arguments are the first locals
            args: Vec::new(), // Not used for tracking, just reference
            upvalues: Vec::new(),
        }
    }

    fn find_local(&self, name: &str) -> Option<u8> {
        self.locals.iter().position(|l| l == name).map(|i| i as u8)
    }

    fn add_local(&mut self, name: String) -> u8 {
        let i = self.locals.len();
        self.locals.push(name);
        i as u8
    }

    fn add_upvalue(&mut self, is_local: bool, index: u8) -> u8 {
        for (i, up) in self.upvalues.iter().enumerate() {
            if up.0 == is_local && up.1 == index {
                return i as u8;
            }
        }
        let i = self.upvalues.len();
        self.upvalues.push((is_local, index));
        i as u8
    }
}

fn resolve_upvalue(contexts: &mut [FunctionContext], name: &str) -> Option<u8> {
    let len = contexts.len();
    if len < 2 { return None; }
    
    // Check immediate parent
    let parent_idx = len - 2;
    if let Some(local_idx) = contexts[parent_idx].find_local(name) {
        return Some(contexts[len - 1].add_upvalue(true, local_idx));
    }
    
    // Check upvalues of parent (recursion)
    let (parent_slice, child_slice) = contexts.split_at_mut(len - 1);
    let child = &mut child_slice[0];
    
    if let Some(upvalue_idx) = resolve_upvalue(parent_slice, name) {
        return Some(child.add_upvalue(false, upvalue_idx));
    }
    
    None
}

fn compile_func_to_chunk(compiler: &mut Compiler, args: Vec<String>, body: &[Stmt]) -> Result<u16, Error> {
    let mut contexts = vec![FunctionContext::new(args)];
    for stmt in body {
        compile_stmt(compiler, &mut contexts, stmt)?;
    }
    let mut func_ctx = contexts.pop().unwrap();
    
    func_ctx.code.push(Opcode::Push as u8);
    let null_idx = compiler.add_constant(Constant::Int(0));
    func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
    func_ctx.code.push(Opcode::Return as u8);

    let chunk = Chunk {
        locals: func_ctx.locals.len() as u16,
        upvalues: func_ctx.upvalues.len() as u16,
        max_stack: 16,
        code: func_ctx.code,
        handlers: vec![],
    };
    
    // Chunk index in the final module will be (existing chunks) + 1 (for main) + 1 (this new one)?
    // No, compiler.chunks contains all chunks except main.
    // So index 0 in compiler.chunks is index 1 in module.chunks.
    // So if compiler.chunks has N items, the next item is at index N+1.
    let chunk_idx = (compiler.chunks.len() + 1) as u16;
    compiler.chunks.push(chunk);
    Ok(chunk_idx)
}

fn compile_expr(compiler: &mut Compiler, contexts: &mut Vec<FunctionContext>, e: &Expr) -> Result<(), Error> {
    match e {
        Expr::Int(v) => {
            let idx = compiler.add_constant(Constant::Int(*v));
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Push as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::Variable(name) => {
            if let Some(idx) = contexts.last().unwrap().find_local(name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::LoadLocal as u8);
                ctx.code.push(idx);
            } else if let Some(idx) = resolve_upvalue(contexts, name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::LoadUpvalue as u8);
                ctx.code.push(idx);
            } else {
                return Err(Error::Compile(format!("undefined variable: {}", name)));
            }
        }
        Expr::Add(a, b) => {
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let didx = compiler.add_string("add");
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&didx.to_le_bytes());
            ctx.code.push(2u8);
        }
        Expr::Call(callee, args) => {
            // Check for InvokeMethod pattern: Call(GetField(obj, method), args)
            if let Expr::GetField(obj, field) = &**callee {
                 // Compile receiver
                 compile_expr(compiler, contexts, obj)?;
                 // Compile args
                 for arg in args {
                     compile_expr(compiler, contexts, arg)?;
                 }
                 let name_idx = compiler.add_string(field);
                 let ctx = contexts.last_mut().unwrap();
                 ctx.code.push(Opcode::InvokeMethod as u8);
                 ctx.code.extend_from_slice(&name_idx.to_le_bytes());
                 ctx.code.push(args.len() as u8);
                 return Ok(());
            }

            let mut is_static_or_ffi = false;
            if let Expr::Variable(name) = &**callee {
                // Check for static function first
                let static_func = compiler.functions.get(name).copied();
                
                if let Some(idx) = static_func {
                    // Static function call
                    for arg in args {
                        compile_expr(compiler, contexts, arg)?;
                    }
                    let ctx = contexts.last_mut().unwrap();
                    ctx.code.push(Opcode::Call as u8);
                    ctx.code.extend_from_slice(&idx.to_le_bytes());
                    ctx.code.push(args.len() as u8);
                    is_static_or_ffi = true;
                } else if contexts.last().unwrap().find_local(name).is_none() && resolve_upvalue(contexts, name).is_none() {
                    // FFI call (if not local variable AND not upvalue)
                    for arg in args {
                        compile_expr(compiler, contexts, arg)?;
                    }
                    let didx = compiler.add_string(name);
                    let ctx = contexts.last_mut().unwrap();
                    ctx.code.push(Opcode::FFICall as u8);
                    ctx.code.extend_from_slice(&didx.to_le_bytes());
                    ctx.code.push(args.len() as u8);
                    is_static_or_ffi = true;
                }
            }
            
            if !is_static_or_ffi {
                // Closure Call
                compile_expr(compiler, contexts, callee)?;
                for arg in args {
                    compile_expr(compiler, contexts, arg)?;
                }
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::CallClosure as u8);
                ctx.code.push(args.len() as u8);
            }
        }
        Expr::Closure(args, body) => {
            contexts.push(FunctionContext::new(args.clone()));
            for stmt in body {
                compile_stmt(compiler, contexts, stmt)?;
            }
            let mut func_ctx = contexts.pop().unwrap();
            
            func_ctx.code.push(Opcode::Push as u8);
            let null_idx = compiler.add_constant(Constant::Int(0));
            func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
            func_ctx.code.push(Opcode::Return as u8);

            let chunk_idx = (compiler.chunks.len() + 1) as u16;
            let upvalues = func_ctx.upvalues.clone();
            
            let chunk = Chunk {
                locals: func_ctx.locals.len() as u16,
                upvalues: func_ctx.upvalues.len() as u16,
                max_stack: 16,
                code: func_ctx.code,
                handlers: vec![],
            };
            compiler.chunks.push(chunk);

            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::MakeClosure as u8);
            ctx.code.extend_from_slice(&chunk_idx.to_le_bytes());
            
            // Emit upvalues info
            ctx.code.push(upvalues.len() as u8);
            for (is_local, index) in upvalues {
                ctx.code.push(if is_local { 1 } else { 0 });
                ctx.code.push(index);
            }
        }
        Expr::New(name) => {
            if let Some(&idx) = compiler.class_map.get(name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::NewObject as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", name)));
            }
        }
        Expr::GetField(obj, field) => {
            compile_expr(compiler, contexts, obj)?;
            let idx = compiler.add_string(field);
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::GetField as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::SetField(obj, field, val) => {
            compile_expr(compiler, contexts, obj)?;
            compile_expr(compiler, contexts, val)?;
            let idx = compiler.add_string(field);
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::SetField as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::InstanceOf(expr, class_name) => {
            compile_expr(compiler, contexts, expr)?;
            if let Some(&idx) = compiler.class_map.get(class_name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::InstanceOf as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
        Expr::Cast(expr, class_name) => {
            compile_expr(compiler, contexts, expr)?;
            if let Some(&idx) = compiler.class_map.get(class_name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Cast as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
    }
    Ok(())
}

fn compile_stmt(compiler: &mut Compiler, contexts: &mut Vec<FunctionContext>, s: &Stmt) -> Result<(), Error> {
    match s {
        Stmt::Expr(e) => {
            compile_expr(compiler, contexts, e)?;
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Pop as u8);
        }
        Stmt::Let(name, val) => {
            compile_expr(compiler, contexts, val)?;
            let ctx = contexts.last_mut().unwrap();
            let idx = ctx.add_local(name.clone());
            ctx.code.push(Opcode::StoreLocal as u8);
            ctx.code.push(idx);
        }
        Stmt::Return(val) => {
            compile_expr(compiler, contexts, val)?;
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Return as u8);
        }
        Stmt::FuncDef(name, args, body) => {
            let chunk_idx = compile_func_to_chunk(compiler, args.clone(), body)?;
            compiler.functions.insert(name.clone(), chunk_idx);
        }
        Stmt::ClassDef(name, fields) => {
            let idx = compiler.classes.len() as u16;
            compiler.classes.push(ClassInfo {
                name: name.clone(),
                fields: fields.clone(),
            });
            compiler.class_map.insert(name.clone(), idx);
        }
        Stmt::TraitDef(name, methods) => {
            let idx = compiler.traits.len() as u16;
            compiler.traits.push(TraitInfo {
                name: name.clone(),
                methods: methods.clone(),
            });
            compiler.trait_map.insert(name.clone(), idx);
        }
        Stmt::ImplDef(trait_name, class_name, methods) => {
            let trait_idx = *compiler.trait_map.get(trait_name)
                .ok_or_else(|| Error::Compile(format!("undefined trait: {}", trait_name)))?;
            let class_idx = *compiler.class_map.get(class_name)
                .ok_or_else(|| Error::Compile(format!("undefined class: {}", class_name)))?;

            // Compile all methods in the impl block
            let mut method_map = HashMap::new();
            for method_stmt in methods {
                if let Stmt::FuncDef(name, args, body) = method_stmt {
                    let chunk_idx = compile_func_to_chunk(compiler, args.clone(), body)?;
                    method_map.insert(name.clone(), chunk_idx);
                } else {
                    return Err(Error::Compile("impl block can only contain function definitions".into()));
                }
            }

            // Construct the method table for the implementation based on the trait definition
            let trait_info = &compiler.traits[trait_idx as usize];
            let mut impl_methods = Vec::new();
            for method_name in &trait_info.methods {
                if let Some(&chunk_idx) = method_map.get(method_name) {
                    impl_methods.push(chunk_idx);
                } else {
                    return Err(Error::Compile(format!("missing implementation for method '{}' of trait '{}'", method_name, trait_name)));
                }
            }

            compiler.impls.push(ImplInfo {
                class_idx,
                trait_idx,
                methods: impl_methods,
            });
        }
    }
    Ok(())
}

pub fn compile(stmts: &[Stmt]) -> Result<NyarcModule, Error> {
    let mut compiler = Compiler::new();
    let mut contexts = vec![FunctionContext::new(vec![])];

    for s in stmts {
        compile_stmt(&mut compiler, &mut contexts, s)?;
    }
    
    let mut main_ctx = contexts.pop().unwrap();
    main_ctx.code.push(Opcode::Halt as u8);
    
    let main_chunk = Chunk {
        locals: main_ctx.locals.len() as u16,
        upvalues: 0,
        max_stack: 16,
        code: main_ctx.code,
        handlers: vec![],
    };
    
    let mut all_chunks = vec![main_chunk];
    all_chunks.extend(compiler.chunks);
    
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    Ok(NyarcModule {
        version: 1,
        flags: 0,
        timestamp: ts,
        constants: compiler.constants,
        effects: vec![],
        chunks: all_chunks,
        classes: compiler.classes,
        traits: compiler.traits,
        impls: compiler.impls,
    })
}
