use chomsky::adapters::GaiaX86Adapter;
use chomsky::extract::{Backend, BackendArtifact, IKunTree};
use chomsky::optimizer::UniversalOptimizer;
use chomsky::uir::IKun;
use chomsky_rules::{AlgebraicSimplification, ConstantFolding};
use dashmap::DashMap;
use gaia_jit::JitMemory;
use nyar_types::VmError;
use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::instruction::Instruction;
use nyar_vm::bytecode::format::Constant as NyarConstant;
use nyar_vm::vm::core::{JitProvider, NyarVM};
use nyar_vm::vm::value::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ic::InlineCache;
use crate::rules::{AllocationSinking, BarrierElision};
use crate::types::{CompiledCode, FromUir, JitEntry, JitTier};

pub struct NyarJit {
    /// Optimizer for Tier 2 using E-Graph Equality Saturation.
    pub(crate) optimizer: std::sync::Mutex<UniversalOptimizer<()>>,
    /// Executable memory manager for JITed code.
    pub(crate) jit_mem: std::sync::Mutex<JitMemory>,
    /// Cache of compiled functions, indexed by (module_idx, chunk_idx).
    pub(crate) code_cache: DashMap<(usize, usize), Arc<CompiledCode>>,
    /// Cache for On-Stack Replacement entries, indexed by (module_idx, chunk_idx, target_offset).
    pub(crate) osr_cache: DashMap<(usize, usize, u32), Arc<CompiledCode>>,
    /// Inline Cache registry.
    pub(crate) ic_registry: DashMap<(usize, usize), Arc<InlineCache>>,
    /// Thresholds for triggering compilation to each tier.
    pub(crate) thresholds: HashMap<JitTier, u32>,
}

impl JitProvider for NyarJit {
    fn try_execute(
        &self,
        vm: &mut NyarVM,
        module_idx: usize,
        chunk_idx: usize,
    ) -> Option<Result<Value, VmError>> {
        let key = (module_idx, chunk_idx);

        if let Some(compiled) = self.code_cache.get(&key) {
            let result = match self.execute_compiled(compiled.value(), vm) {
                Ok(Some(val)) => Ok(val),
                Ok(None) => return None, // OSR Exit, continue with interpreter
                Err(e) => return Some(Err(e)),
            };

            if compiled.tier < JitTier::Extreme {
                let next_tier = match compiled.tier {
                    JitTier::Baseline => JitTier::Optimizing,
                    JitTier::Optimizing => JitTier::Extreme,
                    _ => unreachable!(),
                };

                let threshold = self.get_threshold(next_tier);
                let chunk = &vm.env.modules.get(&module_idx).unwrap().chunks[chunk_idx];
                let hotness = chunk.hotness.load(std::sync::atomic::Ordering::Relaxed);

                if hotness >= threshold {
                    match self.compile(vm, module_idx, chunk_idx, next_tier, 0, 0) {
                        Ok(_) => {}
                        Err(e) => return Some(Err(e)),
                    }
                }
            }
            return Some(result);
        }

        let threshold = self.get_threshold(JitTier::Baseline);
        let chunk = &vm.env.modules.get(&module_idx).unwrap().chunks[chunk_idx];
        let hotness = chunk.hotness.load(std::sync::atomic::Ordering::Relaxed);

        if hotness >= threshold {
            match self.compile(vm, module_idx, chunk_idx, JitTier::Baseline, 0, 0) {
                Ok(_) => {}
                Err(e) => return Some(Err(e)),
            }
        }

        None
    }

    fn osr(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        target: u32,
    ) -> Result<*const u8, VmError> {
        self.osr_internal(vm, module_idx, chunk_idx, target)
    }
}

impl NyarJit {
    pub fn new(capacity: usize) -> Result<Self, VmError> {
        let mut optimizer = UniversalOptimizer::new();
        optimizer.register_rule(
            chomsky_rule_engine::RuleCategory::Algebraic,
            Box::new(ConstantFolding),
        );
        optimizer.register_rule(
            chomsky_rule_engine::RuleCategory::Algebraic,
            Box::new(AlgebraicSimplification),
        );
        optimizer.register_rule(
            chomsky_rule_engine::RuleCategory::Aggressive,
            Box::new(BarrierElision),
        );
        optimizer.register_rule(
            chomsky_rule_engine::RuleCategory::Aggressive,
            Box::new(AllocationSinking),
        );

        let optimizer = std::sync::Mutex::new(optimizer);
        let jit_mem = std::sync::Mutex::new(
            JitMemory::new(capacity).map_err(|e| VmError::RuntimeError(e.to_string()))?,
        );

        let mut thresholds = HashMap::new();
        thresholds.insert(JitTier::Baseline, 256);
        thresholds.insert(JitTier::Optimizing, 5120);
        thresholds.insert(JitTier::Extreme, 51200);

        Ok(Self {
            optimizer,
            jit_mem,
            code_cache: DashMap::new(),
            osr_cache: DashMap::new(),
            ic_registry: DashMap::new(),
            thresholds,
        })
    }

    fn execute_compiled(
        &self,
        compiled: &CompiledCode,
        vm: &mut NyarVM,
    ) -> Result<Option<Value>, VmError> {
        let entry: JitEntry = unsafe { std::mem::transmute(compiled.entry_point) };
        let frame = vm
            .frames
            .last_mut()
            .ok_or(VmError::RuntimeError("No active frame".to_string()))?;

        unsafe {
            let res_code = entry(
                vm.stack.as_mut_ptr(),
                &mut vm.sp as *mut usize,
                frame.locals.as_mut_ptr(),
                &mut frame.ip as *mut usize,
                frame.closure,
                vm as *mut NyarVM,
            );

            match res_code {
                0 => {
                    if vm.sp > 0 {
                        vm.sp -= 1;
                        Ok(Some(vm.stack[vm.sp]))
                    } else {
                        Ok(Some(Value::null()))
                    }
                }
                1 => Ok(Some(Value::null())),
                2 => {
                    // OSR Exit: The JIT code has updated frame.ip and vm.sp.
                    // We need to return to the interpreter.
                    Ok(None)
                }
                _ => Err(VmError::RuntimeError(format!(
                    "JIT execution failed with code {}",
                    res_code
                ))),
            }
        }
    }

    pub fn compile(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        tier: JitTier,
        start_offset: usize,
        initial_stack_depth: usize,
    ) -> Result<Arc<CompiledCode>, VmError> {
        let key = (module_idx, chunk_idx);
        let ic = self
            .ic_registry
            .entry(key)
            .or_insert_with(|| Arc::new(InlineCache::new()))
            .value()
            .clone();
        let intents =
            self.extract_intents(vm, module_idx, chunk_idx, start_offset, initial_stack_depth);
        let context = intents.clone();
        let tree = if !intents.is_empty() {
            IKunTree::from_uir_id(intents.len() - 1, &context)
        } else {
            IKunTree::Symbol("empty_chunk".to_string())
        };

        let optimized_tree = match tier {
            JitTier::Extreme => {
                let mut optimizer = self.optimizer.lock().unwrap();
                optimizer.scheduler.fuel = 30;
                optimizer.scheduler.timeout = std::time::Duration::from_secs(10);
                let root_id = self.add_tree_to_egraph(&mut optimizer, &tree);
                let backend = self.get_backend();
                optimizer.optimize(&optimizer.egraph, root_id, backend.get_model())
            }
            JitTier::Optimizing => {
                let mut optimizer = self.optimizer.lock().unwrap();
                optimizer.scheduler.fuel = 5;
                optimizer.scheduler.timeout = std::time::Duration::from_millis(500);
                let root_id = self.add_tree_to_egraph(&mut optimizer, &tree);
                let backend = self.get_backend();
                optimizer.optimize(&optimizer.egraph, root_id, backend.get_model())
            }
            _ => tree,
        };

        let backend = self.get_backend();
        self.generate_and_cache(key, &optimized_tree, tier, backend.as_ref(), ic)
    }

    fn add_tree_to_egraph(
        &self,
        optimizer: &mut UniversalOptimizer<()>,
        tree: &IKunTree,
    ) -> chomsky::uir::Id {
        match tree {
            IKunTree::Constant(v) => optimizer.add_intent(&IKun::Constant(*v)),
            IKunTree::FloatConstant(v) => optimizer.add_intent(&IKun::FloatConstant(*v)),
            IKunTree::BooleanConstant(v) => optimizer.add_intent(&IKun::BooleanConstant(*v)),
            IKunTree::StringConstant(s) => optimizer.add_intent(&IKun::StringConstant(s.clone())),
            IKunTree::Symbol(s) => optimizer.add_intent(&IKun::Symbol(s.clone())),
            IKunTree::Map(f, x) => {
                let f_id = self.add_tree_to_egraph(optimizer, f);
                let x_id = self.add_tree_to_egraph(optimizer, x);
                optimizer.add_intent(&IKun::Map(f_id, x_id))
            }
            IKunTree::Filter(f, x) => {
                let f_id = self.add_tree_to_egraph(optimizer, f);
                let x_id = self.add_tree_to_egraph(optimizer, x);
                optimizer.add_intent(&IKun::Filter(f_id, x_id))
            }
            IKunTree::Reduce(f, init, list) => {
                let f_id = self.add_tree_to_egraph(optimizer, f);
                let init_id = self.add_tree_to_egraph(optimizer, init);
                let list_id = self.add_tree_to_egraph(optimizer, list);
                optimizer.add_intent(&IKun::Reduce(f_id, init_id, list_id))
            }
            IKunTree::Apply(f, args) => {
                let f_id = self.add_tree_to_egraph(optimizer, f);
                let arg_ids = args
                    .iter()
                    .map(|arg| self.add_tree_to_egraph(optimizer, arg))
                    .collect();
                optimizer.add_intent(&IKun::Apply(f_id, arg_ids))
            }
            IKunTree::Choice(c, t, f) => {
                let c_id = self.add_tree_to_egraph(optimizer, c);
                let t_id = self.add_tree_to_egraph(optimizer, t);
                let f_id = self.add_tree_to_egraph(optimizer, f);
                optimizer.add_intent(&IKun::Choice(c_id, t_id, f_id))
            }
            IKunTree::Return(v) => {
                let v_id = self.add_tree_to_egraph(optimizer, v);
                optimizer.add_intent(&IKun::Return(v_id))
            }
            IKunTree::Seq(nodes) => {
                let node_ids = nodes
                    .iter()
                    .map(|n| self.add_tree_to_egraph(optimizer, n))
                    .collect();
                optimizer.add_intent(&IKun::Seq(node_ids))
            }
            IKunTree::Extension(name, args) => {
                let arg_ids = args
                    .iter()
                    .map(|arg| self.add_tree_to_egraph(optimizer, arg))
                    .collect();
                optimizer.add_intent(&IKun::Extension(name.clone(), arg_ids))
            }
            IKunTree::StateUpdate(key, val) => {
                let key_id = self.add_tree_to_egraph(optimizer, key);
                let val_id = self.add_tree_to_egraph(optimizer, val);
                optimizer.add_intent(&IKun::StateUpdate(key_id, val_id))
            }
            _ => {
                // For other nodes, we might want to log them or handle them generically
                optimizer.add_intent(&IKun::Symbol("unsupported_tree_node".to_string()))
            }
        }
    }

    fn extract_intents(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        start_offset: usize,
        initial_stack_depth: usize,
    ) -> Vec<IKun> {
        let module = &vm.env.modules.get(&module_idx).unwrap();
        let chunk = &module.chunks[chunk_idx];
        let mut intents = Vec::new();
        let mut stack = Vec::new();

        for i in 0..initial_stack_depth {
            let id = intents.len();
            intents.push(IKun::Extension(format!("stack_slot_{}", i), vec![]));
            stack.push(id);
        }

        let mut decoder = Decoder::new(&chunk.code);
        for _ in 0..start_offset {
            let _ = decoder.next_result();
        }

        while let Ok(instruction) = {
            let pos = decoder.position() as u32;
            intents.push(IKun::Extension(format!("label_{}", pos), vec![]));
            decoder.next_result()
        } {
            let current_pos = decoder.position() as i64;
            // The position before decoding was captured in the loop condition.
            // But we need the position relative to which the jump offset is calculated.
            // Usually, jump offsets are relative to the start of the next instruction.
            
            match instruction {
                Instruction::LoadLocal(idx) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("load_local".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::StoreLocal(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        intents.push(IKun::Extension(
                            "store_local".to_string(),
                            vec![const_id, val],
                        ));
                    }
                }
                Instruction::Return => {
                    if let Some(val) = stack.pop() {
                        intents.push(IKun::Extension("return".to_string(), vec![val]));
                    } else {
                        intents.push(IKun::Extension("return".to_string(), vec![]));
                    }
                }
                Instruction::Jump(off) => {
                    let target = (current_pos + off as i64) as u32;
                    let const_id = intents.len();
                    intents.push(IKun::Constant(target as i64));
                    if target < start_offset as u32 {
                        intents.push(IKun::Extension("osr_exit".to_string(), vec![const_id]));
                    } else {
                        intents.push(IKun::Extension("jump".to_string(), vec![const_id]));
                    }
                }
                Instruction::JumpIfFalse(off) => {
                    if let Some(cond) = stack.pop() {
                        let target = (current_pos + off as i64) as u32;
                        let const_id = intents.len();
                        intents.push(IKun::Constant(target as i64));
                        if target < start_offset as u32 {
                            // jump_if_false target < start => if !cond then osr_exit else continue
                            // We can use a local label for the "continue" case
                            let next_pos = decoder.position() as u32;
                            let next_label_id = intents.len();
                            intents.push(IKun::Constant(next_pos as i64));
                            
                            // Implementation of conditional OSR exit:
                            // if cond goto next_label
                            // osr_exit target
                            // next_label:
                            intents.push(IKun::Extension("jump_if_true".to_string(), vec![cond, next_label_id]));
                            intents.push(IKun::Extension("osr_exit".to_string(), vec![const_id]));
                            intents.push(IKun::Extension(format!("label_{}", next_pos), vec![]));
                        } else {
                            intents.push(IKun::Extension(
                                "jump_if_false".to_string(),
                                vec![cond, const_id],
                            ));
                        }
                    }
                }
                Instruction::JumpIfNull(off) => {
                    if let Some(val) = stack.pop() {
                        let target = (current_pos + off as i64) as u32;
                        let const_id = intents.len();
                        intents.push(IKun::Constant(target as i64));
                        if target < start_offset as u32 {
                            // jump_if_null target < start => if val is null then osr_exit else continue
                            let next_pos = decoder.position() as u32;
                            let next_label_id = intents.len();
                            intents.push(IKun::Constant(next_pos as i64));

                            // if val is NOT null goto next_label
                            // osr_exit target
                            // next_label:
                            intents.push(IKun::Extension("jump_if_not_null".to_string(), vec![val, next_label_id]));
                            intents.push(IKun::Extension("osr_exit".to_string(), vec![const_id]));
                            intents.push(IKun::Extension(format!("label_{}", next_pos), vec![]));
                        } else {
                            intents.push(IKun::Extension(
                                "jump_if_null".to_string(),
                                vec![val, const_id],
                            ));
                        }
                    }
                }
                Instruction::Push(idx) => {
                    let constant = &module.constants[idx as usize];
                    let intent = match constant {
                        NyarConstant::Int(v) => IKun::Constant(*v),
                        NyarConstant::Float(v) => IKun::FloatConstant(v.to_bits()),
                        NyarConstant::String(s) => IKun::StringConstant(s.clone()),
                        NyarConstant::QualifiedName(qn) => IKun::Symbol(qn.parts.join("::")),
                    };
                    let id = intents.len();
                    intents.push(intent);
                    stack.push(id);
                }
                Instruction::I32Const(v) => {
                    let id = intents.len();
                    intents.push(IKun::Constant(v as i64));
                    stack.push(id);
                }
                Instruction::I64Const(v) => {
                    let id = intents.len();
                    intents.push(IKun::Constant(v));
                    stack.push(id);
                }
                Instruction::F32Const(v) => {
                    let id = intents.len();
                    intents.push(IKun::FloatConstant(v.to_bits() as u64));
                    stack.push(id);
                }
                Instruction::F64Const(v) => {
                    let id = intents.len();
                    intents.push(IKun::FloatConstant(v.to_bits()));
                    stack.push(id);
                }
                Instruction::Pop => {
                    stack.pop();
                }
                Instruction::Dup(depth) => {
                    if let Some(&id) = stack.iter().rev().nth(depth as usize) {
                        stack.push(id);
                    }
                }
                Instruction::Swap(depth) => {
                    let len = stack.len();
                    if len > depth as usize {
                        stack.swap(len - 1, len - 1 - depth as usize);
                    }
                }
                Instruction::I32Add
                | Instruction::I64Add
                | Instruction::F32Add
                | Instruction::F64Add => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("add".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Sub
                | Instruction::I64Sub
                | Instruction::F32Sub
                | Instruction::F64Sub => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("sub".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Mul
                | Instruction::I64Mul
                | Instruction::F32Mul
                | Instruction::F64Mul => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("mul".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::F32Div
                | Instruction::F64Div
                | Instruction::I32DivS
                | Instruction::I32DivU
                | Instruction::I64DivS
                | Instruction::I64DivU => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("div".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32RemS
                | Instruction::I32RemU
                | Instruction::I64RemS
                | Instruction::I64RemU => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("rem".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Neg
                | Instruction::I64Neg
                | Instruction::F32Neg
                | Instruction::F64Neg => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("neg".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::I32Eq
                | Instruction::I64Eq
                | Instruction::F32Eq
                | Instruction::F64Eq => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("eq".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Ne
                | Instruction::I64Ne
                | Instruction::F32Ne
                | Instruction::F64Ne => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("ne".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32LtS
                | Instruction::I64LtS
                | Instruction::F32Lt
                | Instruction::F64Lt
                | Instruction::I32LtU
                | Instruction::I64LtU => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("lt".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32LeS
                | Instruction::I64LeS
                | Instruction::F32Le
                | Instruction::F64Le
                | Instruction::I32LeU
                | Instruction::I64LeU => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("le".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32GtS
                | Instruction::I64GtS
                | Instruction::F32Gt
                | Instruction::F64Gt
                | Instruction::I32GtU
                | Instruction::I64GtU => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("gt".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32GeS
                | Instruction::I64GeS
                | Instruction::F32Ge
                | Instruction::F64Ge
                | Instruction::I32GeU
                | Instruction::I64GeU => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("ge".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::LoadGlobal(idx) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("load_global".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::StoreGlobal(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        intents.push(IKun::Extension(
                            "store_global".to_string(),
                            vec![const_id, val],
                        ));
                    }
                }
                Instruction::LoadUpvalue(idx) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("load_upvalue".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::StoreUpvalue(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        intents.push(IKun::Extension(
                            "store_upvalue".to_string(),
                            vec![const_id, val],
                        ));
                    }
                }
                Instruction::Call(idx, args_count)
                | Instruction::CallVirtual(idx, args_count)
                | Instruction::CallDynamic(idx, args_count)
                | Instruction::InvokeMethod(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    let op = match instruction {
                        Instruction::Call(_, _) => "call",
                        Instruction::CallVirtual(_, _) => "call_virtual",
                        Instruction::CallDynamic(_, _) => "call_dynamic",
                        Instruction::InvokeMethod(_, _) => "invoke_method",
                        _ => unreachable!(),
                    };
                    intents.push(IKun::Extension(op.to_string(), {
                        let mut v = vec![const_id];
                        v.extend(args);
                        v
                    }));
                    stack.push(id);
                }
                Instruction::GetField(idx) => {
                    if let Some(obj) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        let id = intents.len();
                        intents.push(IKun::Extension("get_field".to_string(), vec![obj, const_id]));
                        stack.push(id);
                    }
                }
                Instruction::SetField(idx) => {
                    if let (Some(val), Some(obj)) = (stack.pop(), stack.pop()) {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        intents.push(IKun::Extension(
                            "set_field".to_string(),
                            vec![obj, const_id, val],
                        ));
                    }
                }
                Instruction::NewObject(idx) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("new_object".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::NewArray(count) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(count as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("new_array".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::GetElement => {
                    if let (Some(idx), Some(arr)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("get_element".to_string(), vec![arr, idx]));
                        stack.push(id);
                    }
                }
                Instruction::SetElement => {
                    if let (Some(val), Some(idx), Some(arr)) = (stack.pop(), stack.pop(), stack.pop()) {
                        intents.push(IKun::Extension(
                            "set_element".to_string(),
                            vec![arr, idx, val],
                        ));
                    }
                }
                Instruction::CallSymbol(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("call_symbol".to_string(), {
                        let mut v = vec![const_id];
                        v.extend(args);
                        v
                    }));
                    stack.push(id);
                }
                Instruction::CheckCast(idx) | Instruction::Cast(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        let id = intents.len();
                        intents.push(IKun::Extension("cast_to".to_string(), vec![val, const_id]));
                        stack.push(id);
                    }
                }
                Instruction::CloseUpvalues => {
                    intents.push(IKun::Extension("close_upvalues".to_string(), vec![]));
                }
                Instruction::MakeClosure(idx, upvalues) => {
                    let mut captures = Vec::new();
                    for up in upvalues {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(up.index as i64));
                        let is_local_id = intents.len();
                        intents.push(IKun::BooleanConstant(up.is_local));
                        let cap_id = intents.len();
                        intents.push(IKun::Extension(
                            "capture_ref".to_string(),
                            vec![const_id, is_local_id],
                        ));
                        captures.push(cap_id);
                    }
                    let func_const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("make_closure".to_string(), {
                        let mut v = vec![func_const_id];
                        v.extend(captures);
                        v
                    }));
                    stack.push(id);
                }
                Instruction::TailCall(idx, argc) => {
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    intents.push(IKun::Extension("tail_call".to_string(), {
                        let mut v = vec![const_id];
                        v.extend(args);
                        v
                    }));
                }
                Instruction::TailCallClosure(argc) => {
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    if let Some(closure_id) = stack.pop() {
                        intents.push(IKun::Extension("tail_call_closure".to_string(), {
                            let mut v = vec![closure_id];
                            v.extend(args);
                            v
                        }));
                    }
                }
                Instruction::CallClosure(argc) => {
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    if let Some(closure_id) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("call_closure".to_string(), {
                            let mut v = vec![closure_id];
                            v.extend(args);
                            v
                        }));
                        stack.push(id);
                    }
                }
                Instruction::MakeTuple(argc) => {
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let id = intents.len();
                    intents.push(IKun::Extension("make_tuple".to_string(), args));
                    stack.push(id);
                }
                Instruction::SizeOf => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("size_of".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::StringConcat => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("str_concat".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::StringLenBytes => {
                    if let Some(s) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("str_len_bytes".to_string(), vec![s]));
                        stack.push(id);
                    }
                }
                Instruction::StringSubstr => {
                    if let (Some(len), Some(start), Some(s)) =
                        (stack.pop(), stack.pop(), stack.pop())
                    {
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "str_substr".to_string(),
                            vec![s, start, len],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::StringEq
                | Instruction::StringNe
                | Instruction::StringLt
                | Instruction::StringLe
                | Instruction::StringGt
                | Instruction::StringGe => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let op = match instruction {
                            Instruction::StringEq => "eq",
                            Instruction::StringNe => "ne",
                            Instruction::StringLt => "lt",
                            Instruction::StringLe => "le",
                            Instruction::StringGt => "gt",
                            Instruction::StringGe => "ge",
                            _ => unreachable!(),
                        };
                        let id = intents.len();
                        intents.push(IKun::Extension(format!("str_{}", op), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::NewList(count) => {
                    let mut elements = Vec::new();
                    for _ in 0..count {
                        if let Some(e) = stack.pop() {
                            elements.push(e);
                        }
                    }
                    elements.reverse();
                    let id = intents.len();
                    intents.push(IKun::Extension("new_list".to_string(), elements));
                    stack.push(id);
                }
                Instruction::PushElementRight => {
                    if let (Some(val), Some(list)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "list_push_right".to_string(),
                            vec![list, val],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::PopElementRight => {
                    if let Some(list) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("list_pop_right".to_string(), vec![list]));
                        stack.push(id);
                    }
                }
                Instruction::HasKey => {
                    if let (Some(key), Some(obj)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("has_key".to_string(), vec![obj, key]));
                        stack.push(id);
                    }
                }
                Instruction::RemoveKey => {
                    if let (Some(key), Some(obj)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("remove_key".to_string(), vec![obj, key]));
                        stack.push(id);
                    }
                }
                Instruction::PushElementLeft => {
                    if let (Some(val), Some(list)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "list_push_left".to_string(),
                            vec![list, val],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::PopElementLeft => {
                    if let Some(list) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("list_pop_left".to_string(), vec![list]));
                        stack.push(id);
                    }
                }
                Instruction::BigIntConst { sign, bytes } => {
                    let mut bytes_ids = Vec::new();
                    for &b in bytes.iter() {
                        let cid = intents.len();
                        intents.push(IKun::Constant(b as i64));
                        bytes_ids.push(cid);
                    }
                    let bytes_id = intents.len();
                    intents.push(IKun::Extension(
                        "bigint_bytes".to_string(),
                        bytes_ids,
                    ));
                    let sign_id = intents.len();
                    intents.push(IKun::Constant(sign as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension(
                        "bigint_const".to_string(),
                        vec![sign_id, bytes_id],
                    ));
                    stack.push(id);
                }
                Instruction::BigIntAdd
                | Instruction::BigIntSub
                | Instruction::BigIntMul
                | Instruction::BigIntDiv
                | Instruction::BigIntMod => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let op = match instruction {
                            Instruction::BigIntAdd => "bigint_add",
                            Instruction::BigIntSub => "bigint_sub",
                            Instruction::BigIntMul => "bigint_mul",
                            Instruction::BigIntDiv => "bigint_div",
                            Instruction::BigIntMod => "bigint_mod",
                            _ => unreachable!(),
                        };
                        let id = intents.len();
                        intents.push(IKun::Extension(op.to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::BigIntNeg => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("bigint_neg".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::BigIntEq
                | Instruction::BigIntNe
                | Instruction::BigIntLt
                | Instruction::BigIntLe
                | Instruction::BigIntGt
                | Instruction::BigIntGe => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let op = match instruction {
                            Instruction::BigIntEq => "bigint_eq",
                            Instruction::BigIntNe => "bigint_ne",
                            Instruction::BigIntLt => "bigint_lt",
                            Instruction::BigIntLe => "bigint_le",
                            Instruction::BigIntGt => "bigint_gt",
                            Instruction::BigIntGe => "bigint_ge",
                            _ => unreachable!(),
                        };
                        let id = intents.len();
                        intents.push(IKun::Extension(op.to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::BigIntToI64 => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("bigint_to_i64".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::BigIntFromI64 => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("bigint_from_i64".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::BigIntToString => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("bigint_to_string".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::NewDynObject => {
                    let id = intents.len();
                    intents.push(IKun::Extension("new_dyn_object".to_string(), vec![]));
                    stack.push(id);
                }
                Instruction::MatchVariant(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "match_variant".to_string(),
                            vec![val, const_id],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::StringLenChars => {
                    if let Some(s) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("str_len_chars".to_string(), vec![s]));
                        stack.push(id);
                    }
                }
                Instruction::Perform(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("raise".to_string(), {
                        let mut v = vec![const_id];
                        v.extend(args);
                        v
                    }));
                    stack.push(id);
                }
                Instruction::WithHandler(idx) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("with_handler".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::ResumeWith => {
                    if let (Some(handler), Some(val)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "resume_with".to_string(),
                            vec![val, handler],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::CaptureCont => {
                    let id = intents.len();
                    intents.push(IKun::Extension("capture_cont".to_string(), vec![]));
                    stack.push(id);
                }
                Instruction::Await => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("await".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::BlockOn => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("block_on".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::MatchEffect(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "match_effect".to_string(),
                            vec![val, const_id],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::GetWitnessTable(idx1, idx2) => {
                    let c1 = intents.len();
                    intents.push(IKun::Constant(idx1 as i64));
                    let c2 = intents.len();
                    intents.push(IKun::Constant(idx2 as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension(
                        "get_witness_table".to_string(),
                        vec![c1, c2],
                    ));
                    stack.push(id);
                }
                Instruction::WitnessMethod(idx) => {
                    if let Some(table) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        let id = intents.len();
                        intents.push(IKun::Extension(
                            "witness_method".to_string(),
                            vec![table, const_id],
                        ));
                        stack.push(id);
                    }
                }
                Instruction::OpenExistential => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("open_existential".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::CloseExistential => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("close_existential".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::Quote(v) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(v as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("quote".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::Splice => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("splice".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::Eval(args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let id = intents.len();
                    intents.push(IKun::Extension("eval".to_string(), args));
                    stack.push(id);
                }
                Instruction::ExpandMacro(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("expand_macro".to_string(), {
                        let mut v = vec![const_id];
                        v.extend(args);
                        v
                    }));
                    stack.push(id);
                }
                Instruction::FFICall(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("ffi_call".to_string(), {
                        let mut v = vec![const_id];
                        v.extend(args);
                        v
                    }));
                    stack.push(id);
                }
                _ => {}
            }
        }
        intents
    }

    fn get_backend(&self) -> Box<dyn Backend> {
        Box::new(GaiaX86Adapter)
    }

    pub fn generate_and_cache(
        &self,
        key: (usize, usize),
        tree: &IKunTree,
        tier: JitTier,
        backend: &dyn Backend,
        ic: Arc<InlineCache>,
    ) -> Result<Arc<CompiledCode>, VmError> {
        let artifact = backend
            .generate(tree)
            .map_err(|e| VmError::RuntimeError(format!("JIT Backend error: {:?}", e)))?;
        match artifact {
            BackendArtifact::Binary(code) => {
                let size = code.len();
                let mut jit_mem = self.jit_mem.lock().unwrap();
                jit_mem
                    .write(&code)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                let ptr = jit_mem
                    .make_executable()
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                let compiled = Arc::new(CompiledCode {
                    entry_point: ptr,
                    tier,
                    size,
                    ic,
                    deopt_metadata: Vec::new(),
                });
                self.code_cache.insert(key, compiled.clone());
                Ok(compiled)
            }
            BackendArtifact::Source(_)
            | BackendArtifact::Assembly(_)
            | BackendArtifact::Collection(_) => Err(VmError::RuntimeError(
                "JIT backend produced non-binary artifact".to_string(),
            )),
        }
    }

    pub fn deoptimize(&self, module_idx: usize, chunk_idx: usize) {
        self.code_cache.remove(&(module_idx, chunk_idx));
    }

    pub fn osr_internal(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        target_offset: u32,
    ) -> Result<*const u8, VmError> {
        let key = (module_idx, chunk_idx, target_offset);
        if let Some(compiled) = self.osr_cache.get(&key) {
            return Ok(compiled.entry_point);
        }
        // OSR: Determine current stack depth for initialization
        let initial_stack_depth = vm.sp;
        let compiled = self.compile(
            vm,
            module_idx,
            chunk_idx,
            JitTier::Baseline,
            target_offset as usize,
            initial_stack_depth,
        )?;
        self.osr_cache.insert(key, compiled.clone());
        Ok(compiled.entry_point)
    }

    pub fn elide_barriers(&self, _tree: &mut IKunTree) {}
    pub fn sink_allocations(&self, _tree: &mut IKunTree) {}
    pub fn get_threshold(&self, tier: JitTier) -> u32 {
        self.thresholds.get(&tier).copied().unwrap_or(u32::MAX)
    }
    pub fn set_threshold(&mut self, tier: JitTier, threshold: u32) {
        self.thresholds.insert(tier, threshold);
    }
    pub fn clear_cache(&self) {
        self.code_cache.clear();
    }
}
