use chomsky_uir::{EGraph, Id, IKunTree};
use chomsky_full::optimizer::UniversalOptimizer;
use chomsky_cost::DefaultCostModel;
use gaia_jit::JitMemory;
use gaia_assembler::program::{GaiaModule, GaiaFunction, GaiaBlock, GaiaConstant, GaiaTerminator};
use gaia_assembler::instruction::{GaiaInstruction, CoreInstruction};
use gaia_assembler::types::{GaiaSignature, GaiaType};

pub struct MiniCRuntime {
    _memory: Option<JitMemory>,
    optimizer: UniversalOptimizer<()>,
}

impl MiniCRuntime {
    pub fn new() -> Self {
        Self {
            _memory: None,
            optimizer: UniversalOptimizer::new(),
        }
    }

    pub fn execute(&mut self, intent_graph: (EGraph, Id)) -> Result<(), String> {
        let (egraph, root_id) = intent_graph;
        
        // 1. Extract the best tree using the default cost model
        let cost_model = DefaultCostModel::default();
        let tree = self.optimizer.extract(&egraph, root_id, cost_model);

        // 2. Translate IKunTree to Gaia Module
        let module = self.translate_to_gaia(&tree)?;

        // 3. Output or JIT execute
        println!("Executing Gaia Module: {}", module.name);
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
        
        // TODO: Load into JitMemory and call the entry point
        Ok(())
    }

    fn translate_to_gaia(&self, tree: &IKunTree) -> Result<GaiaModule, String> {
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
            IKunTree::Extension(name, items) if name == "module" => {
                // First item is module name, rest are items
                for item in items.iter().skip(1) {
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
                // If it's not a module, maybe it's a single function or expression
                // For now, let's wrap it in a main function
                let main = self.translate_function("main", &vec![], tree)?;
                module.functions.push(main);
            }
        }

        Ok(module)
    }

    fn translate_function(&self, name: &str, _params: &[String], body: &IKunTree) -> Result<GaiaFunction, String> {
        let mut instructions = vec![];
        
        // Very basic recursive translation
        self.translate_expr(body, &mut instructions)?;

        Ok(GaiaFunction {
            name: name.to_string(),
            signature: GaiaSignature {
                params: vec![],
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

    fn translate_expr(&self, tree: &IKunTree, insts: &mut Vec<GaiaInstruction>) -> Result<(), String> {
        match tree {
            IKunTree::Constant(v) => {
                insts.push(GaiaInstruction::Core(CoreInstruction::PushConstant(GaiaConstant::I32(*v as i32))));
            }
            IKunTree::Extension(op, args) if args.len() == 2 => {
                self.translate_expr(&args[0], insts)?;
                self.translate_expr(&args[1], insts)?;
                match op.as_str() {
                    "+" => insts.push(GaiaInstruction::Core(CoreInstruction::Add)),
                    "-" => insts.push(GaiaInstruction::Core(CoreInstruction::Sub)),
                    "*" => insts.push(GaiaInstruction::Core(CoreInstruction::Mul)),
                    "/" => insts.push(GaiaInstruction::Core(CoreInstruction::Div)),
                    _ => return Err(format!("Unsupported binary op: {}", op)),
                }
            }
            IKunTree::Seq(stmts) => {
                for stmt in stmts {
                    self.translate_expr(stmt, insts)?;
                }
            }
            IKunTree::Extension(name, args) if name == "return" && args.len() == 1 => {
                self.translate_expr(&args[0], insts)?;
                // Return is handled by GaiaTerminator usually, but here we push the value
            }
            _ => {
                // Ignore or error
            }
        }
        Ok(())
    }
}

