use crate::{QualifiedName, SourceLocation};

pub type VmError = NyarError;
pub type AotError = NyarError;

#[derive(Debug)]
pub enum FormatError {
    InvalidHeader,
    Truncated,
    Text(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

impl FormatError {
    pub fn key(&self) -> &'static str {
        match self {
            FormatError::InvalidHeader => "error.format.invalid_header",
            FormatError::Truncated => "error.format.truncated",
            FormatError::Text(_) => "error.format.text",
        }
    }
}

impl std::error::Error for FormatError {}

impl From<FormatError> for NyarError {
    fn from(e: FormatError) -> Self {
        match e {
            FormatError::InvalidHeader => Self::new(0x4001, NyarErrorKind::Format(FormatErrorKind::InvalidHeader), SourceLocation::default()),
            FormatError::Truncated => Self::new(0x4002, NyarErrorKind::Format(FormatErrorKind::Truncated), SourceLocation::default()),
            FormatError::Text(msg) => Self::new(0x4003, NyarErrorKind::Format(FormatErrorKind::Text(msg)), SourceLocation::default()),
        }
    }
}

#[derive(Debug)]
pub struct NyarError {
    pub code: u32,
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
    TypeMismatch { expected: String, found: String },
    YieldAsync,
    InvalidOpcode(u8),
    DivisionByZero,
    InvalidContinuation,
    FutureFailed,
    RuntimeError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AotErrorKind {
    Generic,
    Message(String),
    EmptyModule,
    UnsupportedOpcode(u8),
    UnsupportedConstant(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitErrorKind {
    Generic,
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
    pub fn new(code: u32, kind: NyarErrorKind, location: SourceLocation) -> Self {
        Self {
            code,
            kind: Box::new(kind),
            location,
        }
    }
    #[allow(non_snake_case)]
    pub fn Compile(msg: String) -> Self {
        Self::new(0x2001, NyarErrorKind::Aot(AotErrorKind::Message(msg)), SourceLocation::default())
    }
    #[allow(non_snake_case)]
    pub fn Parse(msg: String) -> Self {
        Self::new(0x2002, NyarErrorKind::Format(FormatErrorKind::Text(msg)), SourceLocation::default())
    }
    #[allow(non_snake_case)]
    pub fn RuntimeError(msg: String) -> Self {
        Self::new(0x3000, NyarErrorKind::Vm(VmErrorKind::RuntimeError(msg)), SourceLocation::default())
    }
}

impl std::fmt::Display for NyarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[E{:04X}] {} at {}", self.code, self.kind, self.location)
    }
}

impl std::error::Error for NyarError {}

impl NyarErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            NyarErrorKind::Io(_) => "error.io",
            NyarErrorKind::Vm(e) => e.key(),
            NyarErrorKind::Aot(e) => e.key(),
            NyarErrorKind::Jit(e) => e.key(),
            NyarErrorKind::Cli(e) => e.key(),
            NyarErrorKind::Format(e) => e.key(),
            NyarErrorKind::Decode(e) => e.key(),
        }
    }
}

impl std::fmt::Display for NyarErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarErrorKind::Io(e) => write!(f, "io: {}", e),
            NyarErrorKind::Vm(e) => write!(f, "vm: {}", e),
            NyarErrorKind::Aot(e) => write!(f, "aot: {}", e),
            NyarErrorKind::Jit(e) => write!(f, "jit: {}", e),
            NyarErrorKind::Cli(e) => write!(f, "cli: {}", e),
            NyarErrorKind::Format(e) => write!(f, "format: {}", e),
            NyarErrorKind::Decode(e) => write!(f, "decode: {}", e),
        }
    }
}

impl VmErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            VmErrorKind::StackUnderflow => "error.vm.stack_underflow",
            VmErrorKind::IndexOutOfBounds(_) => "error.vm.index_out_of_bounds",
            VmErrorKind::ModuleNotFound(_) => "error.vm.module_not_found",
            VmErrorKind::ChunkNotFound { .. } => "error.vm.chunk_not_found",
            VmErrorKind::SymbolNotFound(_) => "error.vm.symbol_not_found",
            VmErrorKind::NoActiveFrame => "error.vm.no_active_frame",
            VmErrorKind::LimitExceeded => "error.vm.limit_exceeded",
            VmErrorKind::Halt => "error.vm.halt",
            VmErrorKind::ImplNotFound { .. } => "error.vm.impl_not_found",
            VmErrorKind::UnhandledEffect(_) => "error.vm.unhandled_effect",
            VmErrorKind::TypeMismatch { .. } => "error.vm.type_mismatch",
            VmErrorKind::YieldAsync => "error.vm.yield_async",
            VmErrorKind::InvalidOpcode(_) => "error.vm.invalid_opcode",
            VmErrorKind::DivisionByZero => "error.vm.division_by_zero",
            VmErrorKind::InvalidContinuation => "error.vm.invalid_continuation",
            VmErrorKind::FutureFailed => "error.vm.future_failed",
            VmErrorKind::RuntimeError(_) => "error.vm.runtime_error",
        }
    }
}

impl std::fmt::Display for VmErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())?;
        match self {
            VmErrorKind::IndexOutOfBounds(i) => write!(f, "({})", i),
            VmErrorKind::ModuleNotFound(i) => write!(f, "({})", i),
            VmErrorKind::ChunkNotFound { module, chunk } => write!(f, "({}:{})", module, chunk),
            VmErrorKind::SymbolNotFound(name) => write!(f, "({})", name),
            VmErrorKind::ImplNotFound { class, trait_id } => write!(f, "({}:{})", class, trait_id),
            VmErrorKind::UnhandledEffect(name) => write!(f, "({})", name),
            VmErrorKind::TypeMismatch { expected, found } => write!(f, "({} != {})", expected, found),
            VmErrorKind::InvalidOpcode(op) => write!(f, "(0x{:02X})", op),
            VmErrorKind::RuntimeError(msg) => write!(f, "({})", msg),
            _ => Ok(()),
        }
    }
}

impl AotErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            AotErrorKind::Generic => "error.aot.generic",
            AotErrorKind::Message(_) => "error.aot.message",
            AotErrorKind::EmptyModule => "error.aot.empty_module",
            AotErrorKind::UnsupportedOpcode(_) => "error.aot.unsupported_opcode",
            AotErrorKind::UnsupportedConstant(_) => "error.aot.unsupported_constant",
        }
    }
}

impl std::fmt::Display for AotErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())?;
        match self {
            AotErrorKind::Message(msg) => write!(f, "({})", msg),
            AotErrorKind::UnsupportedOpcode(op) => write!(f, "(0x{:02X})", op),
            AotErrorKind::UnsupportedConstant(idx) => write!(f, "({})", idx),
            _ => Ok(()),
        }
    }
}

impl JitErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            JitErrorKind::Generic => "error.jit.generic",
            JitErrorKind::Failed(_) => "error.jit.failed",
        }
    }
}

impl std::fmt::Display for JitErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())?;
        match self {
            JitErrorKind::Failed(code) => write!(f, "({})", code),
            _ => Ok(()),
        }
    }
}

impl CliErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            CliErrorKind::NoChunk => "error.cli.no_chunk",
        }
    }
}

impl std::fmt::Display for CliErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

impl FormatErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            FormatErrorKind::InvalidHeader => "error.format.invalid_header",
            FormatErrorKind::Truncated => "error.format.truncated",
            FormatErrorKind::Text(_) => "error.format.text",
        }
    }
}

impl std::fmt::Display for FormatErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())?;
        match self {
            FormatErrorKind::Text(msg) => write!(f, "({})", msg),
            _ => Ok(()),
        }
    }
}

impl DecodeErrorKind {
    pub fn key(&self) -> &'static str {
        match self {
            DecodeErrorKind::InvalidOpcode(_) => "error.decode.invalid_opcode",
            DecodeErrorKind::Truncated => "error.decode.truncated",
        }
    }
}

impl std::fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())?;
        match self {
            DecodeErrorKind::InvalidOpcode(op) => write!(f, "(0x{:02X})", op),
            _ => Ok(()),
        }
    }
}

impl From<std::io::Error> for NyarError {
    fn from(e: std::io::Error) -> Self {
        Self::new(0x0001, NyarErrorKind::Io(e), SourceLocation::default())
    }
}
