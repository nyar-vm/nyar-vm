use crate::{QualifiedName, SourceLocation};

#[derive(Debug)]
pub struct NyarError {
    pub kind: Box<NyarErrorKind>,
    pub location: SourceLocation,
}

#[derive(Debug)]
pub enum NyarErrorKind {
    Io(std::io::Error),
    Vm(VmErrorKind),
    Aot(AotErrorKind),
    Jit(JitErrorKind),
    Cli(CliErrorKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmErrorKind {
    StackUnderflow,
    IndexOutOfBounds(usize),
    ModuleNotFound(usize),
    ChunkNotFound { module: usize, chunk: usize },
    SymbolNotFound(QualifiedName),
    NoActiveFrame,
    JitFailed(i32),
    LimitExceeded,
    Halt,
    ImplNotFound { class: u16, trait_id: u16 },
    UnhandledEffect(QualifiedName),
    TypeMismatch { expected: String, actual: String },
    Runtime(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AotErrorKind {
    EmptyModule,
    Decode(DecodeErrorKind),
    UnsupportedOpcode(String),
    UnsupportedConstant(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeErrorKind {
    InvalidOpcode(u8),
    Truncated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitErrorKind {
    Failed(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliErrorKind {
    NoChunk,
}

impl NyarError {
    pub fn new(kind: NyarErrorKind, location: SourceLocation) -> Self {
        Self {
            kind: Box::new(kind),
            location,
        }
    }
}

impl std::fmt::Display for NyarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.kind, self.location)
    }
}

impl std::error::Error for NyarError {}

impl std::fmt::Display for NyarErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarErrorKind::Io(e) => write!(f, "IO error: {}", e),
            NyarErrorKind::Vm(e) => write!(f, "VM error: {}", e),
            NyarErrorKind::Aot(e) => write!(f, "AOT error: {:?}", e),
            NyarErrorKind::Jit(e) => write!(f, "JIT error: {:?}", e),
            NyarErrorKind::Cli(e) => write!(f, "CLI error: {:?}", e),
        }
    }
}

impl std::fmt::Display for VmErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmErrorKind::StackUnderflow => write!(f, "Stack underflow"),
            VmErrorKind::IndexOutOfBounds(i) => write!(f, "Index out of bounds: {}", i),
            VmErrorKind::ModuleNotFound(i) => write!(f, "Module not found: {}", i),
            VmErrorKind::ChunkNotFound { module, chunk } => {
                write!(f, "Chunk not found: {} in module {}", chunk, module)
            }
            VmErrorKind::SymbolNotFound(name) => write!(f, "Symbol not found: {}", name),
            VmErrorKind::NoActiveFrame => write!(f, "No active frame"),
            VmErrorKind::JitFailed(code) => write!(f, "JIT failed: {}", code),
            VmErrorKind::LimitExceeded => write!(f, "Limit exceeded"),
            VmErrorKind::Halt => write!(f, "Halt"),
            VmErrorKind::ImplNotFound { class, trait_id } => {
                write!(f, "Impl not found for class {} and trait {}", class, trait_id)
            }
            VmErrorKind::UnhandledEffect(name) => write!(f, "Unhandled effect: {}", name),
            VmErrorKind::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, actual)
            }
            VmErrorKind::Runtime(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}
