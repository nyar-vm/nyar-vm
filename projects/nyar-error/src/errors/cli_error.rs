use crate::{DecodeError, FormatError, VmError, WasmAotError, JvmAotError};

#[derive(Debug)]
pub enum CliError {
    Io(std::io::Error),
    Format(FormatError),
    Decode(DecodeError),
    Vm(VmError),
    Aot(WasmAotError),
    AotJvm(JvmAotError),
    NoChunk,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Io(e) => write!(f, "io error: {}", e),
            CliError::Format(FormatError::InvalidHeader) => {
                write!(f, "format error: invalid header")
            }
            CliError::Format(FormatError::Truncated) => write!(f, "format error: truncated data"),
            CliError::Format(FormatError::Text(msg)) => write!(f, "format error: {}", msg),
            CliError::Decode(DecodeError::InvalidOpcode(op)) => {
                write!(f, "decode error: invalid opcode 0x{:02X}", op)
            }
            CliError::Decode(DecodeError::Truncated) => write!(f, "decode error: truncated code"),
            CliError::Vm(VmError::InvalidOpcode) => write!(f, "vm error: invalid opcode"),
            CliError::Vm(VmError::StackUnderflow) => write!(f, "vm error: stack underflow"),
            CliError::Vm(VmError::IndexOutOfBounds) => write!(f, "vm error: index out of bounds"),
            CliError::Vm(VmError::UnhandledEffect(name)) => {
                write!(f, "vm error: unhandled effect {}", name)
            }
            CliError::Vm(VmError::UnhandledError) => write!(f, "vm error: unhandled error"),
            CliError::Vm(VmError::RuntimeError(msg)) => write!(f, "vm error: {}", msg),
            CliError::Aot(WasmAotError::EmptyModule) => write!(f, "aot error: empty module"),
            CliError::Aot(WasmAotError::Decode(msg)) => write!(f, "aot error: decode: {}", msg),
            CliError::Aot(WasmAotError::UnsupportedOpcode(op)) => {
                write!(f, "aot error: unsupported opcode {}", op)
            }
            CliError::Aot(WasmAotError::UnsupportedConstantType) => {
                write!(f, "aot error: unsupported constant type")
            }
            CliError::Aot(WasmAotError::ConstantOutOfBounds(idx)) => {
                write!(f, "aot error: constant out of bounds: {}", idx)
            }
            CliError::AotJvm(JvmAotError::EmptyModule) => write!(f, "jvm aot error: empty module"),
            CliError::AotJvm(JvmAotError::Decode(msg)) => write!(f, "jvm aot error: decode: {}", msg),
            CliError::AotJvm(JvmAotError::UnsupportedOpcode(op)) => write!(f, "jvm aot error: unsupported opcode {}", op),
            CliError::NoChunk => write!(f, "no chunk to execute"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}
impl From<FormatError> for CliError {
    fn from(e: FormatError) -> Self {
        CliError::Format(e)
    }
}
impl From<DecodeError> for CliError {
    fn from(e: DecodeError) -> Self {
        CliError::Decode(e)
    }
}
impl From<VmError> for CliError {
    fn from(e: VmError) -> Self {
        CliError::Vm(e)
    }
}
impl From<WasmAotError> for CliError {
    fn from(e: WasmAotError) -> Self {
        CliError::Aot(e)
    }
}
impl From<JvmAotError> for CliError {
    fn from(e: JvmAotError) -> Self {
        CliError::AotJvm(e)
    }
}
