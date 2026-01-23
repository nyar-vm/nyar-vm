//! Python 字节码生成器
//!
//! 将 Python AST 转换为 Python 字节码 (.pyc)

use oak_python::ast::*;

/// Python 3.10 常用操作码
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCode {
    PopTop = 1,
    PushNull = 2,
    BinaryOp = 122,
    ReturnValue = 83,
    ReturnConst = 121,
    StoreName = 90,
    UnpackSequence = 92,
    StoreAttr = 95,
    StoreGlobal = 97,
    LoadConst = 100,
    LoadName = 101,
    BuildTuple = 102,
    BuildList = 103,
    BuildSet = 104,
    BuildMap = 105,
    LoadAttr = 106,
    CompareOp = 107,
    ImportName = 108,
    ImportFrom = 109,
    JumpForward = 110,
    PopJumpIfFalse = 114,
    PopJumpIfTrue = 115,
    LoadGlobal = 116,
    LoadFast = 124,
    StoreFast = 125,
    RaiseVarargs = 130,
    MakeFunction = 132,
    BuildSlice = 133,
    Resume = 151,
    LoadMethod = 160,
    Call = 171,
}

/// Python 指令
#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: OpCode,
    pub arg: u32,
}

/// Python 代码对象 (PyCodeObject)
#[derive(Debug, Clone, PartialEq)]
pub struct PyCodeObject {
    pub argcount: u32,
    pub posonlyargcount: u32,
    pub kwonlyargcount: u32,
    pub nlocals: u32,
    pub stacksize: u32,
    pub flags: u32,
    pub code: Vec<u8>,
    pub consts: Vec<PyObject>,
    pub names: Vec<String>,
    pub localsplusnames: Vec<String>,
    pub filename: String,
    pub name: String,
    pub qualname: String,
    pub firstlineno: u32,
    pub linetable: Vec<u8>,
    pub exceptiontable: Vec<u8>,
}

/// Marshal 序列化器
pub struct Marshal {
    data: Vec<u8>,
}

impl Marshal {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn write_object(&mut self, obj: &PyObject) {
        match obj {
            PyObject::None => {
                self.data.push(b'N');
            }
            PyObject::Bool(b) => {
                self.data.push(if *b { b'T' } else { b'F' });
            }
            PyObject::Int(i) => {
                if *i >= -2147483648 && *i <= 2147483647 {
                    self.data.push(b'i' | 0x80); // TYPE_INT | FLAG_REF
                    self.data.extend_from_slice(&(*i as i32).to_le_bytes());
                }
                else {
                    self.data.push(b'I' | 0x80); // TYPE_INT64 | FLAG_REF
                    self.data.extend_from_slice(&(*i as i64).to_le_bytes());
                }
            }
            PyObject::Float(f) => {
                self.data.push(b'g' | 0x80); // TYPE_BINARY_FLOAT | FLAG_REF
                self.data.extend_from_slice(&f.to_le_bytes());
            }
            PyObject::String(s) => {
                if s.len() <= 255 && s.is_ascii() {
                    self.data.push(b'Z' | 0x80); // TYPE_SHORT_ASCII_INTERNED | FLAG_REF
                    self.data.push(s.len() as u8);
                    self.data.extend_from_slice(s.as_bytes());
                }
                else {
                    self.data.push(b'u' | 0x80); // TYPE_UNICODE | FLAG_REF
                    self.data.extend_from_slice(&(s.len() as u32).to_le_bytes());
                    self.data.extend_from_slice(s.as_bytes());
                }
            }
            PyObject::Bytes(b) => {
                self.data.push(b's' | 0x80); // TYPE_STRING | FLAG_REF
                self.data.extend_from_slice(&(b.len() as u32).to_le_bytes());
                self.data.extend_from_slice(b);
            }
            PyObject::Tuple(t) => {
                if t.len() <= 255 {
                    self.data.push(b')' | 0x80); // TYPE_SMALL_TUPLE | FLAG_REF
                    self.data.push(t.len() as u8);
                }
                else {
                    self.data.push(b'(' | 0x80); // TYPE_TUPLE | FLAG_REF
                    self.data.extend_from_slice(&(t.len() as u32).to_le_bytes());
                }
                for item in t {
                    self.write_object(item);
                }
            }
            PyObject::Code(code) => {
                self.write_code_object(code);
            }
        }
    }

    pub fn write_code_object(&mut self, code: &PyCodeObject) {
        self.data.push(b'c' | 0x80); // TYPE_CODE | FLAG_REF
        self.data.extend_from_slice(&(code.argcount as i32).to_le_bytes());
        self.data.extend_from_slice(&(code.posonlyargcount as i32).to_le_bytes());
        self.data.extend_from_slice(&(code.kwonlyargcount as i32).to_le_bytes());
        // nlocals removed in 3.11+ marshal
        self.data.extend_from_slice(&(code.stacksize as i32).to_le_bytes());
        self.data.extend_from_slice(&(code.flags as i32).to_le_bytes());

        // co_code
        self.write_object(&PyObject::Bytes(code.code.clone()));

        // co_consts
        self.write_object(&PyObject::Tuple(code.consts.clone()));

        // co_names
        self.write_object(&PyObject::Tuple(code.names.iter().map(|s| PyObject::String(s.clone())).collect()));

        // co_localsplusnames (Python 3.11+)
        self.write_object(&PyObject::Tuple(code.localsplusnames.iter().map(|s| PyObject::String(s.clone())).collect()));

        // co_localspluskinds (Python 3.11+)
        // For now, assume all are CO_FAST_LOCAL (0x20) if they are in localsplusnames
        let kinds: Vec<u8> = vec![0x20; code.localsplusnames.len()];
        self.write_object(&PyObject::Bytes(kinds));

        // co_filename
        self.write_object(&PyObject::String(code.filename.clone()));

        // co_name
        self.write_object(&PyObject::String(code.name.clone()));

        // co_qualname
        self.write_object(&PyObject::String(code.qualname.clone()));

        // co_firstlineno (w_long)
        self.data.extend_from_slice(&(code.firstlineno as i32).to_le_bytes());

        // co_linetable
        self.write_object(&PyObject::Bytes(code.linetable.clone()));

        // co_exceptiontable
        self.write_object(&PyObject::Bytes(code.exceptiontable.clone()));
    }

    pub fn finish(self) -> Vec<u8> {
        self.data
    }
}

/// Python 对象 (用于常量池)
#[derive(Debug, Clone, PartialEq)]
pub enum PyObject {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Tuple(Vec<PyObject>),
    Code(Box<PyCodeObject>),
}

/// Python 字节码翻译器
pub struct PycTranslator {
    consts: Vec<PyObject>,
    names: Vec<String>,
    localsplusnames: Vec<String>,
    instructions: Vec<Instruction>,
    filename: String,
    name: String,
}

impl PycTranslator {
    pub fn new(filename: &str, name: &str) -> Self {
        Self {
            consts: vec![PyObject::None], // index 0 is always None
            names: Vec::new(),
            localsplusnames: Vec::new(),
            instructions: Vec::new(),
            filename: filename.to_string(),
            name: name.to_string(),
        }
    }

    pub fn translate(&mut self, program: &Program) -> PyCodeObject {
        // Python 3.11+ requires RESUME at the start of every code object
        self.emit(OpCode::Resume, 0);

        for stmt in &program.statements {
            self.compile_statement(stmt);
        }

        // 默认返回 None (Python 3.12 使用 RETURN_CONST)
        let none_idx = self.add_const(PyObject::None);
        self.emit(OpCode::ReturnConst, none_idx);

        self.assemble()
    }

    fn compile_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment { target, value } => {
                self.compile_expression(value);
                if let Expression::Name(name) = target {
                    let idx = self.add_name(name);
                    self.emit(OpCode::StoreName, idx);
                }
            }
            Statement::Expression(expr) => {
                self.compile_expression(expr);
                self.emit(OpCode::PopTop, 0);
            }
            Statement::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expression(e);
                }
                else {
                    self.emit(OpCode::LoadConst, 0);
                }
                self.emit(OpCode::ReturnValue, 0);
            }
            Statement::FunctionDef { name, parameters, body, .. } => {
                // 处理函数定义
                let mut sub_translator = PycTranslator::new(&self.filename, name);
                sub_translator.emit(OpCode::Resume, 0);
                for param in parameters {
                    sub_translator.add_localsplusname(&param.name);
                }
                for s in body {
                    sub_translator.compile_statement(s);
                }
                // 确保有返回语句
                sub_translator.emit(OpCode::LoadConst, 0);
                sub_translator.emit(OpCode::ReturnValue, 0);

                let code_obj = sub_translator.assemble();
                let code_idx = self.add_const(PyObject::Code(Box::new(code_obj)));
                let name_idx = self.add_const(PyObject::String(name.clone()));

                self.emit(OpCode::LoadConst, code_idx);
                self.emit(OpCode::LoadConst, name_idx);
                self.emit(OpCode::MakeFunction, 0);
                let name_idx = self.add_name(name);
                self.emit(OpCode::StoreName, name_idx);
            }
            _ => {
                // 暂时忽略其他语句
            }
        }
    }

    fn compile_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::Literal(lit) => {
                let obj = match lit {
                    Literal::Integer(i) => PyObject::Int(*i),
                    Literal::Float(f) => PyObject::Float(*f),
                    Literal::String(s) => PyObject::String(s.clone()),
                    Literal::Boolean(b) => PyObject::Bool(*b),
                    Literal::None => PyObject::None,
                };
                let idx = self.add_const(obj);
                self.emit(OpCode::LoadConst, idx);
            }
            Expression::Name(name) => {
                let idx = self.add_name(name);
                self.emit(OpCode::LoadName, idx);
            }
            Expression::BinaryOp { left, operator, right } => {
                self.compile_expression(left);
                self.compile_expression(right);
                match operator {
                    BinaryOperator::Add => self.emit(OpCode::BinaryOp, 0),
                    BinaryOperator::Sub => self.emit(OpCode::BinaryOp, 10), // NB_SUBTRACT is 10 in 3.12? No, check
                    BinaryOperator::Mult => self.emit(OpCode::BinaryOp, 5), // NB_MULTIPLY is 5
                    BinaryOperator::Div => self.emit(OpCode::BinaryOp, 11), // NB_TRUE_DIVIDE is 11
                    _ => {}
                }
            }
            Expression::Call { func, args, .. } => {
                // Python 3.11+ CALL expects: [NULL, func, arg1, arg2, ...]
                self.emit(OpCode::PushNull, 0);
                self.compile_expression(func);
                for arg in args {
                    self.compile_expression(arg);
                }
                self.emit(OpCode::Call, args.len() as u32);
            }
            _ => {}
        }
    }

    fn emit(&mut self, opcode: OpCode, arg: u32) {
        self.instructions.push(Instruction { opcode, arg });
    }

    fn add_const(&mut self, obj: PyObject) -> u32 {
        if let Some(pos) = self.consts.iter().position(|x| x == &obj) {
            pos as u32
        }
        else {
            let pos = self.consts.len();
            self.consts.push(obj);
            pos as u32
        }
    }

    fn add_name(&mut self, name: &str) -> u32 {
        if let Some(pos) = self.names.iter().position(|x| x == name) {
            pos as u32
        }
        else {
            let pos = self.names.len();
            self.names.push(name.to_string());
            pos as u32
        }
    }

    fn add_localsplusname(&mut self, name: &str) -> u32 {
        if let Some(pos) = self.localsplusnames.iter().position(|x| x == name) {
            pos as u32
        }
        else {
            let pos = self.localsplusnames.len();
            self.localsplusnames.push(name.to_string());
            pos as u32
        }
    }

    fn assemble(&self) -> PyCodeObject {
        let mut code = Vec::new();
        for inst in &self.instructions {
            code.push(inst.opcode as u8);
            code.push(inst.arg as u8);

            // Add CACHE entries for Python 3.12
            let cache_entries = match inst.opcode {
                OpCode::BinaryOp => 1,
                OpCode::Call => 3,
                OpCode::LoadGlobal => 4,
                OpCode::LoadAttr => 9,
                OpCode::StoreAttr => 4,
                OpCode::CompareOp => 1,
                _ => 0,
            };

            for _ in 0..cache_entries {
                code.push(0);
                code.push(0);
            }
        }

        PyCodeObject {
            argcount: 0,
            posonlyargcount: 0,
            kwonlyargcount: 0,
            nlocals: 0,
            stacksize: 3, // print + 1 + NULL
            flags: 0,
            code,
            consts: self.consts.clone(),
            names: self.names.clone(),
            localsplusnames: Vec::new(),
            filename: self.filename.clone(),
            name: self.name.clone(),
            qualname: self.name.clone(),
            firstlineno: 1,
            linetable: vec![0xf0, 0x03, 0x01, 0x01, 0x01, 0xd8, 0x04, 0x05, 0x81, 0x01],
            exceptiontable: Vec::new(),
        }
    }
}
