use chomsky_uir::{EGraph, Id, IKun, IKunTree};
use chomsky::optimizer::UniversalOptimizer;
use chomsky_cost::DefaultCostModel;
use chomsky_extract::IKunExtractor;
use nyar_vm::vm::interpreter::NyarVM;
use nyar_vm::bytecode::format::{NyarcModule, Chunk, ExportInfo};
use std::fmt::{Display, Formatter};
use std::error::Error;

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
        if let Some(export) = self.vm.modules[module_idx].exports.iter().find(|e| e.symbol == "main").or(self.vm.modules[module_idx].exports.first()) {
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
                            let chunk = self.translate_function(params, body)?;
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

    fn translate_function(&self, _params: &[String], _body: &IKunTree) -> Result<Chunk, RuntimeError> {
        // Placeholder for function translation
        Ok(Chunk::default())
    }
}
