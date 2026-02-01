use crate::errors::{AotError, DecodeError, FormatError, VmError, CliError};

#[derive(Debug)]
pub struct NyarError {
    pub kind: NyarErrorKind,
}

#[derive(Debug)]
pub enum NyarErrorKind {
    Io(std::io::Error),
    Parse(String),
    Compile(String),
    Vm(VmError),
    Format(FormatError),
    Decode(DecodeError),
    Aot(AotError),
    Cli(CliError),
}

impl NyarError {
    pub fn new(kind: NyarErrorKind) -> Self {
        Self { kind }
    }
}

impl std::fmt::Display for NyarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for NyarError {}

impl std::fmt::Display for NyarErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarErrorKind::Io(e) => write!(f, "IO error: {}", e),
            NyarErrorKind::Parse(e) => write!(f, "Parse error: {}", e),
            NyarErrorKind::Compile(e) => write!(f, "Compile error: {}", e),
            NyarErrorKind::Vm(e) => write!(f, "VM error: {}", e),
            NyarErrorKind::Format(e) => write!(f, "Format error: {}", e),
            NyarErrorKind::Decode(e) => write!(f, "Decode error: {}", e),
            NyarErrorKind::Aot(e) => write!(f, "AOT error: {}", e),
            NyarErrorKind::Cli(e) => write!(f, "CLI error: {}", e),
        }
    }
}

impl From<std::io::Error> for NyarError {
    fn from(e: std::io::Error) -> Self {
        Self::new(NyarErrorKind::Io(e))
    }
}

impl From<VmError> for NyarError {
    fn from(e: VmError) -> Self {
        Self::new(NyarErrorKind::Vm(e))
    }
}

impl From<FormatError> for NyarError {
    fn from(e: FormatError) -> Self {
        Self::new(NyarErrorKind::Format(e))
    }
}

impl From<DecodeError> for NyarError {
    fn from(e: DecodeError) -> Self {
        Self::new(NyarErrorKind::Decode(e))
    }
}

impl From<AotError> for NyarError {
    fn from(e: AotError) -> Self {
        Self::new(NyarErrorKind::Aot(e))
    }
}

impl From<CliError> for NyarError {
    fn from(e: CliError) -> Self {
        Self::new(NyarErrorKind::Cli(e))
    }
}
