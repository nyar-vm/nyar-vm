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
    Format(FormatErrorKind),
    Decode(DecodeErrorKind),
    Runtime(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmErrorKind {
    StackUnderflow,
    IndexOutOfBounds(usize),
    ModuleNotFound(usize),
    ChunkNotFound { module: usize, chunk: usize },
    SymbolNotFound(QualifiedName),
    NoActiveFrame,
    LimitExceeded,
    Halt,
    ImplNotFound { class: u16, trait_id: u16 },
    UnhandledEffect(QualifiedName),
    TypeMismatch { expected: String, actual: String },
    YieldAsync,
    InvalidOpcode(u8),
    DivisionByZero,
    InvalidContinuation,
    FutureFailed(String),
    Runtime(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AotErrorKind {
    Generic(String),
    EmptyModule,
    UnsupportedOpcode(u8),
    UnsupportedConstant(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitErrorKind {
    Generic(String),
    Failed(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliErrorKind {
    NoChunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatErrorKind {
    InvalidHeader,
    Truncated,
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeErrorKind {
    InvalidOpcode(u8),
    Truncated,
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
            NyarErrorKind::Aot(e) => write!(f, "AOT error: {}", e),
            NyarErrorKind::Jit(e) => write!(f, "JIT error: {}", e),
            NyarErrorKind::Cli(e) => write!(f, "CLI error: {}", e),
            NyarErrorKind::Format(e) => write!(f, "Format error: {}", e),
            NyarErrorKind::Decode(e) => write!(f, "Decode error: {}", e),
            NyarErrorKind::Runtime(e) => write!(f, "Runtime error: {}", e),
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
            VmErrorKind::LimitExceeded => write!(f, "Limit exceeded"),
            VmErrorKind::Halt => write!(f, "Halt"),
            VmErrorKind::ImplNotFound { class, trait_id } => {
                write!(f, "Impl not found for class {} and trait {}", class, trait_id)
            }
            VmErrorKind::UnhandledEffect(name) => write!(f, "Unhandled effect: {}", name),
            VmErrorKind::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, actual)
            }
            VmErrorKind::YieldAsync => write!(f, "Yield async"),
            VmErrorKind::InvalidOpcode(op) => write!(f, "Invalid opcode: 0x{:02X}", op),
            VmErrorKind::DivisionByZero => write!(f, "Division by zero"),
            VmErrorKind::InvalidContinuation => write!(f, "Invalid continuation"),
            VmErrorKind::FutureFailed(msg) => write!(f, "Future failed: {}", msg),
            VmErrorKind::Runtime(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl std::fmt::Display for AotErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AotErrorKind::Generic(s) => write!(f, "{}", s),
            AotErrorKind::EmptyModule => write!(f, "Empty module"),
            AotErrorKind::UnsupportedOpcode(op) => write!(f, "Unsupported opcode: 0x{:02X}", op),
            AotErrorKind::UnsupportedConstant(idx) => write!(f, "Unsupported constant index: {}", idx),
        }
    }
}

impl std::fmt::Display for JitErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JitErrorKind::Generic(s) => write!(f, "{}", s),
            JitErrorKind::Failed(code) => write!(f, "Execution failed with code {}", code),
        }
    }
}

impl std::fmt::Display for CliErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliErrorKind::NoChunk => write!(f, "No chunk to execute"),
        }
    }
}

impl std::fmt::Display for FormatErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatErrorKind::InvalidHeader => write!(f, "Invalid header"),
            FormatErrorKind::Truncated => write!(f, "Truncated data"),
            FormatErrorKind::Text(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeErrorKind::InvalidOpcode(op) => write!(f, "Invalid opcode: 0x{:02X}", op),
            DecodeErrorKind::Truncated => write!(f, "Truncated code"),
        }
    }
}

impl From<std::io::Error> for NyarError {
    fn from(e: std::io::Error) -> Self {
        Self::new(NyarErrorKind::Io(e), SourceLocation::default())
    }
}
