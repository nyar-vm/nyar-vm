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

pub struct MiniGoRuntime {
    _optimizer: UniversalOptimizer<()>,
    vm: NyarVM,
}

impl MiniGoRuntime {
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
            .find(|e| e.symbol == "main")
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
                                symbol: name.clone(),
                                chunk_idx,
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
            decoded: None,
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
            IKunTree::CrossLangCall(lang, name, args) => {
                if lang == "native" {
                    // Push arguments
                    for arg in args {
                        self.emit_tree(arg, code, module)?;
                    }
                    // FFICall expects (constant_idx_of_name, argc)
                    let name_idx = module.constants.len() as u16;
                    module.constants.push(Constant::String(name.clone()));
                    code.extend_from_slice(
                        &Instruction::FFICall(name_idx, args.len() as u8).encode(),
                    );
                }
            }
            IKunTree::Extension(name, args) => {
                if name == "return" {
                    if let Some(val) = args.first() {
                        self.emit_tree(val, code, module)?;
                    }
                    code.extend_from_slice(&Instruction::Return.encode());
                }
            }
            _ => {
                // Ignore other types for now
            }
        }
        Ok(())
    }
}
