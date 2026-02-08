use chomsky::optimizer::UniversalOptimizer;
use chomsky_cost::DefaultCostModel;
use chomsky_extract::IKunExtractor;
use chomsky_uir::{EGraph, IKun, IKunTree, Id};
use nyar_vm::bytecode::instruction::Instruction;
use nyar_vm::bytecode::format::{Chunk, Constant, ExportInfo, NyarcModule};
use nyar_vm::vm::NyarVM;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum RuntimeError {
    NyarVm(String),
    EntryPointNotFound,
    Other(String),
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::NyarVm(msg) => write!(f, "Nyar VM execution failed: {}", msg),
            RuntimeError::EntryPointNotFound => write!(f, "No entry point found in module"),
            RuntimeError::Other(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl Error for RuntimeError {}

impl From<String> for RuntimeError {
    fn from(s: String) -> Self {
        RuntimeError::Other(s)
    }
}

impl From<&str> for RuntimeError {
    fn from(s: &str) -> Self {
        RuntimeError::Other(s.to_string())
    }
}

pub struct RustyGoRuntime {
    _optimizer: UniversalOptimizer<()>,
    vm: NyarVM,
}

impl RustyGoRuntime {
    pub fn new() -> Self {
        Self {
            _optimizer: UniversalOptimizer::new(),
            vm: NyarVM::new(),
        }
    }

    pub fn execute(&mut self, intent_graph: (EGraph<IKun, ()>, Id)) -> Result<(), RuntimeError> {
        let (egraph, root_id) = intent_graph;

        // 1. Extract the best tree using the default cost model
        let cost_model = DefaultCostModel::default();
        let extractor = IKunExtractor::new(&egraph, cost_model);
        let tree = extractor.extract(root_id);

        // 2. Translate IKunTree to Nyar Module
        let module = self.translate_to_nyar(&tree)?;

        // 3. Execute using Nyar VM
        println!("Executing Nyar Module: {:?}", module.exports);

        let module_idx = self.vm.load_module(module);

        // Find main or first export
        if let Some(export) = self.vm.modules[module_idx]
            .exports
            .iter()
            .find(|e| e.symbol == "main".into())
            .or(self.vm.modules[module_idx].exports.first())
        {
            match self.vm.execute(module_idx, export.chunk_idx as usize) {
                Ok(val) => {
                    println!("Execution result: {}", val);
                }
                Err(e) => {
                    println!("Nyar VM execution failed: {:?}", e);
                    return Err(RuntimeError::NyarVm(format!("{:?}", e)));
                }
            }
        } else {
            return Err(RuntimeError::EntryPointNotFound);
        }

        Ok(())
    }

    fn translate_to_nyar(&self, tree: &IKunTree) -> Result<NyarcModule, RuntimeError> {
        let mut module = NyarcModule::default();

        match tree {
            IKunTree::Module(_name, items) => {
                for item in items {
                    if let IKunTree::Export(name, body) = item {
                        if let IKunTree::Lambda(params, body) = &**body {
                            let chunk = self.translate_function(params, body, &mut module)?;
                            let chunk_idx = module.chunks.len() as u16;
                            module.chunks.push(chunk);
                            module.exports.push(ExportInfo {
                                symbol: name.clone().into(),
                                chunk_idx: chunk_idx as u16,
                            });
                        }
                    }
                }
            }
            _ => return Err(RuntimeError::Other("Root must be a module".to_string())),
        }

        Ok(module)
    }

    fn translate_function(
        &self,
        _params: &[String],
        body: &IKunTree,
        module: &mut NyarcModule,
    ) -> Result<Chunk, RuntimeError> {
        let mut code = Vec::new();
        self.emit_tree(body, &mut code, module)?;

        // Add a Return at the end if not already there
        if code.last() != Some(&(nyar_vm::bytecode::opcode::Opcode::Return as u8)) {
            code.extend_from_slice(&Instruction::Return.encode());
        }

        Ok(Chunk {
            locals: 32,
            upvalues: 0,
            max_stack: 32,
            code,
            handlers: vec![],
            lines: vec![],
            decoded: std::sync::OnceLock::new(),
            hotness: std::sync::atomic::AtomicU32::new(0),
        })
    }

    fn emit_tree(
        &self,
        tree: &IKunTree,
        code: &mut Vec<u8>,
        module: &mut NyarcModule,
    ) -> Result<(), RuntimeError> {
        match tree {
            IKunTree::Seq(items) => {
                for item in items {
                    self.emit_tree(item, code, module)?;
                }
            }
            IKunTree::Constant(v) => {
                code.extend_from_slice(&Instruction::I64Const(*v).encode());
            }
            IKunTree::StringConstant(s) => {
                code.extend_from_slice(&Instruction::StringConst(s.clone()).encode());
            }
            IKunTree::Symbol(name) => {
                // FIXME: 简单起见，假设所有变量都是本地变量，且需要一个名字到索引的映射
                // 目前 Nyar VM 支持 LoadGlobal/LoadLocal，这里先用符号名
                let name_idx = module.constants.len() as u16;
                module.constants.push(Constant::String(name.clone()));
                code.extend_from_slice(&Instruction::LoadGlobal(name_idx).encode());
            }
            IKunTree::StateUpdate(target, value) => {
                self.emit_tree(value, code, module)?;
                if let IKunTree::Symbol(name) = &**target {
                    let name_idx = module.constants.len() as u16;
                    module.constants.push(Constant::String(name.clone()));
                    code.extend_from_slice(&Instruction::StoreGlobal(name_idx).encode());
                } else {
                    return Err(RuntimeError::Other("Assignment target must be a symbol".to_string()));
                }
            }
            IKunTree::Choice(condition, then_body, else_body) => {
                self.emit_tree(condition, code, module)?;
                
                // Placeholder for JumpIfFalse offset
                let jump_if_false_pos = code.len();
                code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                
                self.emit_tree(then_body, code, module)?;
                
                // Placeholder for Jump offset (to skip else)
                let jump_pos = code.len();
                code.extend_from_slice(&Instruction::Jump(0).encode());
                
                // Patch JumpIfFalse
                let else_start = code.len();
                let diff_to_else = (else_start - jump_if_false_pos) as i16;
                let patched_jump_if_false = Instruction::JumpIfFalse(diff_to_else).encode();
                for (i, byte) in patched_jump_if_false.iter().enumerate() {
                    code[jump_if_false_pos + i] = *byte;
                }
                
                self.emit_tree(else_body, code, module)?;
                
                // Patch Jump
                let end_pos = code.len();
                let diff_to_end = (end_pos - jump_pos) as i16;
                let patched_jump = Instruction::Jump(diff_to_end).encode();
                for (i, byte) in patched_jump.iter().enumerate() {
                    code[jump_pos + i] = *byte;
                }
            }
            IKunTree::CrossLangCall { language, module_path, function_name, arguments } => {
                if language == "native" || language == "nyar" {
                    // Push arguments
                    for arg in arguments {
                        self.emit_tree(arg, code, module)?;
                    }
                    // FFICall expects (constant_idx_of_name, argc)
                    let name_idx = module.constants.len() as u16;
                    let full_name = format!("{}::{}", module_path, function_name);
                    module.constants.push(Constant::String(full_name));
                    code.extend_from_slice(
                        &Instruction::FFICall(name_idx, arguments.len() as u8).encode(),
                    );
                }
            }
            IKunTree::Extension(name, args) => {
                match name.as_str() {
                    "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        if args.len() == 2 {
                            self.emit_tree(&args[0], code, module)?;
                            self.emit_tree(&args[1], code, module)?;
                            match name.as_str() {
                                "+" => code.extend_from_slice(&Instruction::I64Add.encode()),
                                "-" => code.extend_from_slice(&Instruction::I64Sub.encode()),
                                "*" => code.extend_from_slice(&Instruction::I64Mul.encode()),
                                "/" => code.extend_from_slice(&Instruction::I64DivS.encode()),
                                "==" => code.extend_from_slice(&Instruction::I64Eq.encode()),
                                "!=" => code.extend_from_slice(&Instruction::I64Ne.encode()),
                                "<" => code.extend_from_slice(&Instruction::I64LtS.encode()),
                                "<=" => code.extend_from_slice(&Instruction::I64LeS.encode()),
                                ">" => code.extend_from_slice(&Instruction::I64GtS.encode()),
                                ">=" => code.extend_from_slice(&Instruction::I64GeS.encode()),
                                _ => unreachable!(),
                            }
                        } else {
                            return Err(RuntimeError::Other(format!("Binary op {} requires 2 arguments", name)));
                        }
                    }
                    "return" => {
                        if let Some(val) = args.first() {
                            self.emit_tree(val, code, module)?;
                        }
                        code.extend_from_slice(&Instruction::Return.encode());
                    }
                    "for" => {
                        // args: [init, cond, post, body]
                        if args.len() == 4 {
                            self.emit_tree(&args[0], code, module)?; // init
                            let loop_start = code.len();
                            self.emit_tree(&args[1], code, module)?; // cond
                            
                            let exit_jump_pos = code.len();
                            code.extend_from_slice(&Instruction::JumpIfFalse(0).encode());
                            
                            self.emit_tree(&args[3], code, module)?; // body
                            self.emit_tree(&args[2], code, module)?; // post
                            
                            let back_jump_diff = (loop_start as isize - code.len() as isize) as i16;
                            code.extend_from_slice(&Instruction::Jump(back_jump_diff).encode());
                            
                            // Patch exit jump
                            let exit_pos = code.len();
                            let exit_diff = (exit_pos - exit_jump_pos) as i16;
                            let patched_exit = Instruction::JumpIfFalse(exit_diff).encode();
                            for (i, byte) in patched_exit.iter().enumerate() {
                                code[exit_jump_pos + i] = *byte;
                            }
                        }
                    }
                    "string" => {
                        // TODO: 完整的字符串字面量支持
                        code.extend_from_slice(&Instruction::StringConst("".to_string()).encode());
                    }
                    _ => {}
                }
            }
            _ => {
                // Ignore other types for now
            }
        }
        Ok(())
    }
}
