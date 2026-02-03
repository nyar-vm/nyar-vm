//! Python 字节码生成器
//!
//! 将 Nyar IR (IKunTree) 转换为 Python 字节码 (.pyc)

use chomsky_uir::IKunTree;
use python_assembler::instructions::PythonInstruction;
use python_assembler::program::{PythonCodeObject, PythonObject, PythonProgram, PycHeader, PythonVersion};
use python_assembler::formats::pyc::writer::PycWriter;
use python_assembler::formats::pyc::PycWriteConfig;
use gaia_types::GaiaError;
use chomsky_extract::{Backend, BackendArtifact};
use chomsky_types::ChomskyResult;

/// 将 IKunTree 转换为 Python 程序的翻译器
pub struct PycTranslator {
    filename: String,
    name: String,
    instructions: Vec<PythonInstruction>,
    consts: Vec<PythonObject>,
    names: Vec<String>,
}

impl Backend for PycTranslator {
    fn name(&self) -> &str {
        "pyc"
    }

    fn generate(&self, tree: &IKunTree) -> ChomskyResult<BackendArtifact> {
        let mut translator = PycTranslator::new(&self.filename, &self.name);
        let program = translator.translate_from_tree(tree);
        let mut buffer = Vec::new();
        let config = PycWriteConfig::default();
        let mut writer = PycWriter::new(&mut buffer, config);
        writer.write(&program).map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("PycWriter error: {:?}", e))
        })?;
        Ok(BackendArtifact::Binary(buffer))
    }
}

impl PycTranslator {
    pub fn new(filename: &str, name: &str) -> Self {
        Self {
            filename: filename.to_string(),
            name: name.to_string(),
            instructions: Vec::new(),
            consts: vec![PythonObject::None], // None is always at index 0
            names: Vec::new(),
        }
    }

    pub fn translate_from_tree(&mut self, tree: &IKunTree) -> PythonProgram {
        // Python 3.11+ 需要在每个代码对象开始处有 RESUME 指令
        self.instructions.push(PythonInstruction::RESUME);

        self.compile_node(tree);

        // 默认返回 None
        self.instructions.push(PythonInstruction::RETURN_CONST(0));

        let version = PythonVersion::Python3_12;
        let header = PycHeader {
            magic: version.as_magic(),
            flags: 0,
            timestamp: 0,
            size: 0,
        };

        PythonProgram {
            header,
            version,
            code_object: PythonCodeObject {
                name: self.name.clone(),
                qualname: self.name.clone(),
                source_name: self.filename.clone(),
                first_line: 1,
                last_line: 1,
                co_argcount: 0,
                co_posonlyargcount: 0,
                co_kwonlyargcount: 0,
                co_nlocals: 0,
                co_stacksize: 10,
                co_flags: 0x40, // CO_NOFREE
                co_code: std::mem::take(&mut self.instructions),
                co_consts: std::mem::take(&mut self.consts),
                co_names: std::mem::take(&mut self.names),
                co_localsplusnames: vec![],
                co_localspluskinds: vec![],
                co_linetable: vec![0, 1],
                co_exceptiontable: vec![],
            },
        }
    }

    fn compile_node(&mut self, tree: &IKunTree) {
        match tree {
            IKunTree::Constant(v) => {
                let idx = self.add_const(PythonObject::Integer(*v as i64));
                self.instructions.push(PythonInstruction::LOAD_CONST(idx));
            }
            IKunTree::FloatConstant(bits) => {
                let f = f64::from_bits(*bits);
                let idx = self.add_const(PythonObject::Float(f));
                self.instructions.push(PythonInstruction::LOAD_CONST(idx));
            }
            IKunTree::BooleanConstant(b) => {
                let idx = self.add_const(PythonObject::Bool(*b));
                self.instructions.push(PythonInstruction::LOAD_CONST(idx));
            }
            IKunTree::StringConstant(s) => {
                let idx = self.add_const(PythonObject::Str(s.clone()));
                self.instructions.push(PythonInstruction::LOAD_CONST(idx));
            }
            IKunTree::Symbol(name) => {
                let idx = self.add_name(name);
                // Python 3.11+ LOAD_GLOBAL arg is (index << 1) | push_null
                // We don't push null here, Apply will handle it if it's a call
                self.instructions.push(PythonInstruction::LOAD_GLOBAL(idx << 1));
            }
            IKunTree::StateUpdate(target, value) => {
                self.compile_node(value);
                if let IKunTree::Symbol(name) = &**target {
                    let idx = self.add_name(name);
                    self.instructions.push(PythonInstruction::STORE_NAME(idx));
                }
            }
            IKunTree::Extension(op, args) => {
                for arg in args {
                    self.compile_node(arg);
                }
                match op.as_str() {
                    "add" => self.instructions.push(PythonInstruction::BINARY_OP(0)), // NB_ADD
                    "sub" => self.instructions.push(PythonInstruction::BINARY_OP(10)), // NB_SUBTRACT
                    "mul" => self.instructions.push(PythonInstruction::BINARY_OP(5)), // NB_MULTIPLY
                    "div" => self.instructions.push(PythonInstruction::BINARY_OP(11)), // NB_TRUE_DIVIDE
                    _ => {}
                }
            }
            IKunTree::Seq(items) => {
                for item in items {
                    self.compile_node(item);
                }
            }
            IKunTree::Apply(func, args) => {
                // 特殊处理 return
                if let IKunTree::Symbol(name) = &**func {
                    if name == "return" {
                        if let Some(val) = args.first() {
                            self.compile_node(val);
                            self.instructions.push(PythonInstruction::RETURN_VALUE);
                        } else {
                            self.instructions.push(PythonInstruction::RETURN_CONST(0));
                        }
                        return;
                    }
                }

                // 常规函数调用
                // Python 3.11+ CALL expects [NULL, func, arg1, ... argN]
                self.instructions.push(PythonInstruction::PUSH_NULL);
                self.compile_node(func);
                for arg in args {
                    self.compile_node(arg);
                }
                self.instructions.push(PythonInstruction::CALL(args.len() as u32));
                self.instructions.push(PythonInstruction::POP_TOP);
            }
            IKunTree::CrossLangCall(lang, name, args) => {
                if lang == "nyar" && name == "std::io::println" {
                    // Map back to Python's print
                    self.instructions.push(PythonInstruction::PUSH_NULL);
                    let idx = self.add_name("print");
                    self.instructions.push(PythonInstruction::LOAD_GLOBAL(idx << 1));
                    for arg in args {
                        self.compile_node(arg);
                    }
                    self.instructions.push(PythonInstruction::CALL(args.len() as u32));
                    self.instructions.push(PythonInstruction::POP_TOP);
                }
            }
            _ => {}
        }
    }

    fn add_const(&mut self, obj: PythonObject) -> u32 {
        if let Some(idx) = self.consts.iter().position(|c| c == &obj) {
            idx as u32
        } else {
            self.consts.push(obj);
            (self.consts.len() - 1) as u32
        }
    }

    fn add_name(&mut self, name: &str) -> u32 {
        if let Some(idx) = self.names.iter().position(|n| n == name) {
            idx as u32
        } else {
            self.names.push(name.to_string());
            (self.names.len() - 1) as u32
        }
    }
}

/// 将 PythonProgram 序列化为字节流
pub fn emit_pyc(program: &PythonProgram) -> Result<Vec<u8>, GaiaError> {
    let mut buffer = Vec::new();
    let config = PycWriteConfig::default();
    let mut writer = PycWriter::new(&mut buffer, config);
    writer.write(program)?;
    Ok(buffer)
}
