mod error_kinds;
mod native_wrap;

pub use self::error_kinds::NyarErrorKind;

use lsp_types::Range;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

pub type Result<T> = std::result::Result<T, NyarError>;

#[derive(Debug)]
pub struct NyarError {
    kind: Box<NyarErrorKind>,
    position: Option<Range>,
}

impl Error for NyarError {}

impl Display for NyarError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{:?}", self.kind)?;
        match self.position {
            Some(r) => write!(f, "--> {}:{}", r.start.line + 1, r.start.character + 1)?,
            None => write!(f, "--> <internal>")?,
        }
        Ok(())
    }
}

impl NyarError {
    pub fn lexer_error(msg: impl Into<String>) -> NyarError {
        Self { kind: box NyarErrorKind::LexerError { info: msg.into() }, position: None }
    }

    pub fn invalid_operation(op: &str, lhs: Option<String>, rhs: Option<String>, position: Option<Range>) -> NyarError {
        match (lhs, rhs) {
            (Some(a), Some(b)) => {
                Self { kind: Box::new(NyarErrorKind::InvalidOperationInfix { op: op.to_string(), lhs: a, rhs: b }), position }
            }
            (Some(a), _) => {
                Self { kind: Box::new(NyarErrorKind::InvalidOperationSuffix { op: op.to_string(), item_type: a }), position }
            }
            (_, Some(b)) => {
                Self { kind: Box::new(NyarErrorKind::InvalidOperationPrefix { op: op.to_string(), item_type: b }), position }
            }
            _ => unreachable!(),
        }
    }

    pub fn invalid_iterator(item_type: impl Into<String>, position: Option<Range>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidIterator { item_type: item_type.into() }), position }
    }

    pub fn invalid_cast(item_type: impl Into<String>, position: Option<Range>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidCast { item_type: item_type.into() }), position }
    }

    pub fn if_lost(position: Option<Range>) -> NyarError {
        Self { kind: box NyarErrorKind::IfLost, position }
    }

    pub fn if_non_bool(position: Option<Range>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::IfNonBoolean), position }
    }

    pub fn invalid_index(index: impl Into<String>, item_type: impl Into<String>, position: Option<Range>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidIndex { index: index.into(), item_type: item_type.into() }), position }
    }

    pub fn variable_not_found(name: impl Into<String>, position: Option<Range>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::VariableNotFound { name: name.into() }), position }
    }

    pub fn write_unwritable_object(name: impl Into<String>, position: Option<Range>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::WriteUnwritable { name: name.into() }), position }
    }

    pub fn msg(text: impl Into<String>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::CustomErrorText { text: text.into() }), position: None }
    }
}
