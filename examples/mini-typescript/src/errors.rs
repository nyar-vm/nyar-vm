use std::fmt::{Display, Formatter};
use std::error::Error;

pub type Result<T> = std::result::Result<T, ScriptError>;

#[derive(Debug, Clone)]
pub struct ScriptError {
    pub kind: Box<ScriptErrorKind>,
}

impl Display for ScriptError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.kind.as_ref() {
            ScriptErrorKind::CustomError { message } => write!(f, "{}", message),
            ScriptErrorKind::UnknownError => write!(f, "Unknown error"),
        }
    }
}

impl Error for ScriptError {}

impl From<String> for ScriptError {
    fn from(message: String) -> Self {
        Self { kind: Box::new(ScriptErrorKind::CustomError { message }) }
    }
}

impl From<&str> for ScriptError {
    fn from(message: &str) -> Self {
        Self { kind: Box::new(ScriptErrorKind::CustomError { message: message.to_string() }) }
    }
}

#[derive(Debug, Clone)]
pub enum ScriptErrorKind {
    CustomError { message: String },
    UnknownError,
}
