mod errors;

pub use crate::errors::cli_error::CliError;

#[derive(Debug)]
pub enum VmError {
    InvalidOpcode,
    StackUnderflow,
    IndexOutOfBounds,
    UnhandledEffect(String),
    UnhandledError,
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
