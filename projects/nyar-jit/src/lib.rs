use std::collections::HashMap;
use std::sync::Arc;

use chomsky::adapters::GaiaX86Adapter;
use chomsky::extract::{Backend, BackendArtifact, IKunTree};
use chomsky::optimizer::UniversalOptimizer;
use chomsky::uir::IKun;
use chomsky_rules::{AlgebraicSimplification, ConstantFolding};

pub struct BarrierElision;

impl<A: chomsky_uir::egraph::Analysis<IKun>> chomsky_rule_engine::RewriteRule<A> for BarrierElision {
    fn name(&self) -> &str {
        "barrier-elision"
    }

    fn apply(&self, egraph: &chomsky_uir::egraph::EGraph<IKun, A>) {
        let mut matches = Vec::new();
        for entry in egraph.classes.iter() {
            let (&id, eclass) = entry.pair();
            for node in &eclass.nodes {
                if let IKun::Extension(op, args) = node {
                    if op == "barrier" && args.len() == 1 {
                        let obj_id = egraph.union_find.find(args[0]);
                        if let Some(obj_class) = egraph.classes.get(&obj_id) {
                            for obj_node in &obj_class.nodes {
                                if let IKun::Extension(obj_op, _) = obj_node {
                                    if obj_op == "alloc" {
                                        // Barrier on freshly allocated object is redundant
                                        matches.push(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for id in matches {
            // Replace barrier with nop or just identity of the object
            let nop_id = egraph.add(IKun::Symbol("nop".to_string()));
            egraph.union(id, nop_id);
        }
    }
}

pub struct AllocationSinking;

impl<A: chomsky_uir::egraph::Analysis<IKun>> chomsky_rule_engine::RewriteRule<A> for AllocationSinking {
    fn name(&self) -> &str {
        "allocation-sinking"
    }

    fn apply(&self, egraph: &chomsky_uir::egraph::EGraph<IKun, A>) {
        let mut matches = Vec::new();
        for entry in egraph.classes.iter() {
            let (&id, eclass) = entry.pair();
            for node in &eclass.nodes {
                if let IKun::Extension(op, _args) = node {
                    if op == "alloc" {
                        // Check if this allocation escapes the current function
                        let mut escapes = false;
                        
                        // Search for all uses of this e-class
                        for other_entry in egraph.classes.iter() {
                            let other_class = other_entry.value();
                            for other_node in &other_class.nodes {
                                match other_node {
                                    IKun::Extension(other_op, other_args) => {
                                        // If used in something other than field access, it might escape
                                        if other_args.contains(&id) && 
                                           other_op != "load_field" && 
                                           other_op != "store_field" &&
                                           other_op != "barrier" &&
                                           other_op != "type_of" {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    IKun::Apply(_, other_args) => {
                                        if other_args.contains(&id) {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    IKun::Map(f, x) => {
                                        if *f == id || *x == id {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    IKun::StateUpdate(_k, v) => {
                                        // If stored into a global or upvalue, it escapes
                                        if *v == id {
                                            escapes = true;
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if escapes { break; }
                        }
                        
                        if !escapes {
                            matches.push(id);
                        }
                    }
                }
            }
        }

        for id in matches {
            // In a real implementation, we would replace the 'alloc' node
            // with a 'virtual_object' node that the backend can then
            // use to perform scalar replacement.
            let virtual_id = egraph.add(IKun::Extension("virtual_object".to_string(), vec![]));
            egraph.union(id, virtual_id);
        }
    }
}
use dashmap::DashMap;
use gaia_jit::JitMemory;
use nyar_types::VmError;

use nyar_vm::bytecode::decoder::{Decoder, Instruction};
use nyar_vm::bytecode::format::{Constant as NyarConstant};
use nyar_vm::vm::interpreter::NyarVM;

/// Represents the compilation tiers in NyarJit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JitTier {
    /// Tier 0: Interpreter (handled by VM)
    Interpreter = 0,
    /// Tier 1: Baseline JIT - fast compilation, minimal optimizations.
    Baseline = 1,
    /// Tier 2: Mid-tier JIT - moderate optimizations.
    Optimizing = 2,
    /// Tier 3: Extreme JIT - expensive E-Graph saturation and global optimizations.
    Extreme = 3,
}

/// Represents a compiled function artifact.
pub struct CompiledCode {
    /// Pointer to the executable machine code.
    pub entry_point: *const u8,
    /// The tier at which this code was compiled.
    pub tier: JitTier,
    /// The size of the generated machine code in bytes.
    pub size: usize,
    /// Inline cache data associated with this function.
    pub ic: Arc<InlineCache>,
    /// Metadata for deoptimization, mapping machine code offsets to VM state.
    pub deopt_metadata: Vec<DeoptPoint>,
}

unsafe impl Send for CompiledCode {}
unsafe impl Sync for CompiledCode {}

/// The function signature for JIT-compiled code.
/// 
/// # Arguments
/// * `stack_ptr` - Pointer to the VM value stack.
/// * `sp` - Pointer to the stack pointer (index).
/// * `locals_ptr` - Pointer to the local variables for the current frame.
/// 
/// # Returns
/// * `0` on success, non-zero for error codes (e.g., deoptimization request).
type JitEntry = unsafe extern "C" fn(
    stack_ptr: *mut Value,
    sp: *mut usize,
    locals_ptr: *mut Value,
    ip_ptr: *mut usize,
) -> i32;

/// Metadata for a single deoptimization point.
pub struct DeoptPoint {
    /// Offset within the machine code where deoptimization can occur.
    pub pc_offset: usize,
    /// Corresponding instruction pointer in the original bytecode.
    pub bc_offset: usize,
    /// Map of stack/register locations to VM values for state restoration.
    pub stack_map: Vec<StackSlot>,
}

/// Represents a location in the JIT frame.
pub enum StackSlot {
    Register(u8),
    Stack(i32),
    Constant(i64),
}

/// Inline Cache (IC) for dynamic dispatch optimization.
pub struct InlineCache {
    /// Maps call site IDs to a single monomorphic target entry.
    /// In a more advanced implementation, this could be a Vec for Polymorphic IC.
    pub entries: DashMap<u32, IcEntry>,
}

/// Represents an entry in the Inline Cache.
#[derive(Clone, Copy)]
pub struct IcEntry {
    /// The class ID or type tag we are caching for.
    pub class_id: u32,
    /// The actual target machine code address.
    pub target: *const u8,
}

unsafe impl Send for InlineCache {}
unsafe impl Sync for InlineCache {}

impl InlineCache {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Records a successful dispatch in the cache.
    pub fn record(&self, call_site: u32, class_id: u32, target: *const u8) {
        self.entries.insert(call_site, IcEntry { class_id, target });
    }

    /// Looks up a cached target for a call site.
    pub fn lookup(&self, call_site: u32, class_id: u32) -> Option<*const u8> {
        self.entries.get(&call_site).and_then(|entry| {
            if entry.class_id == class_id {
                Some(entry.target)
            } else {
                None
            }
        })
    }
}

/// The main JIT compiler for NyarVM.
use nyar_vm::vm::interpreter::JitProvider;
use nyar_vm::vm::value::Value;

impl JitProvider for NyarJit {
    fn try_execute(&self, vm: &mut NyarVM, module_idx: usize, chunk_idx: usize) -> Option<Result<Value, VmError>> {
        let key = (module_idx, chunk_idx);
        
        // 1. Check if already compiled
        if let Some(compiled) = self.code_cache.get(&key) {
            // Found compiled code, execute it!
            let result = self.execute_compiled(compiled.value(), vm);
            
            if compiled.tier < JitTier::Extreme {
                // Check if we should upgrade to the next tier
                let next_tier = match compiled.tier {
                    JitTier::Baseline => JitTier::Optimizing,
                    JitTier::Optimizing => JitTier::Extreme,
                    _ => unreachable!(),
                };
                
                let threshold = self.get_threshold(next_tier);
                let chunk = &vm.modules[module_idx].chunks[chunk_idx];
                
                // Use the same u8 overflow logic in JIT compiled code if needed, 
                // but here we are in the JIT controller, so we just check the current hotness.
                let hotness = chunk.hotness.load(std::sync::atomic::Ordering::Relaxed);
                
                if hotness >= threshold {
                    match self.compile(vm, module_idx, chunk_idx, next_tier, 0) {
                        Ok(_) => { /* Upgrade successful, next call will use it */ }
                        Err(e) => return Some(Err(e)),
                    }
                }
            }
            
            return Some(result);
        }

        // 2. Increment hotness in VM's chunk for baseline trigger
        let threshold = self.get_threshold(JitTier::Baseline);
        let chunk = &vm.modules[module_idx].chunks[chunk_idx];
        
        // We only trigger baseline when hotness is checked.
        // The actual increment happens in the interpreter using u8 overflow.
        let hotness = chunk.hotness.load(std::sync::atomic::Ordering::Relaxed);

        if hotness >= threshold {
            // Trigger baseline compilation
            match self.compile(vm, module_idx, chunk_idx, JitTier::Baseline, 0) {
                Ok(_) => {
                    // Compilation successful, next call will use it
                }
                Err(e) => return Some(Err(e)),
            }
        }

        None
    }

    fn osr(&self, vm: &NyarVM, module_idx: usize, chunk_idx: usize, target: u32) -> Result<*const u8, VmError> {
        // OSR allows transitioning from the interpreter to JITed code in the middle of a function.
        // For now, delegate to the internal osr method.
        self.osr_internal(vm, module_idx, chunk_idx, target)
    }
}

pub struct NyarJit {
    /// Optimizer for Tier 2 using E-Graph Equality Saturation.
    optimizer: std::sync::Mutex<UniversalOptimizer<()>>,
    /// Executable memory manager for JITed code.
    jit_mem: std::sync::Mutex<JitMemory>,
    /// Cache of compiled functions, indexed by (module_idx, chunk_idx).
    code_cache: DashMap<(usize, usize), Arc<CompiledCode>>,
    /// Cache for On-Stack Replacement entries, indexed by (module_idx, chunk_idx, target_offset).
    osr_cache: DashMap<(usize, usize, u32), Arc<CompiledCode>>,
    /// Inline Cache registry.
    ic_registry: DashMap<(usize, usize), Arc<InlineCache>>,
    /// Thresholds for triggering compilation to each tier.
    thresholds: HashMap<JitTier, u32>,
}

impl NyarJit {
    /// Creates a new NyarJit instance with specified memory capacity.
    pub fn new(capacity: usize) -> Result<Self, VmError> {
        let mut optimizer = UniversalOptimizer::new();
        
        // Register default optimization rules
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
        let jit_mem = std::sync::Mutex::new(JitMemory::new(capacity).map_err(|e| VmError::RuntimeError(e.to_string()))?);
        
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

    /// Executes compiled machine code.
    fn execute_compiled(&self, compiled: &CompiledCode, vm: &mut NyarVM) -> Result<Value, VmError> {
        let entry: JitEntry = unsafe { std::mem::transmute(compiled.entry_point) };
        
        // Use the locals from the current VM frame.
        let frame = vm.frames.last_mut().ok_or(VmError::RuntimeError("No active frame".to_string()))?;
        
        unsafe {
            let res_code = entry(
                vm.stack.as_mut_ptr(),
                &mut vm.sp as *mut usize,
                frame.locals.as_mut_ptr(),
                &mut frame.ip as *mut usize,
            );
            
            if res_code == 0 {
                // Success! JIT finished the whole function.
                // The result should be at the top of the stack.
                if vm.sp > 0 {
                    vm.sp -= 1;
                    Ok(vm.stack[vm.sp])
                } else {
                    Ok(Value::null())
                }
            } else if res_code == 1 {
                // OSR/Partial success: JIT finished a loop or portion and wants to return to interpreter.
                // State is already updated via pointers (stack, sp, locals).
                // We just need to return null as a placeholder, the VM loop will continue.
                Ok(Value::null())
            } else {
                // Handle deoptimization or errors
                Err(VmError::RuntimeError(format!("JIT execution failed with code {}", res_code)))
            }
        }
    }

    /// Compiles a chunk of bytecode into machine code, optionally starting from an offset (for OSR).
    pub fn compile(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        tier: JitTier,
        start_offset: usize,
    ) -> Result<Arc<CompiledCode>, VmError> {
        let key = (module_idx, chunk_idx);

        // 1. Get or create Inline Cache for this chunk
        let ic = self.ic_registry.entry(key).or_insert_with(|| Arc::new(InlineCache::new())).value().clone();

        // 2. Intent Extraction
        let intents = self.extract_intents(vm, module_idx, chunk_idx, start_offset);

        // 3. Build initial IKunTree from intents
        let context = intents.clone();
        let tree = if !intents.is_empty() {
            IKunTree::from_uir_id(intents.len() - 1, &context)
        } else {
            IKunTree::Symbol("empty_chunk".to_string())
        };

        // 4. E-Graph Optimization
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

        // 5. Machine Code Generation via Gaia
        let backend = self.get_backend();
        self.generate_and_cache(key, &optimized_tree, tier, backend.as_ref(), ic)
    }

    fn add_tree_to_egraph(&self, optimizer: &mut UniversalOptimizer<()>, tree: &IKunTree) -> chomsky::uir::Id {
        // Recursively add IKunTree nodes to E-Graph.
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
                let arg_ids = args.iter().map(|arg| self.add_tree_to_egraph(optimizer, arg)).collect();
                optimizer.add_intent(&IKun::Apply(f_id, arg_ids))
            }
            IKunTree::Extension(name, args) => {
                let arg_ids = args.iter().map(|arg| self.add_tree_to_egraph(optimizer, arg)).collect();
                optimizer.add_intent(&IKun::Extension(name.clone(), arg_ids))
            }
            IKunTree::StateUpdate(key, val) => {
                let key_id = self.add_tree_to_egraph(optimizer, key);
                let val_id = self.add_tree_to_egraph(optimizer, val);
                optimizer.add_intent(&IKun::StateUpdate(key_id, val_id))
            }
            _ => {
                optimizer.add_intent(&IKun::Symbol("unsupported_tree_node".to_string()))
            }
        }
    }

    fn extract_intents(&self, vm: &NyarVM, module_idx: usize, chunk_idx: usize, start_offset: usize) -> Vec<IKun> {
        let module = &vm.modules[module_idx];
        let chunk = &module.chunks[chunk_idx];
        let mut intents = Vec::new();
        let mut stack = Vec::new();
        let mut decoder = Decoder::new(&chunk.code);
        
        // Skip instructions until start_offset
        for _ in 0..start_offset {
            let _ = decoder.next_result();
        }

        while let Ok(instruction) = decoder.next_result() {
            match instruction {
                Instruction::Push(idx) => {
                    let constant = &module.constants[idx as usize];
                    let intent = match constant {
                        NyarConstant::Int(v) => IKun::Constant(*v),
                        NyarConstant::Float(v) => IKun::FloatConstant(v.to_bits()),
                        NyarConstant::String(s) => IKun::StringConstant(s.clone()),
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
                Instruction::I32Add | Instruction::I64Add | Instruction::F32Add | Instruction::F64Add => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Map(lhs, rhs));
                        stack.push(id);
                    }
                }
                Instruction::I32Sub | Instruction::I64Sub | Instruction::F32Sub | Instruction::F64Sub => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("sub".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Mul | Instruction::I64Mul | Instruction::F32Mul | Instruction::F64Mul => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("mul".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::F32Div | Instruction::F64Div => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("div".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Neg | Instruction::I64Neg | Instruction::F32Neg | Instruction::F64Neg => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("neg".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::I32Eq | Instruction::I64Eq | Instruction::F32Eq | Instruction::F64Eq => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("eq".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32Ne | Instruction::I64Ne | Instruction::F32Ne | Instruction::F64Ne => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("ne".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32LtS | Instruction::I64LtS | Instruction::F32Lt | Instruction::F64Lt => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("lt".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32LeS | Instruction::I64LeS | Instruction::F32Le | Instruction::F64Le => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("le".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32GtS | Instruction::I64GtS | Instruction::F32Gt | Instruction::F64Gt => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("gt".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32GeS | Instruction::I64GeS | Instruction::F32Ge | Instruction::F64Ge => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("ge".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::LoadLocal(idx) => {
                    let const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    let id = intents.len();
                    intents.push(IKun::Extension("load_local".to_string(), vec![const_id]));
                    stack.push(id);
                }
                Instruction::StoreLocal(idx) => {
                    if let Some(val_id) = stack.pop() {
                        let key_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        intents.push(IKun::StateUpdate(key_id, val_id));
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
                    if let Some(val_id) = stack.pop() {
                        let key_id = intents.len();
                        intents.push(IKun::Extension("global_key".to_string(), vec![IKun::Constant(idx as i64)]));
                        intents.push(IKun::StateUpdate(key_id, val_id));
                    }
                }
                Instruction::Jump(offset) => {
                    let _id = intents.len();
                    intents.push(IKun::Extension("jump".to_string(), vec![]));
                    // Target offset could be stored in metadata or as a constant
                    let _target_id = intents.len();
                    intents.push(IKun::Constant(offset as i64));
                }
                Instruction::JumpIfFalse(offset) => {
                    if let Some(cond) = stack.pop() {
                        let id = intents.len();
                        let target_id = id + 1;
                        intents.push(IKun::Extension("branch_false".to_string(), vec![cond, target_id]));
                        intents.push(IKun::Constant(offset as i64));
                    }
                }
                Instruction::Call(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(id) = stack.pop() {
                            args.push(id);
                        }
                    }
                    args.reverse();
                    
                    let func_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    
                    let id = intents.len();
                    intents.push(IKun::Apply(func_id, args));
                    stack.push(id);
                }
                Instruction::NewObject(idx) => {
                    let _id = intents.len();
                    let const_id = _id;
                    intents.push(IKun::Constant(idx as i64));
                    
                    let alloc_id = intents.len();
                    intents.push(IKun::Extension("alloc".to_string(), vec![const_id]));
                    stack.push(alloc_id);
                }
                Instruction::GetField(idx) => {
                    if let Some(obj) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        
                        let id = intents.len();
                        intents.push(IKun::Extension("load_field".to_string(), vec![obj, const_id]));
                        stack.push(id);
                    }
                }
                Instruction::SetField(idx) => {
                    if let (Some(val), Some(obj)) = (stack.pop(), stack.pop()) {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        
                        let _store_id = intents.len();
                        intents.push(IKun::Extension("store_field".to_string(), vec![obj, const_id, val]));
                        
                        let _barrier_id = intents.len();
                        intents.push(IKun::Extension("barrier".to_string(), vec![obj]));
                    }
                }
                Instruction::LoadUpvalue(idx) => {
                    let id = intents.len();
                    intents.push(IKun::Symbol(format!("upvalue_{}", idx)));
                    stack.push(id);
                }
                Instruction::StoreUpvalue(idx) => {
                    if let Some(val) = stack.pop() {
                        let _id = intents.len();
                        let key_id = intents.len() + 1;
                        intents.push(IKun::StateUpdate(key_id, val));
                        intents.push(IKun::Symbol(format!("upvalue_{}", idx)));
                    }
                }
                Instruction::CallVirtual(idx, args_count) | Instruction::CallDynamic(idx, args_count) | Instruction::InvokeMethod(idx, args_count) | Instruction::CallSymbol(idx, args_count) => {
                    let mut args = Vec::new();
                    for _ in 0..args_count {
                        if let Some(arg) = stack.pop() {
                            args.push(arg);
                        }
                    }
                    args.reverse();
                    let _const_id = intents.len();
                    intents.push(IKun::Constant(idx as i64));
                    
                    let id = intents.len();
                    intents.push(IKun::Extension("dynamic_call".to_string(), args));
                    stack.push(id);
                }
                Instruction::I32DivS | Instruction::I64DivS => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("div_s".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::I32RemS | Instruction::I64RemS => {
                    if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                        let id = intents.len();
                        intents.push(IKun::Extension("rem_s".to_string(), vec![lhs, rhs]));
                        stack.push(id);
                    }
                }
                Instruction::Return => {
                    if let Some(val_id) = stack.pop() {
                        let intent_id = intents.len();
                        intents.push(IKun::Symbol(format!("id_{}", val_id)));
                        intents.push(IKun::Extension("return".to_string(), vec![intent_id]));
                    }
                }
                Instruction::TypeOf => {
                    if let Some(val) = stack.pop() {
                        let id = intents.len();
                        intents.push(IKun::Extension("type_of".to_string(), vec![val]));
                        stack.push(id);
                    }
                }
                Instruction::InstanceOf(idx) => {
                    if let Some(val) = stack.pop() {
                        let const_id = intents.len();
                        intents.push(IKun::Constant(idx as i64));
                        let id = intents.len();
                        intents.push(IKun::Extension("instance_of".to_string(), vec![val, const_id]));
                        stack.push(id);
                    }
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
                Instruction::Halt => {
                    intents.push(IKun::Extension("halt".to_string(), vec![]));
                }
                _ => {
                    // Other instructions can be added here
                }
            }
        }

        intents
    }

    fn get_backend(&self) -> Box<dyn Backend> {
        // Returns the appropriate Gaia backend for the current architecture.
        // For now, use the X86_64 adapter as a default.
        Box::new(GaiaX86Adapter)
    }

    /// Generates machine code from an IKunTree and caches it.
    pub fn generate_and_cache(
        &self,
        key: (usize, usize),
        tree: &IKunTree,
        tier: JitTier,
        backend: &dyn Backend,
        ic: Arc<InlineCache>,
    ) -> Result<Arc<CompiledCode>, VmError> {
        let artifact = backend.generate(tree)
            .map_err(|e| VmError::RuntimeError(format!("JIT Backend error: {:?}", e)))?;
            
        match artifact {
            BackendArtifact::Binary(code) => {
                let size = code.len();
                let mut jit_mem = self.jit_mem.lock().unwrap();
                jit_mem.write(&code).map_err(|e| VmError::RuntimeError(e.to_string()))?;
                
                let ptr = jit_mem.make_executable().map_err(|e| VmError::RuntimeError(e.to_string()))?;
                
                let compiled = Arc::new(CompiledCode {
                    entry_point: ptr,
                    tier,
                    size,
                    ic,
                    deopt_metadata: Vec::new(), // Populated by backend in a full implementation
                });
                
                self.code_cache.insert(key, compiled.clone());
                Ok(compiled)
            }
            BackendArtifact::Source(_) => {
                Err(VmError::RuntimeError("JIT backend produced source code instead of binary".to_string()))
            }
        }
    }

    /// Handles deoptimization by safely returning execution to the interpreter.
    pub fn deoptimize(&self, module_idx: usize, chunk_idx: usize) {
        // Invalidate the JITed code and update the VM state to resume in the interpreter.
        self.code_cache.remove(&(module_idx, chunk_idx));
    }

    /// Triggers On-Stack Replacement (OSR) for long-running loops.
    pub fn osr_internal(&self, vm: &NyarVM, module_idx: usize, chunk_idx: usize, target_offset: u32) -> Result<*const u8, VmError> {
        let key = (module_idx, chunk_idx, target_offset);
        
        // 1. Check if already compiled for this OSR target
        if let Some(compiled) = self.osr_cache.get(&key) {
            return Ok(compiled.entry_point);
        }
        
        // 2. Perform OSR compilation
        // We trigger a baseline compilation starting from target_offset.
        // In a real OSR, we would need to know the stack state at target_offset.
        // For now, we assume a simple case where the stack is relatively stable.
        let compiled = self.compile(vm, module_idx, chunk_idx, JitTier::Baseline, target_offset as usize)?;
        
        // 3. Cache and return
        self.osr_cache.insert(key, compiled.clone());
        Ok(compiled.entry_point)
    }

    /// GC-JIT Co-optimization: Barrier Elision.
    /// Informs the JIT that certain objects are guaranteed to be in the young generation,
    /// allowing it to skip write barriers.
    pub fn elide_barriers(&self, _tree: &mut IKunTree) {
        // Implementation would analyze object lifetimes and remove redundant barrier instructions.
    }

    /// GC-JIT Co-optimization: Allocation Sinking.
    /// Delays or eliminates heap allocations by keeping object fields in registers.
    pub fn sink_allocations(&self, _tree: &mut IKunTree) {
        // Implementation would use escape analysis to perform scalar replacement of objects.
    }

    /// Gets the hotness threshold for a specific tier.
    pub fn get_threshold(&self, tier: JitTier) -> u32 {
        self.thresholds.get(&tier).copied().unwrap_or(u32::MAX)
    }

    /// Sets the hotness threshold for a specific tier.
    pub fn set_threshold(&mut self, tier: JitTier, threshold: u32) {
        self.thresholds.insert(tier, threshold);
    }

    /// Clears the compiled code cache.
    pub fn clear_cache(&self) {
        self.code_cache.clear();
    }
}

pub trait FromUir {
    fn from_uir(ikun: &IKun, context: &[IKun]) -> Self;
    fn from_uir_id(id: chomsky::uir::Id, context: &[IKun]) -> Self;
}

impl FromUir for IKunTree {
    fn from_uir(ikun: &IKun, context: &[IKun]) -> IKunTree {
        match ikun {
            IKun::Constant(v) => IKunTree::Constant(*v),
            IKun::FloatConstant(v) => IKunTree::FloatConstant(*v),
            IKun::BooleanConstant(v) => IKunTree::BooleanConstant(*v),
            IKun::StringConstant(s) => IKunTree::StringConstant(s.clone()),
            IKun::Symbol(s) => IKunTree::Symbol(s.clone()),
            IKun::Map(f, x) => IKunTree::Map(
                Box::new(Self::from_uir_id(*f, context)),
                Box::new(Self::from_uir_id(*x, context)),
            ),
            IKun::Filter(f, x) => IKunTree::Filter(
                Box::new(Self::from_uir_id(*f, context)),
                Box::new(Self::from_uir_id(*x, context)),
            ),
            IKun::Reduce(f, init, list) => IKunTree::Reduce(
                Box::new(Self::from_uir_id(*f, context)),
                Box::new(Self::from_uir_id(*init, context)),
                Box::new(Self::from_uir_id(*list, context)),
            ),
            IKun::Apply(f, args) => IKunTree::Apply(
                Box::new(Self::from_uir_id(*f, context)),
                args.iter().map(|&id| Self::from_uir_id(id, context)).collect(),
            ),
            IKun::Extension(name, args) => IKunTree::Extension(
                name.clone(),
                args.iter().map(|&id| Self::from_uir_id(id, context)).collect(),
            ),
            IKun::StateUpdate(key, val) => IKunTree::StateUpdate(
                Box::new(Self::from_uir_id(*key, context)),
                Box::new(Self::from_uir_id(*val, context)),
            ),
            _ => IKunTree::Symbol("unsupported".to_string()),
        }
    }

    fn from_uir_id(id: chomsky::uir::Id, context: &[IKun]) -> Self {
        if id < context.len() {
            Self::from_uir(&context[id], context)
        } else {
            IKunTree::Symbol(format!("unknown_id_{}", id))
        }
    }
}
