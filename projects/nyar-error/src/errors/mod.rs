mod log_level;

pub use self::log_level::Level3;
use std::ops::Range;

pub type Result<T> = std::result::Result<T, NyarError>;

#[derive(Debug, Clone)]
pub enum ErrorKind {
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct NyarError {
    kind: Box<ErrorKind>,
    range: Option<Range<u32>>,
    file: Option<String>,
}

impl ErrorKind {
    pub fn to_error(self) -> NyarError {
        NyarError { kind: Box::new(self), range: None, file: None }
    }
}

impl NyarError {
    pub fn msg(s: impl Into<String>) -> NyarError {
        ErrorKind::Custom(s.into()).to_error()
    }

    pub fn with_range(self, range: Range<u32>) -> Self {
        Self { range: Some(range), ..self }
    }
    pub fn with_file(self, file: String) -> Self {
        Self { file: Some(file), ..self }
    }
}
