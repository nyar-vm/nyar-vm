use crate::{QualifiedName, SourceLocation};

#[derive(Debug)]
pub struct VmError {
    pub kind: VmErrorKind,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmErrorKind {
    InvalidOpcode,
    StackUnderflow,
    IndexOutOfBounds,
    ModuleIndexOutOfBounds(usize),
    ChunkIndexOutOfBounds {
        module_idx: usize,
        chunk_idx: usize,
    },
    NoActiveFrame,
    SymbolNotFound(QualifiedName),
    JitExecutionFailed(i32),
    InstructionLimitExceeded,
    HaltInstruction,
    ImplNotFound {
        class_idx: u16,
        trait_idx: u16,
    },
    UnhandledEffect(QualifiedName),
    UnhandledError,
    DivisionByZero,
    TypeMismatch {
        expected: String,
        actual: String,
    },
    RuntimeError(String),
    YieldAsync,
}

impl VmError {
    pub fn new(kind: VmErrorKind, location: SourceLocation) -> Self {
        Self { kind, location }
    }
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            VmErrorKind::YieldAsync => write!(f, "Yield async"),
            _ => write!(f, "{} at {}", self.kind, self.location),
        }
    }
}

impl std::error::Error for VmError {}

impl std::fmt::Display for VmErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmErrorKind::InvalidOpcode => write!(f, "Invalid opcode"),
            VmErrorKind::StackUnderflow => write!(f, "Stack underflow"),
            VmErrorKind::IndexOutOfBounds => write!(f, "Index out of bounds"),
            VmErrorKind::ModuleIndexOutOfBounds(idx) => {
                write!(f, "Module index out of bounds: {}", idx)
            }
            VmErrorKind::ChunkIndexOutOfBounds {
                module_idx,
                chunk_idx,
            } => {
                write!(
                    f,
                    "Chunk index out of bounds: {} in module {}",
                    chunk_idx, module_idx
                )
            }
            VmErrorKind::NoActiveFrame => write!(f, "No active frame"),
            VmErrorKind::SymbolNotFound(name) => write!(f, "Symbol not found: {}", name),
            VmErrorKind::JitExecutionFailed(code) => {
                write!(f, "JIT execution failed with code {}", code)
            }
            VmErrorKind::InstructionLimitExceeded => write!(f, "Maximum instruction limit exceeded"),
            VmErrorKind::HaltInstruction => write!(f, "Halt instruction encountered"),
            VmErrorKind::ImplNotFound {
                class_idx,
                trait_idx,
            } => {
                write!(
                    f,
                    "Impl not found for class {} and trait {}",
                    class_idx, trait_idx
                )
            }
            VmErrorKind::UnhandledEffect(name) => write!(f, "Unhandled effect: {}", name),
            VmErrorKind::UnhandledError => write!(f, "Unhandled error"),
            VmErrorKind::DivisionByZero => write!(f, "Division by zero"),
            VmErrorKind::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, actual)
            }
            VmErrorKind::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            VmErrorKind::YieldAsync => write!(f, "Yield async"),
        }
    }
}
