pub mod async_rt;
pub mod effects;
pub mod ffi;
pub mod interpreter;
pub mod safety;
pub mod stage;
pub mod traits;
pub mod value;

#[derive(Debug)]
pub enum VmError {
    InvalidOpcode,
    StackUnderflow,
    IndexOutOfBounds,
    UnhandledEffect(String),
    UnhandledError,
}
