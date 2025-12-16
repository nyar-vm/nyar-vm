use crate::ast::{Expr, Pattern, Stmt};
use crate::lexer::Error;
use nyar_vm::bytecode::format::{Chunk, ClassInfo, Constant, ImplInfo, NyarcModule, TraitInfo};
use nyar_vm::bytecode::opcode::Opcode;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeKind {
    Int,
    Bool,
    String,
    Unknown,
}

impl TypeKind {
    fn is_int(self) -> bool {
        matches!(self, TypeKind::Int)
    }
    fn is_bool(self) -> bool {
        matches!(self, TypeKind::Bool)
    }
}

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
    namespace_stack: Vec<String>,
    use_prefixes: Vec<Vec<String>>, // list of namespace paths opened by `using`
}

struct FunctionContext {
    code: Vec<u8>,
    locals: Vec<String>,
    upvalues: Vec<(bool, u8)>, // (is_local, index)
    loops: Vec<LoopContext>,
    local_types: HashMap<String, TypeKind>,
    lines: Vec<(u32, u32)>, // offset, line
}

#[derive(Clone)]
struct LoopContext {
    start_pos: usize,
    cond_pos: Option<usize>,
    breaks: Vec<usize>,
    continues: Vec<usize>,
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
            namespace_stack: Vec::new(),
            use_prefixes: Vec::new(),
        }
    }

    fn add_constant(&mut self, c: Constant) -> u16 {
        if let Some((i, _)) = self.constants.iter().enumerate().find(|(_, v)| *v == &c) {
            i as u16
        } else {
            let i = self.constants.len() as u16;
            println!("DEBUG: Added constant {}: {:?}", i, c);
            self.constants.push(c);
            i
        }
    }

    fn add_string(&mut self, s: &str) -> u16 {
        self.add_constant(Constant::String(s.to_string()))
    }

    fn qualify(&self, name: &str) -> String {
        if self.namespace_stack.is_empty() {
            name.to_string()
        } else {
            format!("{}::{name}", self.namespace_stack.join("::"))
        }
    }

    fn def_name(&self, name: &str) -> String {
        if name.contains("::") {
            name.to_string()
        } else {
            self.qualify(name)
        }
    }

    fn resolve_function(&self, name: &str) -> Option<u16> {
        if name.contains("Token") {
            println!("DEBUG: Resolving function: {}", name);
        }
        if let Some(&idx) = self.functions.get(name) {
            if name.contains("Token") {
                println!("DEBUG: Found exact: {} -> {}", name, idx);
            }
            return Some(idx);
        }
        let q = self.qualify(name);
        if name.contains("Token") {
            println!("DEBUG: Resolving function qualified: {}", q);
        }
        if let Some(&idx) = self.functions.get(&q) {
            if name.contains("Token") {
                println!("DEBUG: Found qualified: {} -> {}", q, idx);
            }
            return Some(idx);
        }
        for p in &self.use_prefixes {
            let qname = format!("{}::{name}", p.join("::"));
            if name.contains("Token") {
                println!("DEBUG: Resolving function imported: {}", qname);
            }
            if let Some(&idx) = self.functions.get(&qname) {
                return Some(idx);
            }
        }
        None
    }

    fn resolve_class(&self, name: &str) -> Option<u16> {
        if let Some(&idx) = self.class_map.get(name) {
            return Some(idx);
        }
        let q = self.qualify(name);
        if let Some(&idx) = self.class_map.get(&q) {
            return Some(idx);
        }
        for p in &self.use_prefixes {
            let qname = format!("{}::{name}", p.join("::"));
            if let Some(&idx) = self.class_map.get(&qname) {
                return Some(idx);
            }
        }
        for (k, &v) in &self.class_map {
            if k.ends_with(&format!("::{}", name)) || k == name {
                return Some(v);
            }
        }
        None
    }

    fn resolve_trait(&self, name: &str) -> Option<u16> {
        if let Some(&idx) = self.trait_map.get(name) {
            return Some(idx);
        }
        let q = self.qualify(name);
        if let Some(&idx) = self.trait_map.get(&q) {
            return Some(idx);
        }
        for p in &self.use_prefixes {
            let qname = format!("{}::{name}", p.join("::"));
            if let Some(&idx) = self.trait_map.get(&qname) {
                return Some(idx);
            }
        }
        None
    }
}

impl FunctionContext {
    fn new(args: Vec<String>) -> Self {
        Self {
            code: Vec::new(),
            locals: args, // Arguments are the first locals
            upvalues: Vec::new(),
            loops: Vec::new(),
            local_types: HashMap::new(),
            lines: Vec::new(),
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

    fn set_local_type(&mut self, name: String, ty: TypeKind) {
        self.local_types.insert(name, ty);
    }

    fn get_local_type(&self, name: &str) -> Option<TypeKind> {
        self.local_types.get(name).copied()
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

fn infer_expr_type(contexts: &[FunctionContext], e: &Expr) -> TypeKind {
    match e {
        Expr::Int(_) => TypeKind::Int,
        Expr::String(_) => TypeKind::String,
        Expr::Bool(_) => TypeKind::Bool,
        Expr::Variable(name) => {
            if let Some(ctx) = contexts.last() {
                ctx.get_local_type(name).unwrap_or(TypeKind::Unknown)
            } else {
                TypeKind::Unknown
            }
        }
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) => {
            let ta = infer_expr_type(contexts, a);
            let tb = infer_expr_type(contexts, b);
            if ta.is_int() && tb.is_int() {
                TypeKind::Int
            } else {
                TypeKind::Unknown
            }
        }
        Expr::Eq(a, b)
        | Expr::Ne(a, b)
        | Expr::Lt(a, b)
        | Expr::Le(a, b)
        | Expr::Gt(a, b)
        | Expr::Ge(a, b) => {
            let ta = infer_expr_type(contexts, a);
            let tb = infer_expr_type(contexts, b);
            if ta.is_int() && tb.is_int() {
                TypeKind::Bool
            } else {
                TypeKind::Unknown
            }
        }
        Expr::And(a, b) | Expr::Or(a, b) => {
            let ta = infer_expr_type(contexts, a);
            let tb = infer_expr_type(contexts, b);
            if ta.is_bool() && tb.is_bool() {
                TypeKind::Bool
            } else {
                TypeKind::Unknown
            }
        }
        Expr::Not(a) => {
            let ta = infer_expr_type(contexts, a);
            if ta.is_bool() {
                TypeKind::Bool
            } else {
                TypeKind::Unknown
            }
        }
        Expr::Neg(a) => {
            let ta = infer_expr_type(contexts, a);
            if ta.is_int() {
                TypeKind::Int
            } else {
                TypeKind::Unknown
            }
        }
        Expr::TypeOf(_) => TypeKind::Unknown,
        Expr::Call(_, _) => TypeKind::Unknown,
        Expr::Closure(_, _) => TypeKind::Unknown,
        Expr::New(_) => TypeKind::Unknown,
        Expr::GetField(_, _) => TypeKind::Unknown,
        Expr::SetField(_, _, _) => TypeKind::Unknown,
        Expr::SetLocal(_, v) => infer_expr_type(contexts, v),
        Expr::InstanceOf(_, _) => TypeKind::Bool,
        Expr::Cast(_, _) => TypeKind::Unknown,
        Expr::CheckCast(_, _) => TypeKind::Unknown,
        Expr::Match(_, _) => TypeKind::Unknown,
    }
}

fn instr_size(code: &[u8], pos: usize) -> usize {
    if pos >= code.len() {
        return 0;
    }
    match code[pos] {
        x if x == Opcode::Nop as u8 => 1,
        x if x == Opcode::Push as u8 => 3,
        x if x == Opcode::Pop as u8 => 1,
        x if x == Opcode::Dup as u8 => 2,
        x if x == Opcode::Swap as u8 => 2,
        x if x == Opcode::LoadLocal as u8 => 2,
        x if x == Opcode::StoreLocal as u8 => 2,
        x if x == Opcode::LoadGlobal as u8 => 3,
        x if x == Opcode::StoreGlobal as u8 => 3,
        x if x == Opcode::LoadUpvalue as u8 => 2,
        x if x == Opcode::StoreUpvalue as u8 => 2,
        x if x == Opcode::CloseUpvalues as u8 => 1,
        x if x == Opcode::Jump as u8 => 3,
        x if x == Opcode::JumpIfFalse as u8 => 3,
        x if x == Opcode::JumpIfNull as u8 => 3,
        x if x == Opcode::Return as u8 => 1,
        x if x == Opcode::MakeClosure as u8 => {
            let upc = if pos + 3 < code.len() {
                code[pos + 3] as usize
            } else {
                0
            };
            1 + 2 + 1 + (2 * upc)
        }
        x if x == Opcode::TailCall as u8 => 1,
        x if x == Opcode::Call as u8 => 4,
        x if x == Opcode::CallVirtual as u8 => 4,
        x if x == Opcode::CallDynamic as u8 => 4,
        x if x == Opcode::CallClosure as u8 => 2,
        x if x == Opcode::InvokeMethod as u8 => 4,
        x if x == Opcode::GetField as u8 => 3,
        x if x == Opcode::SetField as u8 => 3,
        x if x == Opcode::NewObject as u8 => 3,
        x if x == Opcode::NewArray as u8 => 3,
        x if x == Opcode::GetElement as u8 => 1,
        x if x == Opcode::SetElement as u8 => 1,
        x if x == Opcode::TypeOf as u8 => 1,
        x if x == Opcode::InstanceOf as u8 => 3,
        x if x == Opcode::CheckCast as u8 => 3,
        x if x == Opcode::Cast as u8 => 3,
        x if x == Opcode::Perform as u8 => 4,
        x if x == Opcode::WithHandler as u8 => 3,
        x if x == Opcode::ResumeWith as u8 => 1,
        x if x == Opcode::CaptureCont as u8 => 1,
        x if x == Opcode::GetWitnessTable as u8 => 5,
        x if x == Opcode::WitnessMethod as u8 => 3,
        x if x == Opcode::OpenExistential as u8 => 1,
        x if x == Opcode::CloseExistential as u8 => 1,
        x if x == Opcode::Quote as u8 => 5,
        x if x == Opcode::Splice as u8 => 1,
        x if x == Opcode::Eval as u8 => 2,
        x if x == Opcode::ExpandMacro as u8 => 4,
        x if x == Opcode::FFICall as u8 => 4,
        x if x == Opcode::Halt as u8 => 1,
        _ => 1,
    }
}

fn count_instructions(code: &[u8], from_opcode_pos: usize, to_opcode_pos: usize) -> i16 {
    let mut p = from_opcode_pos;
    let mut n = 0i16;
    while p < to_opcode_pos {
        let sz = instr_size(code, p);
        if sz == 0 {
            break;
        }
        p += sz;
        n += 1;
    }
    n
}

fn resolve_upvalue(contexts: &mut [FunctionContext], name: &str) -> Option<u8> {
    let len = contexts.len();
    if len < 2 {
        return None;
    }

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

fn compile_func_to_chunk(
    compiler: &mut Compiler,
    args: Vec<String>,
    body: &[Stmt],
) -> Result<u16, Error> {
    let mut contexts = vec![FunctionContext::new(args)];

    let len = body.len();
    if len == 0 {
        let func_ctx = contexts.last_mut().unwrap();
        func_ctx.code.push(Opcode::Push as u8);
        let null_idx = compiler.add_constant(Constant::Int(0));
        func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
        func_ctx.code.push(Opcode::Return as u8);
    } else {
        for (i, stmt) in body.iter().enumerate() {
            if i == len - 1 {
                match stmt {
                    Stmt::Expr(e) => {
                        compile_expr(compiler, &mut contexts, e)?;
                        let func_ctx = contexts.last_mut().unwrap();
                        func_ctx.code.push(Opcode::Return as u8);
                    }
                    Stmt::Return(_) => {
                        compile_stmt(compiler, &mut contexts, stmt)?;
                    }
                    _ => {
                        compile_stmt(compiler, &mut contexts, stmt)?;
                        let func_ctx = contexts.last_mut().unwrap();
                        func_ctx.code.push(Opcode::Push as u8);
                        let null_idx = compiler.add_constant(Constant::Int(0));
                        func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
                        func_ctx.code.push(Opcode::Return as u8);
                    }
                }
            } else {
                compile_stmt(compiler, &mut contexts, stmt)?;
            }
        }
    }

    let func_ctx = contexts.pop().unwrap();

    let chunk = Chunk {
        locals: func_ctx.locals.len() as u16,
        upvalues: func_ctx.upvalues.len() as u16,
        max_stack: 16,
        code: func_ctx.code,
        handlers: vec![],
        lines: func_ctx.lines,
    };

    // Chunk index in the final module will be (existing chunks) + 1 (for main) + 1 (this new one)?
    // No, compiler.chunks contains all chunks except main.
    // So index 0 in compiler.chunks is index 1 in module.chunks.
    // So if compiler.chunks has N items, the next item is at index N+1.
    let chunk_idx = (compiler.chunks.len() + 1) as u16;
    compiler.chunks.push(chunk);
    Ok(chunk_idx)
}

fn compile_expr(
    compiler: &mut Compiler,
    contexts: &mut Vec<FunctionContext>,
    e: &Expr,
) -> Result<(), Error> {
    match e {
        Expr::Int(v) => {
            let idx = compiler.add_constant(Constant::Int(*v));
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Push as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::String(s) => {
            let idx = compiler.add_constant(Constant::String(s.clone()));
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Push as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::Bool(b) => {
            let s = if *b { "true" } else { "false" };
            let idx = compiler.add_string(s);
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
            ctx.code.push(0u8); // 0 args
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
            } else if name == "null" {
                let ctx = contexts.last_mut().unwrap();
                let idx = compiler.add_constant(Constant::Int(0));
                ctx.code.push(Opcode::Push as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else if let Some(idx) = compiler.resolve_function(name) {
                // Resolved as a global function (possibly enum variant constructor)
                // We should treat it as a function call with 0 arguments if it is a unit variant,
                // OR return a closure.
                // For now, assuming unit variants are called immediately if used as variable.
                // But wait, if I use it as `let x = Token::EOF`, x becomes the token.
                // If I use it as `let f = Token::Int`, f becomes the constructor.
                // Since we don't have type info here easily, let's assume we call it with 0 args.
                // If it expects args, runtime will fail or stack underflow.
                // But for Token::EOF it is correct.
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Call as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(0u8);
            } else {
                return Err(Error::Compile(format!("undefined variable: {}", name)));
            }
        }
        Expr::Add(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::Add as u8);
            } else {
                let idx = compiler.add_string("add");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Sub(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::Sub as u8);
            } else {
                let idx = compiler.add_string("sub");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Mul(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::Mul as u8);
            } else {
                let idx = compiler.add_string("mul");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Div(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::DivS as u8);
            } else {
                let idx = compiler.add_string("div");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::And(a, b) => {
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let idx = compiler.add_string("and");
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::InvokeMethod as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
            ctx.code.push(1u8);
        }
        Expr::Or(a, b) => {
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let idx = compiler.add_string("or");
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::InvokeMethod as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
            ctx.code.push(1u8);
        }
        Expr::Eq(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::Eq as u8);
            } else {
                let name_idx = compiler.add_string("eq");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&name_idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Ne(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::Ne as u8);
            } else {
                let name_idx = compiler.add_string("ne");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&name_idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Lt(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::LtS as u8);
            } else {
                let idx = compiler.add_string("lt");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Le(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::LeS as u8);
            } else {
                let idx = compiler.add_string("le");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Gt(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::GtS as u8);
            } else {
                let idx = compiler.add_string("gt");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Ge(a, b) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int
                && infer_expr_type(contexts, b) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            compile_expr(compiler, contexts, b)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::GeS as u8);
            } else {
                let idx = compiler.add_string("ge");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(1u8);
            }
        }
        Expr::Not(a) => {
            compile_expr(compiler, contexts, a)?;
            let idx = compiler.add_string("not");
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::InvokeMethod as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
            ctx.code.push(0u8);
        }
        Expr::Neg(a) => {
            let is_int = infer_expr_type(contexts, a) == TypeKind::Int;
            compile_expr(compiler, contexts, a)?;
            let ctx = contexts.last_mut().unwrap();
            if is_int {
                ctx.code.push(Opcode::I64Ext as u8);
                ctx.code.push(nyar_vm::bytecode::opcode::I64Ext::Neg as u8);
            } else {
                let idx = compiler.add_string("neg");
                ctx.code.push(Opcode::InvokeMethod as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
                ctx.code.push(0u8);
            }
        }
        Expr::TypeOf(a) => {
            compile_expr(compiler, contexts, a)?;
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::TypeOf as u8);
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
                if name == "__macro_type_of" {
                     if let Some(arg) = args.first() {
                         let ty = infer_expr_type(contexts, arg);
                         let s = match ty {
                             TypeKind::Int => "i64",
                             TypeKind::String => "string",
                             TypeKind::Bool => "bool",
                             _ => "any",
                         };
                         let idx = compiler.add_constant(Constant::String(s.to_string()));
                         let ctx = contexts.last_mut().unwrap();
                         ctx.code.push(Opcode::Push as u8);
                         ctx.code.extend_from_slice(&idx.to_le_bytes());
                         return Ok(());
                     }
                }

                // Check for static function first with namespace/using resolution
                if let Some(idx) = compiler.resolve_function(name) {
                    println!("DEBUG: resolving call {} -> function idx {}", name, idx);
                    // Static function call
                    for arg in args {
                        compile_expr(compiler, contexts, arg)?;
                    }
                    let ctx = contexts.last_mut().unwrap();
                    ctx.code.push(Opcode::Call as u8);
                    ctx.code.extend_from_slice(&idx.to_le_bytes());
                    ctx.code.push(args.len() as u8);
                    is_static_or_ffi = true;
                } else if contexts.last().unwrap().find_local(name).is_none()
                    && resolve_upvalue(contexts, name).is_none()
                {
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

            let len = body.len();
            if len == 0 {
                let func_ctx = contexts.last_mut().unwrap();
                func_ctx.code.push(Opcode::Push as u8);
                let null_idx = compiler.add_constant(Constant::Int(0));
                func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
                func_ctx.code.push(Opcode::Return as u8);
            } else {
                for (i, stmt) in body.iter().enumerate() {
                    if i == len - 1 {
                        match stmt {
                            Stmt::Expr(e) => {
                                compile_expr(compiler, contexts, e)?;
                                let func_ctx = contexts.last_mut().unwrap();
                                func_ctx.code.push(Opcode::Return as u8);
                            }
                            Stmt::Return(_) => {
                                compile_stmt(compiler, contexts, stmt)?;
                            }
                            _ => {
                                compile_stmt(compiler, contexts, stmt)?;
                                let func_ctx = contexts.last_mut().unwrap();
                                func_ctx.code.push(Opcode::Push as u8);
                                let null_idx = compiler.add_constant(Constant::Int(0));
                                func_ctx.code.extend_from_slice(&null_idx.to_le_bytes());
                                func_ctx.code.push(Opcode::Return as u8);
                            }
                        }
                    } else {
                        compile_stmt(compiler, contexts, stmt)?;
                    }
                }
            }

            let func_ctx = contexts.pop().unwrap();

            let chunk_idx = (compiler.chunks.len() + 1) as u16;
            let upvalues = func_ctx.upvalues.clone();

            let chunk = Chunk {
                locals: func_ctx.locals.len() as u16,
                upvalues: func_ctx.upvalues.len() as u16,
                max_stack: 16,
                code: func_ctx.code,
                handlers: vec![],
                lines: func_ctx.lines,
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
            if let Some(idx) = compiler.resolve_class(name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::NewObject as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", name)));
            }
        }
        Expr::GetField(obj, field) => {
            if let Expr::Variable(name) = &**obj {
                if name == "__macro_location" {
                    if field == "line_number" {
                         let idx = compiler.add_constant(Constant::Int(0));
                         let ctx = contexts.last_mut().unwrap();
                         ctx.code.push(Opcode::Push as u8);
                         ctx.code.extend_from_slice(&idx.to_le_bytes());
                         return Ok(());
                    }
                }
            }
            compile_expr(compiler, contexts, obj)?;
            let idx = compiler.add_string(field);
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::GetField as u8);
            ctx.code.extend_from_slice(&idx.to_le_bytes());
        }
        Expr::SetLocal(name, val) => {
            // Evaluate right-hand side
            compile_expr(compiler, contexts, val)?;
            // Duplicate the value so the assignment expression yields the value
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Dup as u8);
            ctx.code.push(0u8);
            // Store into local or upvalue
            if let Some(idx) = ctx.find_local(name) {
                ctx.code.push(Opcode::StoreLocal as u8);
                ctx.code.push(idx);
            } else if let Some(idx) = resolve_upvalue(contexts, name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::StoreUpvalue as u8);
                ctx.code.push(idx);
            } else {
                return Err(Error::Compile(format!(
                    "undefined variable for assignment: {}",
                    name
                )));
            }
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
            if let Some(idx) = compiler.resolve_class(class_name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::InstanceOf as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
        Expr::Cast(expr, class_name) => {
            compile_expr(compiler, contexts, expr)?;
            if let Some(idx) = compiler.resolve_class(class_name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Cast as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
        Expr::CheckCast(expr, class_name) => {
            compile_expr(compiler, contexts, expr)?;
            if let Some(idx) = compiler.resolve_class(class_name) {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::CheckCast as u8);
                ctx.code.extend_from_slice(&idx.to_le_bytes());
            } else {
                return Err(Error::Compile(format!("undefined class: {}", class_name)));
            }
        }
        Expr::Match(target, branches) => {
            compile_expr(compiler, contexts, target)?;

            let mut end_jumps = Vec::new();

            for (pat, body) in branches {
                let jump_idx = {
                    let ctx = contexts.last_mut().unwrap();
                    ctx.code.push(Opcode::Dup as u8);
                    ctx.code.push(0u8);
                    compile_pattern_check(compiler, ctx, pat)?;

                    ctx.code.push(Opcode::JumpIfFalse as u8);
                    let idx = ctx.code.len();
                    ctx.code.extend_from_slice(&0u16.to_le_bytes());

                    compile_pattern_binding(compiler, ctx, pat)?;
                    ctx.code.push(Opcode::Pop as u8);
                    idx
                };

                let len = body.len();
                if len == 0 {
                    let ctx = contexts.last_mut().unwrap();
                    ctx.code.push(Opcode::Push as u8);
                    let null_idx = compiler.add_constant(Constant::Int(0));
                    ctx.code.extend_from_slice(&null_idx.to_le_bytes());
                } else {
                    for (i, s) in body.iter().enumerate() {
                        if i == len - 1 {
                            match s {
                                Stmt::Expr(e) => {
                                    compile_expr(compiler, contexts, e)?;
                                }
                                _ => {
                                    compile_stmt(compiler, contexts, s)?;
                                    let ctx = contexts.last_mut().unwrap();
                                    ctx.code.push(Opcode::Push as u8);
                                    let null_idx = compiler.add_constant(Constant::Int(0));
                                    ctx.code.extend_from_slice(&null_idx.to_le_bytes());
                                }
                            }
                        } else {
                            compile_stmt(compiler, contexts, s)?;
                        }
                    }
                }

                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Jump as u8);
                let end_jump_idx = ctx.code.len();
                ctx.code.extend_from_slice(&0u16.to_le_bytes());
                end_jumps.push(end_jump_idx);

                let next_branch_offset = (ctx.code.len() - (jump_idx + 2)) as u16;
                let bytes = next_branch_offset.to_le_bytes();
                ctx.code[jump_idx] = bytes[0];
                ctx.code[jump_idx + 1] = bytes[1];
            }

            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Pop as u8);
            ctx.code.push(Opcode::Push as u8);
            let null_idx = compiler.add_constant(Constant::Int(0));
            ctx.code.extend_from_slice(&null_idx.to_le_bytes());

            let end_pos = ctx.code.len();
            for idx in end_jumps {
                let offset = (end_pos - (idx + 2)) as u16;
                let bytes = offset.to_le_bytes();
                ctx.code[idx] = bytes[0];
                ctx.code[idx + 1] = bytes[1];
            }
        }
    }
    Ok(())
}

fn compile_pattern_check(
    compiler: &mut Compiler,
    ctx: &mut FunctionContext,
    pat: &Pattern,
) -> Result<(), Error> {
    match pat {
        Pattern::Literal(val) => {
            ctx.code.push(Opcode::Push as u8);
            let idx = compiler.add_constant(Constant::Int(*val));
            ctx.code.extend_from_slice(&idx.to_le_bytes());

            let eq_idx = compiler.add_string("eq");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&eq_idx.to_le_bytes());
            ctx.code.push(2u8);
        }
        Pattern::LiteralString(val) => {
            ctx.code.push(Opcode::Push as u8);
            let idx = compiler.add_string(val);
            ctx.code.extend_from_slice(&idx.to_le_bytes());

            let eq_idx = compiler.add_string("eq");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&eq_idx.to_le_bytes());
            ctx.code.push(2u8);
        }
        Pattern::Wildcard | Pattern::Variable(_) => {
            ctx.code.push(Opcode::Pop as u8);
            let t_idx = compiler.add_string("true");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&t_idx.to_le_bytes());
            ctx.code.push(0u8);
        }
        Pattern::Constructor(name, sub_pats) => {
            ctx.code.push(Opcode::Dup as u8);
            ctx.code.push(0u8);
            ctx.code.push(Opcode::GetField as u8);
            let v_idx = compiler.add_string("__variant__");
            ctx.code.extend_from_slice(&v_idx.to_le_bytes());

            ctx.code.push(Opcode::Push as u8);
            let short_name = name.split("::").last().unwrap_or(name);
            let n_idx = compiler.add_string(short_name);
            ctx.code.extend_from_slice(&n_idx.to_le_bytes());

            let eq_idx = compiler.add_string("eq");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&eq_idx.to_le_bytes());
            ctx.code.push(2u8);

            let mut jumps = Vec::new();
            ctx.code.push(Opcode::JumpIfFalse as u8);
            let j = ctx.code.len();
            ctx.code.extend_from_slice(&0u16.to_le_bytes());
            jumps.push(j);

            for (i, p) in sub_pats.iter().enumerate() {
                ctx.code.push(Opcode::Dup as u8);
                ctx.code.push(0u8);
                ctx.code.push(Opcode::GetField as u8);
                let field_name = format!("_{}", i);
                let f_idx = compiler.add_string(&field_name);
                ctx.code.extend_from_slice(&f_idx.to_le_bytes());

                compile_pattern_check(compiler, ctx, p)?;

                ctx.code.push(Opcode::JumpIfFalse as u8);
                let j = ctx.code.len();
                ctx.code.extend_from_slice(&0u16.to_le_bytes());
                jumps.push(j);
            }

            // Success path
            ctx.code.push(Opcode::Pop as u8); // Pop target
            let t_idx = compiler.add_string("true");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&t_idx.to_le_bytes());
            ctx.code.push(0u8);

            ctx.code.push(Opcode::Jump as u8);
            let success_jump = ctx.code.len();
            ctx.code.extend_from_slice(&0u16.to_le_bytes());

            // Fail path
            let fail_pos = ctx.code.len();
            for j in jumps {
                let offset = (fail_pos - (j + 2)) as u16;
                let bytes = offset.to_le_bytes();
                ctx.code[j] = bytes[0];
                ctx.code[j + 1] = bytes[1];
            }

            ctx.code.push(Opcode::Pop as u8); // Pop target
            let f_idx = compiler.add_string("false");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&f_idx.to_le_bytes());
            ctx.code.push(0u8);

            let success_pos = ctx.code.len();
            let offset = (success_pos - (success_jump + 2)) as u16;
            let bytes = offset.to_le_bytes();
            ctx.code[success_jump] = bytes[0];
            ctx.code[success_jump + 1] = bytes[1];
        }
    }
    Ok(())
}

fn compile_pattern_binding(
    compiler: &mut Compiler,
    ctx: &mut FunctionContext,
    pat: &Pattern,
) -> Result<(), Error> {
    match pat {
        Pattern::Variable(name) => {
            ctx.code.push(Opcode::Dup as u8);
            ctx.code.push(0u8);
            let idx = ctx.add_local(name.clone());
            ctx.code.push(Opcode::StoreLocal as u8);
            ctx.code.push(idx);
        }
        Pattern::Constructor(_name, sub_pats) => {
            for (i, p) in sub_pats.iter().enumerate() {
                ctx.code.push(Opcode::Dup as u8);
                ctx.code.push(0u8);
                ctx.code.push(Opcode::GetField as u8);
                let field_name = format!("_{}", i);
                let f_idx = compiler.add_string(&field_name);
                ctx.code.extend_from_slice(&f_idx.to_le_bytes());

                compile_pattern_binding(compiler, ctx, p)?;

                ctx.code.push(Opcode::Pop as u8);
            }
        }
        _ => {}
    }
    Ok(())
}

fn compile_stmt(
    compiler: &mut Compiler,
    contexts: &mut Vec<FunctionContext>,
    s: &Stmt,
) -> Result<(), Error> {
    println!("DEBUG: compile_stmt {:?}", s);
    match s {
        Stmt::Decorated(_decorators, stmt) => {
             compile_stmt(compiler, contexts, stmt)?;
        }
        Stmt::NamespaceSet(path) => {
            compiler.namespace_stack = path.clone();
        }
        Stmt::NamespaceDef(name, body) => {
            let base_uses = compiler.use_prefixes.len();
            compiler.namespace_stack.push(name.clone());
            for stmt in body {
                compile_stmt(compiler, contexts, stmt)?;
            }
            compiler.namespace_stack.pop();
            compiler.use_prefixes.truncate(base_uses);
        }
        Stmt::Using(path) => {
            compiler.use_prefixes.push(path.clone());
        }
        Stmt::ImplyDef(class_name, methods) => {
            println!("DEBUG: compiling ImplyDef for {}", class_name);
            for m in methods {
                match m {
                    Stmt::FuncDef(name, args, body) => {
                        let qname = format!("{}::{}", class_name, name);
                        println!("DEBUG: compiling method {}", qname);
                        compile_stmt(
                            compiler,
                            contexts,
                            &Stmt::FuncDef(qname, args.clone(), body.clone()),
                        )?;
                    }
                    _ => {
                        return Err(Error::Compile(
                            "imply block can only contain function definitions".into(),
                        ))
                    }
                }
            }
        }
        Stmt::Assert(cond, msg) => {
            compile_expr(compiler, contexts, cond)?;
            let j_false = {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::JumpIfFalse as u8);
                let idx = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                idx
            };
            let j_end = {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Jump as u8);
                let idx = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                idx
            };
            let fail_pos = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            if let Some(e) = msg {
                compile_expr(compiler, contexts, e)?;
                let didx = compiler.add_string("assert");
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::FFICall as u8);
                ctx.code.extend_from_slice(&didx.to_le_bytes());
                ctx.code.push(1u8);
            } else {
                let didx = compiler.add_string("assert");
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::FFICall as u8);
                ctx.code.extend_from_slice(&didx.to_le_bytes());
                ctx.code.push(0u8);
            }
            let end_pos = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                let jump_pos = j_false - 1;
                let off = count_instructions(&ctx.code, jump_pos, fail_pos) as i16;
                let bytes = off.to_le_bytes();
                ctx.code[j_false] = bytes[0];
                ctx.code[j_false + 1] = bytes[1];
                let jump_pos2 = j_end - 1;
                let off2 = count_instructions(&ctx.code, jump_pos2, end_pos) as i16;
                let b2 = off2.to_le_bytes();
                ctx.code[j_end] = b2[0];
                ctx.code[j_end + 1] = b2[1];
            }
        }
        Stmt::Debug(e) => {
            compile_expr(compiler, contexts, e)?;
            let ctx = contexts.last_mut().unwrap();
            let didx = compiler.add_string("print");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&didx.to_le_bytes());
            ctx.code.push(1u8);
            ctx.code.push(Opcode::Pop as u8);
        }

        Stmt::If(cond, then_body, else_body) => {
            compile_expr(compiler, contexts, cond)?;
            let j_false = {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::JumpIfFalse as u8);
                let idx = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                idx
            };

            for stmt in then_body {
                compile_stmt(compiler, contexts, stmt)?;
            }

            let j_end = {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Jump as u8);
                let idx = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                let else_pos = ctx.code.len();
                let jump_pos = j_false - 1;
                let off = count_instructions(&ctx.code, jump_pos, else_pos);
                let bytes = (off as i16).to_le_bytes();
                ctx.code[j_false] = bytes[0];
                ctx.code[j_false + 1] = bytes[1];
                idx
            };

            if let Some(body) = else_body {
                for stmt in body {
                    compile_stmt(compiler, contexts, &stmt)?;
                }
            }

            let ctx = contexts.last_mut().unwrap();
            let end_pos = ctx.code.len();
            let jump_pos = j_end - 1;
            let off = count_instructions(&ctx.code, jump_pos, end_pos) as i16;
            let bytes = off.to_le_bytes();
            ctx.code[j_end] = bytes[0];
            ctx.code[j_end + 1] = bytes[1];
        }
        Stmt::While(cond, body) => {
            let start_pos = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                ctx.loops.push(LoopContext {
                    start_pos,
                    cond_pos: Some(start_pos),
                    breaks: Vec::new(),
                    continues: Vec::new(),
                });
            }

            compile_expr(compiler, contexts, cond)?;
            let j_exit = {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::JumpIfFalse as u8);
                let idx = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                idx
            };

            for stmt in body {
                compile_stmt(compiler, contexts, stmt)?;
            }

            {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Jump as u8);
                let jpos = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                let jump_pos = jpos - 1;
                let off = -(count_instructions(&ctx.code, start_pos, jump_pos));
                let bytes = (off as i16).to_le_bytes();
                ctx.code[jpos] = bytes[0];
                ctx.code[jpos + 1] = bytes[1];
            }

            let end_pos = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                let jump_pos = j_exit - 1;
                let off = count_instructions(&ctx.code, jump_pos, end_pos) as i16;
                let bytes = off.to_le_bytes();
                ctx.code[j_exit] = bytes[0];
                ctx.code[j_exit + 1] = bytes[1];
            }

            let lc = {
                let ctx = contexts.last_mut().unwrap();
                ctx.loops.pop().unwrap()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                for idx in lc.breaks {
                    let jump_pos = idx - 1;
                    let off = count_instructions(&ctx.code, jump_pos, end_pos) as i16;
                    let b = off.to_le_bytes();
                    ctx.code[idx] = b[0];
                    ctx.code[idx + 1] = b[1];
                }
                if let Some(cpos) = lc.cond_pos {
                    for idx in lc.continues {
                        let jump_pos = idx - 1;
                        let off = -(count_instructions(&ctx.code, cpos, jump_pos) as i16);
                        let b = off.to_le_bytes();
                        ctx.code[idx] = b[0];
                        ctx.code[idx + 1] = b[1];
                    }
                }
            }
        }
        Stmt::Loop(body) => {
            let start_pos = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                ctx.loops.push(LoopContext {
                    start_pos,
                    cond_pos: None,
                    breaks: Vec::new(),
                    continues: Vec::new(),
                });
            }

            for stmt in body {
                compile_stmt(compiler, contexts, stmt)?;
            }

            let end_pos = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                ctx.code.push(Opcode::Jump as u8);
                let jpos = ctx.code.len();
                ctx.code.extend_from_slice(&0i16.to_le_bytes());
                let jump_pos = jpos - 1;
                let off = -(count_instructions(&ctx.code, start_pos, jump_pos));
                let b = (off as i16).to_le_bytes();
                ctx.code[jpos] = b[0];
                ctx.code[jpos + 1] = b[1];
            }
            let end_pos2 = {
                let ctx = contexts.last().unwrap();
                ctx.code.len()
            };
            let lc = {
                let ctx = contexts.last_mut().unwrap();
                ctx.loops.pop().unwrap()
            };
            {
                let ctx = contexts.last_mut().unwrap();
                for idx in lc.breaks {
                    let jump_pos = idx - 1;
                    let off = count_instructions(&ctx.code, jump_pos, end_pos2) as i16;
                    let b = off.to_le_bytes();
                    ctx.code[idx] = b[0];
                    ctx.code[idx + 1] = b[1];
                }
                for idx in lc.continues {
                    let jump_pos = idx - 1;
                    let off = -(count_instructions(&ctx.code, start_pos, jump_pos) as i16);
                    let b = off.to_le_bytes();
                    ctx.code[idx] = b[0];
                    ctx.code[idx + 1] = b[1];
                }
            }
        }
        Stmt::Break => {
            let ctx = contexts.last_mut().unwrap();
            if ctx.loops.is_empty() {
                return Err(Error::Compile("break outside loop".into()));
            }
            ctx.code.push(Opcode::Jump as u8);
            let idx = ctx.code.len();
            ctx.code.extend_from_slice(&0i16.to_le_bytes());
            let last = ctx.loops.len() - 1;
            ctx.loops[last].breaks.push(idx);
        }
        Stmt::Continue => {
            let ctx = contexts.last_mut().unwrap();
            if ctx.loops.is_empty() {
                return Err(Error::Compile("continue outside loop".into()));
            }
            ctx.code.push(Opcode::Jump as u8);
            let idx = ctx.code.len();
            ctx.code.extend_from_slice(&0i16.to_le_bytes());
            let last = ctx.loops.len() - 1;
            ctx.loops[last].continues.push(idx);
        }
        Stmt::Expr(e) => {
            compile_expr(compiler, contexts, e)?;
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Pop as u8);
        }
        Stmt::Let(name, val) => {
            let ty = infer_expr_type(&contexts, val);
            compile_expr(compiler, contexts, val)?;
            let ctx = contexts.last_mut().unwrap();
            let idx = ctx.add_local(name.clone());
            ctx.set_local_type(name.clone(), ty);
            ctx.code.push(Opcode::StoreLocal as u8);
            ctx.code.push(idx);
        }
        Stmt::Return(val) => {
            compile_expr(compiler, contexts, val)?;
            let ctx = contexts.last_mut().unwrap();
            ctx.code.push(Opcode::Return as u8);
        }
        Stmt::Yield(val) => {
            compile_expr(compiler, contexts, val)?;
            let ctx = contexts.last_mut().unwrap();
            let didx = compiler.add_string("yield");
            ctx.code.push(Opcode::FFICall as u8);
            ctx.code.extend_from_slice(&didx.to_le_bytes());
            ctx.code.push(1u8);
            ctx.code.push(Opcode::Pop as u8);
        }
        Stmt::FuncDef(name, args, body) => {
            let chunk_idx = compile_func_to_chunk(compiler, args.clone(), body)?;
            let key = compiler.def_name(&name);
            println!("DEBUG: Registered function: {} -> chunk {}", key, chunk_idx);
            compiler.functions.insert(key, chunk_idx);
        }
        Stmt::ClassDef(name, fields) => {
            let idx = compiler.classes.len() as u16;
            compiler.classes.push(ClassInfo {
                name: compiler.def_name(&name),
                fields: fields.clone(),
            });
            let key = compiler.def_name(&name);
            compiler.class_map.insert(key, idx);
        }
        Stmt::TraitDef(name, methods) => {
            let idx = compiler.traits.len() as u16;
            compiler.traits.push(TraitInfo {
                name: compiler.def_name(&name),
                methods: methods.clone(),
            });
            let key = compiler.def_name(&name);
            compiler.trait_map.insert(key, idx);
        }
        Stmt::EnumDef(name, variants) => {
            println!(
                "DEBUG: compiling EnumDef {} with variants {:?}",
                name,
                variants.iter().map(|(v, _)| v).collect::<Vec<_>>()
            );
            // 1. Define Class for the Enum
            // Collect all possible field names (max count) to define the class structure
            // We use positional fields _0, _1, etc.
            let mut max_fields = 0;
            for (_, v_fields) in variants {
                if v_fields.len() > max_fields {
                    max_fields = v_fields.len();
                }
            }

            let mut class_fields = vec!["__variant__".to_string()];
            for i in 0..max_fields {
                class_fields.push(format!("_{}", i));
            }

            let class_idx = compiler.classes.len() as u16;
            compiler.classes.push(ClassInfo {
                name: compiler.def_name(&name),
                fields: class_fields,
            });
            let class_key = compiler.def_name(&name);
            compiler.class_map.insert(class_key.clone(), class_idx);

            // 2. Define Constructor Functions for each variant
            for (v_name, v_fields) in variants {
                let mut ctx = FunctionContext::new(v_fields.clone());

                // Debug print
                ctx.code.push(Opcode::Push as u8);
                let msg_idx = compiler.add_string(&format!("DEBUG: Constructing {}", v_name));
                ctx.code.extend_from_slice(&msg_idx.to_le_bytes());
                let print_idx = compiler.add_string("print");
                ctx.code.push(Opcode::FFICall as u8);
                ctx.code.extend_from_slice(&print_idx.to_le_bytes());
                ctx.code.push(1u8);
                ctx.code.push(Opcode::Pop as u8);

                // Create new object
                ctx.code.push(Opcode::NewObject as u8);
                ctx.code.extend_from_slice(&class_idx.to_le_bytes());

                // Set __variant__
                ctx.code.push(Opcode::Dup as u8);
                ctx.code.push(0u8);
                ctx.code.push(Opcode::Push as u8);
                let v_name_idx = compiler.add_string(v_name);
                ctx.code.extend_from_slice(&v_name_idx.to_le_bytes());

                let variant_field_idx = compiler.add_string("__variant__");
                ctx.code.push(Opcode::SetField as u8);
                ctx.code.extend_from_slice(&variant_field_idx.to_le_bytes());
                ctx.code.push(Opcode::Pop as u8);

                // Set fields
                for (i, _) in v_fields.iter().enumerate() {
                    ctx.code.push(Opcode::Dup as u8);
                    ctx.code.push(0u8);

                    // Load argument
                    ctx.code.push(Opcode::LoadLocal as u8);
                    // Arguments are locals 0..n
                    ctx.code.push(i as u8);

                    // Set field _i
                    let field_name = format!("_{}", i);
                    let f_idx = compiler.add_string(&field_name);
                    ctx.code.push(Opcode::SetField as u8);
                    ctx.code.extend_from_slice(&f_idx.to_le_bytes());
                    ctx.code.push(Opcode::Pop as u8);
                }

                // Return object
                ctx.code.push(Opcode::Return as u8);

                // Finalize chunk
                let chunk = Chunk {
                    locals: ctx.locals.len() as u16,
                    upvalues: 0,
                    max_stack: 16,
                    code: ctx.code,
                    handlers: vec![],
                    lines: ctx.lines,
                };

                let chunk_idx = (compiler.chunks.len() + 1) as u16;
                compiler.chunks.push(chunk);
                let v_key = format!("{}::{}", class_key, v_name);
                println!(
                    "DEBUG: Registered function: {} -> chunk {}",
                    v_key, chunk_idx
                );
                // println!("DEBUG: Chunk {} code: {:?}", chunk_idx, chunk.code); // Cannot use chunk here as it moved
                // Use compiler.chunks[chunk_idx]
                println!(
                    "DEBUG: Chunk {} code: {:?}",
                    chunk_idx,
                    compiler.chunks.last().unwrap().code
                );
                compiler.functions.insert(v_key, chunk_idx);
            }
        }
        Stmt::ImplDef(trait_name, class_name, methods) => {
            let trait_idx = compiler
                .resolve_trait(trait_name)
                .ok_or_else(|| Error::Compile(format!("undefined trait: {}", trait_name)))?;
            let class_idx = compiler
                .resolve_class(class_name)
                .ok_or_else(|| Error::Compile(format!("undefined class: {}", class_name)))?;

            // Compile all methods in the impl block
            let mut method_map = HashMap::new();
            for method_stmt in methods {
                if let Stmt::FuncDef(name, args, body) = method_stmt {
                    let chunk_idx = compile_func_to_chunk(compiler, args.clone(), &body)?;
                    method_map.insert(name.clone(), chunk_idx);
                } else {
                    return Err(Error::Compile(
                        "impl block can only contain function definitions".into(),
                    ));
                }
            }

            // Construct the method table for the implementation based on the trait definition
            let trait_info = &compiler.traits[trait_idx as usize];
            let mut impl_methods = Vec::new();
            for method_name in &trait_info.methods {
                if let Some(&chunk_idx) = method_map.get(method_name) {
                    impl_methods.push(chunk_idx);
                } else {
                    return Err(Error::Compile(format!(
                        "missing implementation for method '{}' of trait '{}'",
                        method_name, trait_name
                    )));
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

    // Auto-build method tables for class-qualified micro functions: ClassPath::method
    {
        use std::collections::BTreeMap;
        let mut grouped: BTreeMap<u16, Vec<(String, u16)>> = BTreeMap::new();
        for (fname, &chunk_idx) in &compiler.functions {
            if let Some(pos) = fname.rfind("::") {
                let class_name = &fname[..pos];
                let method_name = &fname[pos + 2..];
                let mut found = compiler.class_map.get(class_name).copied();
                if found.is_none() {
                    for (k, &v) in &compiler.class_map {
                        if k.ends_with(&format!("::{}", class_name)) || k == class_name {
                            found = Some(v);
                            break;
                        }
                    }
                }
                if let Some(class_idx) = found {
                    grouped
                        .entry(class_idx)
                        .or_default()
                        .push((method_name.to_string(), chunk_idx));
                }
            }
        }
        for (class_idx, methods) in grouped {
            let trait_name = format!("{}::__methods__", compiler.classes[class_idx as usize].name);
            let trait_info = TraitInfo {
                name: trait_name,
                methods: methods.iter().map(|(n, _)| n.clone()).collect(),
            };
            let trait_idx = compiler.traits.len() as u16;
            compiler.traits.push(trait_info);
            let impl_info = ImplInfo {
                class_idx,
                trait_idx,
                methods: methods.iter().map(|(_, c)| *c).collect(),
            };
            compiler.impls.push(impl_info);
        }
    }

    let mut main_ctx = contexts.pop().unwrap();
    println!(
        "DEBUG: main_ctx code len before Halt: {}",
        main_ctx.code.len()
    );
    main_ctx.code.push(Opcode::Halt as u8);

    let main_chunk = Chunk {
        locals: main_ctx.locals.len() as u16,
        upvalues: 0,
        max_stack: 16,
        code: main_ctx.code,
        handlers: vec![],
        lines: main_ctx.lines,
    };

    let mut all_chunks = vec![main_chunk];
    all_chunks.extend(compiler.chunks);

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

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
