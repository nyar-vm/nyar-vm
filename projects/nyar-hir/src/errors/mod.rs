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

    pub fn invalid_operation(op: &str, lhs: Option<String>, rhs: Option<String>, p: Range) -> NyarError {
        match (lhs, rhs) {
            (Some(a), Some(b)) => Self {
                kind: Box::new(NyarErrorKind::InvalidOperationInfix { op: op.to_string(), lhs: a, rhs: b }),
                position: Some(p),
            },
            (Some(a), _) => Self {
                kind: Box::new(NyarErrorKind::InvalidOperationSuffix { op: op.to_string(), item_type: a }),
                position: Some(p),
            },
            (_, Some(b)) => Self {
                kind: Box::new(NyarErrorKind::InvalidOperationPrefix { op: op.to_string(), item_type: b }),
                position: Some(p),
            },
            _ => unreachable!(),
        }
    }

    pub fn invalid_iterator(item_type: impl Into<String>, p: Range) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidIterator { item_type: item_type.into() }), position: Some(p) }
    }

    pub fn invalid_cast(item_type: impl Into<String>, p: Range) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidCast { item_type: item_type.into() }), position: Some(p) }
    }

    pub fn if_lost(p: Range) -> NyarError {
        Self { kind: box NyarErrorKind::IfLost, position: Some(p) }
    }

    pub fn if_non_bool(p: Range) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::IfNonBoolean), position: Some(p) }
    }

    pub fn invalid_index(index: impl Into<String>, item_type: impl Into<String>, p: Range) -> NyarError {
        Self {
            kind: Box::new(NyarErrorKind::InvalidIndex { index: index.into(), item_type: item_type.into() }),
            position: Some(p),
        }
    }
}
