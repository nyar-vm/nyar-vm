use chomsky_uir::{EGraph, Id, IKun, IKunTree};
use chomsky_full::optimizer::UniversalOptimizer;
use chomsky_full::cost::DefaultCostModel;
use chomsky_full::extract::IKunExtractor;
use gaia_jit::JitMemory;
use gaia_assembler::program::{GaiaModule, GaiaFunction, GaiaBlock, GaiaConstant, GaiaTerminator};
use gaia_assembler::instruction::{GaiaInstruction, CoreInstruction, CmpCondition};
use gaia_assembler::types::{GaiaSignature, GaiaType};
use gaia_assembler::assembler::GaiaAssembler;
use gaia_types::helpers::{CompilationTarget, Architecture, AbiCompatible, ApiCompatible};
use anyhow::{Result, anyhow};

pub struct MiniCRuntime {
    _memory: Option<JitMemory>,
    _optimizer: UniversalOptimizer<()>,
    assembler: GaiaAssembler,
}

impl MiniCRuntime {
    pub fn new() -> Self {
        Self {
            _memory: None,
            _optimizer: UniversalOptimizer::new(),
            assembler: GaiaAssembler::new(),
        }
    }

    pub fn execute(&mut self, intent_graph: (EGraph<IKun, ()>, Id)) -> Result<()> {
        let (egraph, root_id) = intent_graph;
        
        // 1. Extract the best tree using the default cost model
        let cost_model = DefaultCostModel::default();
        let extractor = IKunExtractor::new(&egraph, cost_model);
        let tree = extractor.extract(root_id);

        // 2. Translate IKunTree to Gaia Module
        let module = self.translate_to_gaia(&tree)?;

        // 3. Compile using Gaia Assembler (Gaia Adapter Mode)
        println!("Compiling Gaia Module: {}", module.name);
        
        let target = CompilationTarget {
            build: Architecture::X86_64,
            host: AbiCompatible::PE,
            target: ApiCompatible::MicrosoftVisualC,
        };

        match self.assembler.compile(&module, &target) {
            Ok(files) => {
                println!("Successfully compiled to target: {:?}", target.build);
                for (name, bytes) in &files.files {
                    println!("  Generated file: {} ({} bytes)", name, bytes.len());
                }
            }
            Err(e) => {
                println!("Warning: Gaia Assembler failed to compile: {:?}", e);
                println!("Falling back to interpreted/simulated execution info:");
                for func in &module.functions {
                    println!("  Function: {}", func.name);
                    for block in &func.blocks {
                        println!("    Block {}:", block.label);
                        for inst in &block.instructions {
                            println!("      {:?}", inst);
                        }
                        println!("      {:?}", block.terminator);
                    }
                }
            }
        }
        
        Ok(())
    }

    fn translate_to_gaia(&self, tree: &IKunTree) -> Result<GaiaModule> {
        let mut module = GaiaModule {
            name: "mini-c-module".to_string(),
            functions: vec![],
            structs: vec![],
            classes: vec![],
            constants: vec![],
            globals: vec![],
            imports: vec![],
        };

        match tree {
            IKunTree::Module(name, items) => {
                module.name = name.clone();
                for item in items {
                    if let IKunTree::Export(name, body) = item {
                        if let IKunTree::Lambda(params, body) = &**body {
                            let func = self.translate_function(name, params, body)?;
                            module.functions.push(func);
                        }
                    }
                }
            }
            IKunTree::Extension(name, items) if name == "module" => {
                // Compatibility for old extension-style module
                for item in items.iter() {
                    if let IKunTree::StateUpdate(target, body) = item {
                        if let IKunTree::Symbol(name) = &**target {
                            if let IKunTree::Lambda(params, body) = &**body {
                                let func = self.translate_function(name, params, body)?;
                                module.functions.push(func);
                            }
                        }
                    }
                }
            }
            _ => {
                let main = self.translate_function("main", &vec![], tree)?;
                module.functions.push(main);
            }
        }

        Ok(module)
    }

    fn translate_function(&self, name: &str, params: &[String], body: &IKunTree) -> Result<GaiaFunction> {
        let mut instructions = vec![];
        let mut symbols = std::collections::HashMap::new();
        
        // Handle parameters
        let mut param_types = vec![];
        for (i, param) in params.iter().enumerate() {
            param_types.push(GaiaType::I32); // Default to I32 for mini-c
            symbols.insert(param.clone(), i as u32);
        }

        self.translate_expr(body, &mut instructions, &mut symbols)?;

        Ok(GaiaFunction {
            name: name.to_string(),
            signature: GaiaSignature {
                params: param_types,
                return_type: GaiaType::I32,
            },
            blocks: vec![GaiaBlock {
                label: "entry".to_string(),
                instructions,
                terminator: GaiaTerminator::Return,
            }],
            is_external: false,
        })
    }

    fn translate_expr(&self, tree: &IKunTree, insts: &mut Vec<GaiaInstruction>, symbols: &mut std::collections::HashMap<String, u32>) -> Result<()> {
        match tree {
            IKunTree::Constant(v) => {
                insts.push(GaiaInstruction::Core(CoreInstruction::PushConstant(GaiaConstant::I32(*v as i32))));
            }
            IKunTree::FloatConstant(v) => {
                insts.push(GaiaInstruction::Core(CoreInstruction::PushConstant(GaiaConstant::F64(f64::from_bits(*v)))));
            }
            IKunTree::Symbol(name) => {
                if let Some(&idx) = symbols.get(name) {
                    insts.push(GaiaInstruction::Core(CoreInstruction::LoadLocal(idx, GaiaType::I32)));
                } else {
                    // If not found, assume it's a new local (not ideal but works for mini-c examples)
                    let idx = symbols.len() as u32;
                    symbols.insert(name.clone(), idx);
                    insts.push(GaiaInstruction::Core(CoreInstruction::LoadLocal(idx, GaiaType::I32)));
                }
            }
            IKunTree::Extension(op, args) if args.len() == 2 => {
                self.translate_expr(&args[0], insts, symbols)?;
                self.translate_expr(&args[1], insts, symbols)?;
                match op.as_str() {
                    "+" => insts.push(GaiaInstruction::Core(CoreInstruction::Add(GaiaType::I32))),
                    "-" => insts.push(GaiaInstruction::Core(CoreInstruction::Sub(GaiaType::I32))),
                    "*" => insts.push(GaiaInstruction::Core(CoreInstruction::Mul(GaiaType::I32))),
                    "/" => insts.push(GaiaInstruction::Core(CoreInstruction::Div(GaiaType::I32))),
                    "==" => insts.push(GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Eq, GaiaType::I32))),
                    "!=" => insts.push(GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Ne, GaiaType::I32))),
                    "<" => insts.push(GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Lt, GaiaType::I32))),
                    "<=" => insts.push(GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Le, GaiaType::I32))),
                    ">" => insts.push(GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Gt, GaiaType::I32))),
                    ">=" => insts.push(GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Ge, GaiaType::I32))),
                    _ => {
                        println!("DEBUG: Unsupported binary op: '{}'", op);
                        return Err(anyhow!("Unsupported binary op: {}", op));
                    }
                }
            }
            IKunTree::Choice(cond, then_br, else_br) => {
                self.translate_expr(cond, insts, symbols)?;
                insts.push(GaiaInstruction::Core(CoreInstruction::BrTrue("then".to_string())));
                self.translate_expr(else_br, insts, symbols)?;
                insts.push(GaiaInstruction::Core(CoreInstruction::Br("end".to_string())));
                insts.push(GaiaInstruction::Core(CoreInstruction::Label("then".to_string())));
                self.translate_expr(then_br, insts, symbols)?;
                insts.push(GaiaInstruction::Core(CoreInstruction::Label("end".to_string())));
            }
            IKunTree::Repeat(cond, body) => {
                insts.push(GaiaInstruction::Core(CoreInstruction::Label("loop_start".to_string())));
                self.translate_expr(cond, insts, symbols)?;
                insts.push(GaiaInstruction::Core(CoreInstruction::BrFalse("loop_end".to_string())));
                self.translate_expr(body, insts, symbols)?;
                insts.push(GaiaInstruction::Core(CoreInstruction::Br("loop_start".to_string())));
                insts.push(GaiaInstruction::Core(CoreInstruction::Label("loop_end".to_string())));
            }
            IKunTree::Seq(stmts) => {
                for stmt in stmts {
                    self.translate_expr(stmt, insts, symbols)?;
                }
            }
            IKunTree::Extension(name, args) if name == "return" && args.len() == 1 => {
                self.translate_expr(&args[0], insts, symbols)?;
                insts.push(GaiaInstruction::Core(CoreInstruction::Ret));
            }
            IKunTree::StateUpdate(target, value) => {
                self.translate_expr(value, insts, symbols)?;
                if let IKunTree::Symbol(name) = &**target {
                    let idx = if let Some(&i) = symbols.get(name) {
                        i
                    } else {
                        let i = symbols.len() as u32;
                        symbols.insert(name.clone(), i);
                        i
                    };
                    insts.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(idx, GaiaType::I32)));
                }
            }
            _ => {
                // Ignore or log
            }
        }
        Ok(())
    }
}

