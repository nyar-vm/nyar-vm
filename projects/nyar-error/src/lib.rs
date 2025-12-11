mod errors;

pub use crate::errors::cli_error::CliError;

#[derive(Debug)]
pub enum VmError {
    InvalidOpcode,
    StackUnderflow,
    IndexOutOfBounds,
    UnhandledEffect(String),
    UnhandledError,
    RuntimeError(String),
}

#[derive(Debug)]
pub enum FormatError {
    InvalidHeader,
    Truncated,
    Text(String),
}

#[derive(Debug)]
pub enum DecodeError {
    InvalidOpcode(u8),
    Truncated,
}

#[derive(Debug)]
pub enum WasmAotError {
    EmptyModule,
    Decode(String),
    UnsupportedOpcode(String),
    UnsupportedConstantType,
    ConstantOutOfBounds(u16),
}

#[derive(Debug)]
pub enum JvmAotError {
    EmptyModule,
    Decode(String),
    UnsupportedOpcode(String),
}
