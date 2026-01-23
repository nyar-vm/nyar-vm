use chomsky_uir::{EGraph, Id, IKun, IKunTree};
use chomsky_full::optimizer::UniversalOptimizer;
use chomsky_full::cost::DefaultCostModel;
use chomsky_full::extract::IKunExtractor;
use nyar_vm::vm::interpreter::NyarVM;
use nyar_vm::bytecode::format::{NyarcModule, Chunk, ExportInfo};
use nyar_vm::bytecode::decoder::Instruction;
use std::fmt::{Display, Formatter};
use std::error::Error;
use std::collections::HashMap;

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

pub struct MiniCRuntime {
    _optimizer: UniversalOptimizer<()>,
    vm: NyarVM,
}

impl MiniCRuntime {
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
            IKunTree::Extension(name, items) if name == "module" => {
                for item in items.iter() {
                    if let IKunTree::StateUpdate(target, body) = item {
                        if let IKunTree::Symbol(name) = &**target {
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
            }
            _ => {
                let chunk = self.translate_function(&vec![], tree)?;
                module.chunks.push(chunk);
                module.exports.push(ExportInfo {
                    symbol: "main".to_string(),
                    chunk_idx: 0,
                });
            }
        }

        Ok(module)
    }

    fn translate_function(&self, params: &[String], body: &IKunTree) -> Result<Chunk, RuntimeError> {
        let mut instructions = vec![];
        let mut symbols = HashMap::new();
        
        // Handle parameters (map to locals)
        for (i, param) in params.iter().enumerate() {
            symbols.insert(param.clone(), i as u8);
        }

        self.translate_expr(body, &mut instructions, &mut symbols)?;
        
        // Add return if not present
        if instructions.last() != Some(&Instruction::Return) {
            instructions.push(Instruction::Return);
        }

        let mut code = vec![];
        for ins in instructions {
            code.extend_from_slice(&ins.encode());
        }

        Ok(Chunk {
            locals: 32,
            upvalues: 0,
            max_stack: 32,
            code,
            handlers: vec![],
            lines: vec![],
        })
    }

    fn translate_expr(&self, tree: &IKunTree, insts: &mut Vec<Instruction>, symbols: &mut HashMap<String, u8>) -> Result<(), RuntimeError> {
        match tree {
            IKunTree::Constant(v) => {
                insts.push(Instruction::I32Const(*v as i32));
            }
            IKunTree::Symbol(name) => {
                if let Some(&idx) = symbols.get(name) {
                    insts.push(Instruction::LoadLocal(idx));
                } else {
                    let idx = symbols.len() as u8;
                    symbols.insert(name.clone(), idx);
                    insts.push(Instruction::LoadLocal(idx));
                }
            }
            IKunTree::Extension(op, args) if args.len() == 2 => {
                self.translate_expr(&args[0], insts, symbols)?;
                self.translate_expr(&args[1], insts, symbols)?;
                match op.as_str() {
                    "+" => insts.push(Instruction::I32Add),
                    "-" => insts.push(Instruction::I32Sub),
                    "*" => insts.push(Instruction::I32Mul),
                    "/" => insts.push(Instruction::I32DivS),
                    "==" => insts.push(Instruction::I32Eq),
                    "!=" => insts.push(Instruction::I32Ne),
                    "<" => insts.push(Instruction::I32LtS),
                    "<=" => insts.push(Instruction::I32LeS),
                    ">" => insts.push(Instruction::I32GtS),
                    ">=" => insts.push(Instruction::I32GeS),
                    _ => return Err(RuntimeError::Other(format!("Unsupported binary op: {}", op))),
                }
            }
            IKunTree::Choice(cond, then_br, else_br) => {
                self.translate_expr(cond, insts, symbols)?;
                
                let jump_if_false_idx = insts.len();
                insts.push(Instruction::JumpIfFalse(0)); 
                
                self.translate_expr(then_br, insts, symbols)?;
                
                let jump_idx = insts.len();
                insts.push(Instruction::Jump(0)); 
                
                let then_start = jump_if_false_idx + 1;
                let then_end = jump_idx;
                let then_len = self.calculate_code_size(&insts[then_start..then_end]);
                
                self.translate_expr(else_br, insts, symbols)?;
                
                let else_start = jump_idx + 1;
                let else_end = insts.len();
                let else_len = self.calculate_code_size(&insts[else_start..else_end]);
                
                insts[jump_if_false_idx] = Instruction::JumpIfFalse(then_len as i16 + 3); 
                insts[jump_idx] = Instruction::Jump(else_len as i16);
            }
            IKunTree::Repeat(cond, body) => {
                let start_pos = self.calculate_code_size(insts);
                self.translate_expr(cond, insts, symbols)?;
                let jump_if_false_idx = insts.len();
                insts.push(Instruction::JumpIfFalse(0));
                self.translate_expr(body, insts, symbols)?;
                let body_end_pos = self.calculate_code_size(insts);
                let jump_back_offset = -( (body_end_pos - start_pos) as i16 + 3 );
                insts.push(Instruction::Jump(jump_back_offset));
                let final_pos = self.calculate_code_size(insts);
                let jump_forward_offset = (final_pos - self.calculate_code_size(&insts[..jump_if_false_idx+1])) as i16;
                insts[jump_if_false_idx] = Instruction::JumpIfFalse(jump_forward_offset);
            }
            IKunTree::Extension(name, args) if name == "while" && args.len() == 2 => {
                let start_pos = self.calculate_code_size(insts);
                self.translate_expr(&args[0], insts, symbols)?;
                let jump_if_false_idx = insts.len();
                insts.push(Instruction::JumpIfFalse(0));
                self.translate_expr(&args[1], insts, symbols)?;
                let body_end_pos = self.calculate_code_size(insts);
                let jump_back_offset = -( (body_end_pos - start_pos) as i16 + 3 );
                insts.push(Instruction::Jump(jump_back_offset));
                let final_pos = self.calculate_code_size(insts);
                let jump_forward_offset = (final_pos - self.calculate_code_size(&insts[..jump_if_false_idx+1])) as i16;
                insts[jump_if_false_idx] = Instruction::JumpIfFalse(jump_forward_offset);
            }
            IKunTree::Seq(stmts) => {
                for stmt in stmts {
                    self.translate_expr(stmt, insts, symbols)?;
                }
            }
            IKunTree::Apply(func, args) => {
                for arg in args {
                    self.translate_expr(arg, insts, symbols)?;
                }
                self.translate_expr(func, insts, symbols)?;
                insts.push(Instruction::CallClosure(args.len() as u8));
            }
            IKunTree::Extension(name, args) if name == "return" && args.len() == 1 => {
                self.translate_expr(&args[0], insts, symbols)?;
                insts.push(Instruction::Return);
            }
            IKunTree::StateUpdate(target, value) => {
                self.translate_expr(value, insts, symbols)?;
                if let IKunTree::Symbol(name) = &**target {
                    let idx = if let Some(&i) = symbols.get(name) {
                        i
                    } else {
                        let i = symbols.len() as u8;
                        symbols.insert(name.clone(), i);
                        i
                    };
                    insts.push(Instruction::StoreLocal(idx));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn calculate_code_size(&self, insts: &[Instruction]) -> usize {
        insts.iter().map(|i| i.encode().len()).sum()
    }
}
