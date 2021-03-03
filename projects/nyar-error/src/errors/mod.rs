use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use crate::Span;

pub use self::{error_kinds::NyarErrorKind, numeric_errors::ParseIntegerError};

mod error_kinds;
mod native_wrap;
mod numeric_errors;

pub type Result<T> = std::result::Result<T, NyarError>;

#[derive(Debug)]
pub struct NyarError {
    pub kind: Box<NyarErrorKind>,
    pub span: Span,
}

impl Error for NyarError {}

impl Display for NyarError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{:?}", self.kind)?;
        // match &self.span {
        //     Some(r) => write!(f, "--> ({}:{}, {}:{})", r.start.0, r.start.1, r.end.0, r.end.1)?,
        //     None => write!(f, "--> <internal>")?,
        // }
        Ok(())
    }
}

impl NyarError {
    pub fn set_range(&mut self, start: u32, end: u32) {
        self.span.start = start;
        self.span.end = end;
    }
    pub fn set_file_id(&mut self, file_id: u32) {
        self.span.file_id = file_id;
    }
    pub fn with_span(mut self, span: Span) -> Self {
        Self { kind: self.kind, span }
    }
}

impl NyarError {
    pub fn syntax_error(msg: impl Into<String>) -> NyarError {
        Self { kind: box NyarErrorKind::SyntaxError { info: msg.into() }, span: Default::default() }
    }

    pub fn invalid_operation(op: &str, lhs: Option<String>, rhs: Option<String>, position: Span) -> NyarError {
        match (lhs, rhs) {
            (Some(a), Some(b)) => Self {
                kind: Box::new(NyarErrorKind::InvalidOperationInfix { op: op.to_string(), lhs: a, rhs: b }),
                span: position,
            },
            (Some(a), _) => Self {
                kind: Box::new(NyarErrorKind::InvalidOperationSuffix { op: op.to_string(), item_type: a }),
                span: position,
            },
            (_, Some(b)) => Self {
                kind: Box::new(NyarErrorKind::InvalidOperationPrefix { op: op.to_string(), item_type: b }),
                span: position,
            },
            _ => unreachable!(),
        }
    }

    pub fn invalid_iterator(item_type: impl Into<String>, position: Span) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidIterator { item_type: item_type.into() }), span: position }
    }

    pub fn invalid_cast(item_type: impl Into<String>, position: Span) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::InvalidCast { item_type: item_type.into() }), span: position }
    }

    pub fn if_lost(position: Span) -> NyarError {
        Self { kind: box NyarErrorKind::IfLost, span: position }
    }

    pub fn if_non_bool(position: Span) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::IfNonBoolean), span: position }
    }

    pub fn invalid_index(index: impl Into<String>, item_type: impl Into<String>, position: Span) -> NyarError {
        Self {
            kind: Box::new(NyarErrorKind::InvalidIndex { index: index.into(), item_type: item_type.into() }),
            span: position,
        }
    }

    pub fn int_handler_not_found(handler: impl Into<String>, position: Span) -> NyarError {
        let kind = ParseIntegerError::HandlerNotFound(handler.into());
        Self { kind: Box::new(NyarErrorKind::ParseIntegerError { kind }), span: position }
    }

    pub fn variable_not_found(name: impl Into<String>, position: Span) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::VariableNotFound { name: name.into() }), span: position }
    }

    pub fn write_unwritable_object(name: impl Into<String>, position: Span) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::WriteUnwritable { name: name.into() }), span: position }
    }

    pub fn msg(text: impl Into<String>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::CustomErrorText { text: text.into() }), span: Default::default() }
    }
}
