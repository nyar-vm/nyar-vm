pub mod value;
pub mod interpreter;
pub mod effects;
pub mod traits;
pub mod async_rt;
pub mod stage;
pub mod ffi;
pub mod safety;

#[derive(Debug)]
pub enum VmError {
    InvalidOpcode,
    StackUnderflow,
    IndexOutOfBounds,
    UnhandledEffect(String),
    UnhandledError,
}

