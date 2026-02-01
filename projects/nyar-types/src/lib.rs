pub use chomsky_extract::IKunTree;
use nyar_gc::{MarkContext, Trace};
use oak_core::Language;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod errors;

pub use crate::errors::cli_error::CliError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QualifiedName {
    pub parts: Vec<String>,
}

impl QualifiedName {
    pub fn new(parts: Vec<String>) -> Self {
        Self { parts }
    }
}

impl Trace for QualifiedName {
    fn trace(&self, _ctx: &mut MarkContext) {}
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join("::"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub source_id: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectInfo {
    pub name: QualifiedName,
    pub location: SourceLocation,
}

#[derive(Debug, Error)]
pub enum NyarError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Compile error: {0}")]
    Compile(String),
    #[error("VM error: {0:?}")]
    Vm(#[from] VmError),
    #[error("Format error: {0:?}")]
    Format(#[from] FormatError),
    #[error("Decode error: {0:?}")]
    Decode(#[from] DecodeError),
    #[error("Wasm AOT error: {0:?}")]
    WasmAot(#[from] WasmAotError),
    #[error("JVM AOT error: {0:?}")]
    JvmAot(#[from] JvmAotError),
    #[error("CLI error: {0}")]
    Cli(#[from] CliError),
}

#[derive(Debug, Error)]
pub enum VmError {
    #[error("Invalid opcode")]
    InvalidOpcode,
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Index out of bounds")]
    IndexOutOfBounds,
    #[error("Unhandled effect: {0}")]
    UnhandledEffect(QualifiedName),
    #[error("Unhandled error")]
    UnhandledError,
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Yield async")]
    YieldAsync,
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("Invalid header")]
    InvalidHeader,
    #[error("Truncated")]
    Truncated,
    #[error("Format error: {0}")]
    Text(String),
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),
    #[error("Truncated")]
    Truncated,
}

#[derive(Debug, Error)]
pub enum WasmAotError {
    #[error("Empty module")]
    EmptyModule,
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Unsupported opcode: {0}")]
    UnsupportedOpcode(String),
    #[error("Unsupported constant type")]
    UnsupportedConstantType,
    #[error("Constant out of bounds: {0}")]
    ConstantOutOfBounds(u16),
}

#[derive(Debug, Error)]
pub enum JvmAotError {
    #[error("Empty module")]
    EmptyModule,
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Unsupported opcode: {0}")]
    UnsupportedOpcode(String),
}

pub trait NyarFrontend: Default {
    type Language: Language;

    fn parse(&self, source: &str) -> Result<<Self::Language as Language>::TypedRoot, NyarError>;
    fn lower(&self, ast: &<Self::Language as Language>::TypedRoot) -> Result<IKunTree, NyarError>;
}
