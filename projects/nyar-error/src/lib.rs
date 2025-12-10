#[derive(Debug)]
pub enum VmError {
    InvalidOpcode,
    StackUnderflow,
    IndexOutOfBounds,
    UnhandledEffect(String),
    UnhandledError,
}

