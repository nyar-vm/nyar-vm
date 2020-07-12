mod log_level;

pub use log_level::Level3;

#[derive(Debug, Clone)]
pub struct NyarError {
    kind: Box<ErrorKind>,
}

#[derive(Debug, Clone)]
pub enum ErrorKind {}

pub type Result<T> = std::result::Result<T, NyarError>;
