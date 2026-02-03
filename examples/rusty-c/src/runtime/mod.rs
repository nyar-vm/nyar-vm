use chomsky::cost::DefaultCostModel;
use chomsky::extract::IKunExtractor;
use chomsky::optimizer::UniversalOptimizer;
use chomsky_uir::{EGraph, IKun, IKunTree, Id};
use nyar_vm::bytecode::instruction::Instruction;
use nyar_vm::bytecode::format::{Chunk, Constant, ExportInfo, NyarcModule};
use nyar_vm::vm::NyarVM;
use nyar_vm::runtime::NyarBuiltin;
use std::collections::HashMap;
use std::cell::RefCell;
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

pub struct RustyCRuntime {
    _optimizer: UniversalOptimizer<()>,
    vm: NyarVM,
    switch_counter: RefCell<usize>,
}

struct SwitchInfo {
    id: usize,
    cases: Vec<(i32, String)>,
    default_label: Option<String>,
}

impl RustyCRuntime {
    pub fn new() -> Self {
        Self {
            _optimizer: UniversalOptimizer::new(),
            vm: NyarVM::new(),
            switch_counter: RefCell::new(0),
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
                                let chunk = self.translate_function(params, body, &mut module)?;
                                let chunk_idx = module.chunks.len() as u16;
                                module.chunks.push(chunk);
                                module.exports.push(ExportInfo {
                                    symbol: name.clone().into(),
                                    chunk_idx,
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                let chunk = self.translate_function(&vec![], tree, &mut module)?;
                module.chunks.push(chunk);
                module.exports.push(ExportInfo {
                    symbol: "main".to_string().into(),
                    chunk_idx: 0,
                });
            }
        }

        Ok(module)
    }

    fn translate_function(
        &self,
        params: &[String],
        body: &IKunTree,
        module: &mut NyarcModule,
    ) -> Result<Chunk, RuntimeError> {
        let mut instructions = vec![];
        let mut symbols = HashMap::new();
        let mut labels = HashMap::new();
        let mut pending_gotos = Vec::new();

        // Handle parameters (map to locals)
        for (i, param) in params.iter().enumerate() {
            symbols.insert(param.clone(), i as u8);
        }

        self.translate_expr(
            body,
            &mut instructions,
            &mut symbols,
            None,
            None,
            None,
            &mut labels,
            &mut pending_gotos,
            None,
            module,
        )?;

        // Patch pending gotos
        for (label_name, jump_idx) in pending_gotos {
            if let Some(&target_pos) = labels.get(&label_name) {
                let jump_pos = self.calculate_code_size(&instructions[..jump_idx + 1]);
                instructions[jump_idx] = Instruction::Jump((target_pos as i16 - jump_pos as i16));
            } else {
                return Err(RuntimeError::Other(format!("Label '{}' not found", label_name)));
            }
        }

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
            ..Default::default()
        })
    }

    fn translate_expr(
        &self,
        tree: &IKunTree,
        insts: &mut Vec<Instruction>,
        symbols: &mut HashMap<String, u8>,
        mut break_indices: Option<&mut Vec<usize>>,
        mut continue_indices: Option<&mut Vec<usize>>,
        continue_pos: Option<usize>,
        labels: &mut HashMap<String, usize>,
        pending_gotos: &mut Vec<(String, usize)>,
        mut switch_info: Option<&mut SwitchInfo>,
        module: &mut NyarcModule,
    ) -> Result<(), RuntimeError> {
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
            IKunTree::Extension(op, args) if args.len() == 2 && ["+", "-", "*", "/", "==", "!=", "<", "<=", ">", ">="].contains(&op.as_str()) => {
                self.translate_expr(&args[0], insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;
                self.translate_expr(&args[1], insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;
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
                    _ => unreachable!(),
                }
            }
            IKunTree::Choice(cond, then_br, else_br) => {
                self.translate_expr(cond, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let jump_if_false_idx = insts.len();
                insts.push(Instruction::JumpIfFalse(0));

                self.translate_expr(then_br, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let jump_idx = insts.len();
                insts.push(Instruction::Jump(0));

                let then_start = jump_if_false_idx + 1;
                let then_end = jump_idx;
                let then_len = self.calculate_code_size(&insts[then_start..then_end]);

                self.translate_expr(else_br, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let else_start = jump_idx + 1;
                let else_end = insts.len();
                let else_len = self.calculate_code_size(&insts[else_start..else_end]);

                insts[jump_if_false_idx] = Instruction::JumpIfFalse(then_len as i16 + 3);
                insts[jump_idx] = Instruction::Jump(else_len as i16);
            }
            IKunTree::Extension(name, args) if name == "while" && args.len() == 2 => {
                let cond = &args[0];
                let body = &args[1];

                let start_pos = self.calculate_code_size(insts);
                self.translate_expr(cond, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let jump_if_false_idx = insts.len();
                insts.push(Instruction::JumpIfFalse(0));

                let mut current_break_indices = Vec::new();
                let mut current_continue_indices = Vec::new();
                
                self.translate_expr(body, insts, symbols, Some(&mut current_break_indices), Some(&mut current_continue_indices), Some(start_pos), labels, pending_gotos, switch_info.as_mut().map(|s| &mut **s), module)?;
                
                let current_pos = self.calculate_code_size(insts);
                insts.push(Instruction::Jump((start_pos as i16 - current_pos as i16 - 3)));

                let end_pos = self.calculate_code_size(insts);
                let loop_len = end_pos - self.calculate_code_size(&insts[..jump_if_false_idx + 1]);
                insts[jump_if_false_idx] = Instruction::JumpIfFalse(loop_len as i16);

                // Patch breaks
                for idx in current_break_indices {
                    let break_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((end_pos - break_pos) as i16);
                }
                // Patch continues
                for idx in current_continue_indices {
                    let cont_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((start_pos as i16 - cont_pos as i16));
                }
            }
            IKunTree::Extension(name, args) if name == "do-while" && args.len() == 2 => {
                let body = &args[0];
                let cond = &args[1];

                let start_pos = self.calculate_code_size(insts);
                let mut current_break_indices = Vec::new();
                let mut current_continue_indices = Vec::new();

                self.translate_expr(body, insts, symbols, Some(&mut current_break_indices), Some(&mut current_continue_indices), None, labels, pending_gotos, switch_info.as_mut().map(|s| &mut **s), module)?;

                let continue_pos_val = self.calculate_code_size(insts);
                self.translate_expr(cond, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let current_pos = self.calculate_code_size(insts);
                // If cond is true, jump back to start
                insts.push(Instruction::I32Not);
                insts.push(Instruction::JumpIfFalse((start_pos as i16 - current_pos as i16 - 4))); // 4 bytes for Not + JumpIfFalse

                let end_pos = self.calculate_code_size(insts);
                // Patch breaks
                for idx in current_break_indices {
                    let break_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((end_pos - break_pos) as i16);
                }
                // Patch continues
                for idx in current_continue_indices {
                    let cont_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((continue_pos_val as i16 - cont_pos as i16));
                }
            }
            IKunTree::Extension(name, args) if name == "for" && args.len() == 4 => {
                let init = &args[0];
                let cond = &args[1];
                let step = &args[2];
                let body = &args[3];

                self.translate_expr(init, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let cond_pos = self.calculate_code_size(insts);
                self.translate_expr(cond, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let jump_if_false_idx = insts.len();
                insts.push(Instruction::JumpIfFalse(0));

                let mut current_break_indices = Vec::new();
                let mut current_continue_indices = Vec::new();

                self.translate_expr(body, insts, symbols, Some(&mut current_break_indices), Some(&mut current_continue_indices), None, labels, pending_gotos, switch_info.as_mut().map(|s| &mut **s), module)?;

                let step_pos = self.calculate_code_size(insts);
                self.translate_expr(step, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let current_pos = self.calculate_code_size(insts);
                insts.push(Instruction::Jump((cond_pos as i16 - current_pos as i16 - 3)));

                let end_pos = self.calculate_code_size(insts);
                let loop_len = end_pos - self.calculate_code_size(&insts[..jump_if_false_idx + 1]);
                insts[jump_if_false_idx] = Instruction::JumpIfFalse(loop_len as i16);

                // Patch breaks
                for idx in current_break_indices {
                    let break_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((end_pos - break_pos) as i16);
                }
                // Patch continues
                for idx in current_continue_indices {
                    let cont_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((step_pos as i16 - cont_pos as i16));
                }
            }
            IKunTree::Extension(name, args) if name == "switch" && args.len() == 2 => {
                let cond = &args[0];
                let body = &args[1];

                self.translate_expr(cond, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;

                let cond_idx = symbols.len() as u8;
                symbols.insert(format!("$switch_cond_{}", cond_idx), cond_idx);
                insts.push(Instruction::StoreLocal(cond_idx));

                // Dispatch placeholder
                let dispatch_jump_idx = insts.len();
                insts.push(Instruction::Jump(0));

                let mut current_break_indices = Vec::new();
                let mut current_switch_info = SwitchInfo {
                    id: {
                        let mut counter = self.switch_counter.borrow_mut();
                        *counter += 1;
                        *counter
                    },
                    cases: Vec::new(),
                    default_label: None,
                };
                
                self.translate_expr(body, insts, symbols, Some(&mut current_break_indices), continue_indices, continue_pos, labels, pending_gotos, Some(&mut current_switch_info), module)?;
                
                let body_end_idx = insts.len();
                insts.push(Instruction::Jump(0)); // Jump to end of switch

                let dispatch_start_pos = self.calculate_code_size(insts);
                let jump_to_dispatch = (dispatch_start_pos - self.calculate_code_size(&insts[..dispatch_jump_idx + 1])) as i16;
                insts[dispatch_jump_idx] = Instruction::Jump(jump_to_dispatch);

                // Dispatch logic
                for (val, label_name) in current_switch_info.cases {
                    if let Some(&target_pos) = labels.get(&label_name) {
                        insts.push(Instruction::LoadLocal(cond_idx));
                        insts.push(Instruction::I32Const(val));
                        insts.push(Instruction::I32Eq);
                        
                        let jump_to_next_case_idx = insts.len();
                        insts.push(Instruction::JumpIfFalse(0)); // If not equal, skip the jump to target
                        
                        let current_pos = self.calculate_code_size(insts);
                        insts.push(Instruction::Jump((target_pos as i16 - current_pos as i16 - 3)));
                        
                        let next_case_pos = self.calculate_code_size(insts);
                        let skip_len = next_case_pos - self.calculate_code_size(&insts[..jump_to_next_case_idx + 1]);
                        insts[jump_to_next_case_idx] = Instruction::JumpIfFalse(skip_len as i16);
                    }
                }

                if let Some(label_name) = current_switch_info.default_label {
                    if let Some(&target_pos) = labels.get(&label_name) {
                        let jump_pos = self.calculate_code_size(insts);
                        insts.push(Instruction::Jump((target_pos as i16 - jump_pos as i16 - 3)));
                    }
                }

                let final_pos = self.calculate_code_size(insts);
                let jump_to_end = (final_pos - self.calculate_code_size(&insts[..body_end_idx + 1])) as i16;
                insts[body_end_idx] = Instruction::Jump(jump_to_end);

                // Patch breaks
                for idx in current_break_indices {
                    let break_pos = self.calculate_code_size(&insts[..idx + 1]);
                    insts[idx] = Instruction::Jump((final_pos - break_pos) as i16);
                }
            }
            IKunTree::Extension(name, args) if name == "case" && args.len() == 2 => {
                if let Some(info) = switch_info {
                    if let IKunTree::Constant(val) = &args[0] {
                        let label_name = format!("$switch_{}_case_{}", info.id, val);
                        let pos = self.calculate_code_size(insts);
                        labels.insert(label_name.clone(), pos);
                        info.cases.push((*val as i32, label_name));
                        self.translate_expr(&args[1], insts, symbols, break_indices, continue_indices, continue_pos, labels, pending_gotos, Some(info), module)?;
                    }
                }
            }
            IKunTree::Extension(name, args) if name == "default" && args.len() == 1 => {
                if let Some(info) = switch_info {
                    let label_name = format!("$switch_{}_default", info.id);
                    let pos = self.calculate_code_size(insts);
                    labels.insert(label_name.clone(), pos);
                    info.default_label = Some(label_name);
                    self.translate_expr(&args[0], insts, symbols, break_indices, continue_indices, continue_pos, labels, pending_gotos, Some(info), module)?;
                }
            }
            IKunTree::Extension(name, args) if name == "label" && args.len() == 2 => {
                let label_name = format!("label_{:?}", args[0]); 
                let pos = self.calculate_code_size(insts);
                labels.insert(label_name, pos);
                self.translate_expr(&args[1], insts, symbols, break_indices, continue_indices, continue_pos, labels, pending_gotos, switch_info, module)?;
            }
            IKunTree::Extension(name, args) if name == "goto" && args.len() == 1 => {
                let label_name = format!("label_{:?}", args[0]);
                pending_gotos.push((label_name, insts.len()));
                insts.push(Instruction::Jump(0));
            }
            IKunTree::Extension(name, _args) if name == "break" => {
                if let Some(indices) = break_indices {
                    indices.push(insts.len());
                    insts.push(Instruction::Jump(0));
                }
            }
            IKunTree::Extension(name, _args) if name == "continue" => {
                if let Some(pos) = continue_pos {
                    let current_pos = self.calculate_code_size(insts);
                    insts.push(Instruction::Jump((pos as i16 - current_pos as i16 - 3)));
                } else if let Some(indices) = continue_indices {
                    indices.push(insts.len());
                    insts.push(Instruction::Jump(0));
                }
            }
            IKunTree::Seq(stmts) => {
                for stmt in stmts {
                    self.translate_expr(
                        stmt,
                        insts,
                        symbols,
                        break_indices.as_mut().map(|b| &mut **b),
                        continue_indices.as_mut().map(|c| &mut **c),
                        continue_pos,
                        labels,
                        pending_gotos,
                        switch_info.as_mut().map(|s| &mut **s),
                        module,
                    )?;
                }
            }
            IKunTree::Return(val) => {
                self.translate_expr(val, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;
                insts.push(Instruction::Return);
            }
            IKunTree::StateUpdate(target, value) => {
                self.translate_expr(value, insts, symbols, 
                    break_indices.as_mut().map(|b| &mut **b),
                    continue_indices.as_mut().map(|c| &mut **c),
                    continue_pos,
                    labels,
                    pending_gotos,
                    switch_info.as_mut().map(|s| &mut **s),
                    module
                )?;
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
            IKunTree::CrossLangCall {
                language,
                module_path,
                function_name,
                arguments,
            } => {
                for arg in arguments {
                    self.translate_expr(
                        arg,
                        insts,
                        symbols,
                        break_indices.as_mut().map(|b| &mut **b),
                        continue_indices.as_mut().map(|c| &mut **c),
                        continue_pos,
                        labels,
                        pending_gotos,
                        switch_info.as_mut().map(|s| &mut **s),
                        module,
                    )?;
                }
                let name = if language == "nyar" {
                    // Try to map to intrinsic ID
                    let id = match (module_path.as_str(), function_name.as_str()) {
                        ("io", "print") | ("", "print") => NyarBuiltin::Print as u32,
                        ("io", "println") | ("", "println") => NyarBuiltin::Println as u32,
                        ("std", "exit") | ("", "exit") => NyarBuiltin::Exit as u32,
                        ("time", "now") | ("", "get_time") => NyarBuiltin::GetTime as u32,
                        ("time", "sleep") | ("", "sleep") => NyarBuiltin::Sleep as u32,
                        ("ops", "add") | ("", "native_add") => NyarBuiltin::NativeAdd as u32,
                        ("std", "panic") | ("", "panic") => NyarBuiltin::Panic as u32,
                        ("math", "sin") | ("", "sin") => NyarBuiltin::MathSin as u32,
                        ("math", "sqrt") | ("", "sqrt") => NyarBuiltin::MathSqrt as u32,
                        ("mem", "alloc") | ("", "alloc") => NyarBuiltin::MemAlloc as u32,
                        ("math", "abs") | ("", "abs") => NyarBuiltin::MathAbs as u32,
                        ("math", "cos") | ("", "cos") => NyarBuiltin::MathCos as u32,
                        ("math", "tan") | ("", "tan") => NyarBuiltin::MathTan as u32,
                        ("ops", "bit_and") | ("", "bit_and") => NyarBuiltin::BitAnd as u32,
                        ("ops", "bit_or") | ("", "bit_or") => NyarBuiltin::BitOr as u32,
                        ("ops", "bit_xor") | ("", "bit_xor") => NyarBuiltin::BitXor as u32,
                        ("ops", "bit_not") | ("", "bit_not") => NyarBuiltin::BitNot as u32,
                        ("ops", "bit_shl") | ("", "bit_shl") => NyarBuiltin::BitShl as u32,
                        ("ops", "bit_shr") | ("", "bit_shr") => NyarBuiltin::BitShr as u32,
                        ("mem", "free") => NyarBuiltin::MemFree as u32,
                        ("mem", "realloc") => NyarBuiltin::MemRealloc as u32,
                        ("mem", "set") => NyarBuiltin::MemSet as u32,
                        ("mem", "copy") => NyarBuiltin::MemCopy as u32,
                        ("str", "len") => NyarBuiltin::StrLen as u32,
                        ("str", "cmp") => NyarBuiltin::StrCmp as u32,
                        ("math", "rand") => NyarBuiltin::MathRand as u32,
                        _ => 0,
                    };
                    if id > 0 {
                        format!("$intrinsic:{}", id)
                    } else {
                        format!("{}:{}:{}", language, module_path, function_name)
                    }
                } else {
                    format!("{}:{}:{}", language, module_path, function_name)
                };
                
                let name_constant = Constant::String(name);
                let name_idx = if let Some(idx) = module.constants.iter().position(|c| c == &name_constant) {
                    idx as u16
                } else {
                    let idx = module.constants.len() as u16;
                    module.constants.push(name_constant);
                    idx
                };
                insts.push(Instruction::FFICall(name_idx, arguments.len() as u8));
            }
            _ => {}
        }
        Ok(())
    }

    fn calculate_code_size(&self, insts: &[Instruction]) -> usize {
        insts.iter().map(|ins| ins.encode().len()).sum()
    }
}
