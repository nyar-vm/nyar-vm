use chomsky::extract::IKunTree;
use chomsky::uir::IKun;
use nyar_types::VmError;
use nyar_vm::vm::value::Value;
use std::sync::Arc;

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
    pub ic: Arc<crate::ic::InlineCache>,
    /// Metadata for deoptimization, mapping machine code offsets to VM state.
    pub deopt_metadata: Vec<DeoptPoint>,
}

unsafe impl Send for CompiledCode {}
unsafe impl Sync for CompiledCode {}

/// The function signature for JIT-compiled code.
pub type JitEntry = unsafe extern "C" fn(
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
                args.iter()
                    .map(|&id| Self::from_uir_id(id, context))
                    .collect(),
            ),
            IKun::Extension(name, args) => IKunTree::Extension(
                name.clone(),
                args.iter()
                    .map(|&id| Self::from_uir_id(id, context))
                    .collect(),
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
