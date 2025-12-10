use crate::ast::{Expr, Stmt};
use crate::lexer::Error;
use nyar_vm::bytecode::format::{Chunk, ClassInfo, Constant, NyarcModule};
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
}

struct FunctionContext {
    code: Vec<u8>,
    locals: Vec<String>,
    args: Vec<String>,
}

impl Compiler {
    fn new() -> Self {
        Self {
            constants: Vec::new(),
            chunks: Vec::new(),
            functions: HashMap::new(),
            classes: Vec::new(),
            class_map: HashMap::new(),
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
}

fn compile_expr(compiler: &mut Compiler, ctx: &mut FunctionContext, e: &Expr) -> Result<(), Error> {
    match e {
        Expr::Int(v) => {
            let idx = compiler.add_constant(Constant::Int(*v));
            ctx.code.push(Opcode::Push as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::Variable(name) => {
            if let Some(idx) = ctx.find_local(name) {
                ctx.code.push(Opcode::LoadLocal as u8);
                ctx.code.push(idx);
            } else {
                return Err(Error::Compile(format!("undefined variable: {}", name)));
            }
        }
        Expr::Add(a, b) => {
            compile_expr(compiler, ctx, a)?;
            compile_expr(compiler, ctx, b)?;
            let didx = compiler.add_string("add");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&didx.to_le_bytes());
            ctx.code.push(2u8);
        }
        Expr::Call(name, args) => {
            for arg in args {
                compile_expr(compiler, ctx, arg)?;
            }
            if let Some(idx) = compiler.functions.get(name) {
                // User function call
                ctx.code.push(Opcode::Call as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(args.len() as u8);
            } else {
                // FFI call
                let didx = compiler.add_string(name);
                ctx.code.push(Opcode::FFICall as u8);
                ctx.code.extend_from_slice(&didx.to_le_bytes());
                ctx.code.push(args.len() as u8);
            }
        }
        Expr::Closure(args, body) => {
            let mut func_ctx = FunctionContext::new(args.clone());
            for stmt in body {
                compile_stmt(compiler, &mut func_ctx, stmt)?;
            }
            func_ctx.code.push(Opcode::Push as u8);
            let null_idx = compiler.add_constant(Constant::Int(0));
            func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
            func_ctx.code.push(Opcode::Return as u8);

            let chunk_idx = (compiler.chunks.len() + 1) as u16;
            let chunk = Chunk {
                locals: func_ctx.locals.len() as u16,
                upvalues: 0,
                max_stack: 16,
                code: func_ctx.code,
                handlers: vec![],
            };
            compiler.chunks.push(chunk);

            ctx.code.push(Opcode::MakeClosure as u8);
            ctx.code.extend_from_slice(&chunk_idx.to_le_bytes());
        }
        Expr::New(name) => {
            if let Some(&idx) = compiler.class_map.get(name) {
                ctx.code.push(Opcode::NewObject as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", name)));
            }
        }
        Expr::GetField(obj, field) => {
            compile_expr(compiler, ctx, obj)?;
            let idx = compiler.add_string(field);
            ctx.code.push(Opcode::GetField as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::SetField(obj, field, val) => {
            compile_expr(compiler, ctx, obj)?;
            compile_expr(compiler, ctx, val)?;
            let idx = compiler.add_string(field);
            ctx.code.push(Opcode::SetField as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::InstanceOf(expr, class_name) => {
            compile_expr(compiler, ctx, expr)?;
            if let Some(&idx) = compiler.class_map.get(class_name) {
                ctx.code.push(Opcode::InstanceOf as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
        Expr::Cast(expr, class_name) => {
            compile_expr(compiler, ctx, expr)?;
            if let Some(&idx) = compiler.class_map.get(class_name) {
                ctx.code.push(Opcode::Cast as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
    }
    Ok(())
}

fn compile_stmt(compiler: &mut Compiler, ctx: &mut FunctionContext, s: &Stmt) -> Result<(), Error> {
    match s {
        Stmt::Expr(e) => {
            compile_expr(compiler, ctx, e)?;
            // Pop result? No, we keep it as implicit return or side effect?
            // VM expects statements to leave stack clean?
            // Usually we should pop if it's an expression statement.
            ctx.code.push(Opcode::Pop as u8);
        }
        Stmt::Let(name, val) => {
            compile_expr(compiler, ctx, val)?;
            let idx = ctx.add_local(name.clone());
            ctx.code.push(Opcode::StoreLocal as u8);
            ctx.code.push(idx);
        }
        Stmt::Return(val) => {
            compile_expr(compiler, ctx, val)?;
            ctx.code.push(Opcode::Return as u8);
        }
        Stmt::FuncDef(name, args, body) => {
            let mut func_ctx = FunctionContext::new(args.clone());
            for stmt in body {
                compile_stmt(compiler, &mut func_ctx, stmt)?;
            }
            func_ctx.code.push(Opcode::Push as u8);
            let null_idx = compiler.add_constant(Constant::Int(0));
            func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
            func_ctx.code.push(Opcode::Return as u8);

            let chunk_idx = compiler.chunks.len() as u16 + 1;
            
            let chunk = Chunk {
                locals: func_ctx.locals.len() as u16,
                upvalues: 0,
                max_stack: 16,
                code: func_ctx.code,
                handlers: vec![],
            };
            compiler.chunks.push(chunk);
            
            let func_idx = (compiler.chunks.len()) as u16; // Chunks are 1-based in module.chunks (0 is main)
            // Wait, compiler.chunks doesn't include main yet.
            // Main will be inserted at index 0.
            // So compiler.chunks[0] will be module.chunks[1].
            // So func_idx should be chunk_idx?
            // Yes, chunk_idx = compiler.chunks.len() + 1.
            // But I just pushed it. So len is now N.
            // So chunk_idx = N. (Since Main is 0).
            // Example: Empty compiler.chunks. Push -> len=1.
            // This is module.chunks[1]. So index 1.
            // Correct.
            
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
    }
    Ok(())
}

pub fn compile(stmts: &[Stmt]) -> Result<NyarcModule, Error> {
    let mut compiler = Compiler::new();
    let mut main_ctx = FunctionContext::new(vec![]);

    for s in stmts {
        compile_stmt(&mut compiler, &mut main_ctx, s)?;
    }
    
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
    })
}
