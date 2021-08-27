use std::{
    char::ParseCharError,
    error::Error,
    fmt::{Display, Formatter, Write},
    num::{ParseFloatError, ParseIntError},
    ops::Range,
};

use crate::NyarError2;
use miette::{LabeledSpan, NamedSource};
use url::Url;

use crate::Span;

pub use self::error_kinds::NyarErrorKind;

mod error_kinds;
pub mod internal_error;
mod native_wrap;
pub mod parse_errors;

pub type Result<T> = std::result::Result<T, NyarError>;

#[derive(Debug)]
pub struct NyarError {
    pub kind: Box<NyarErrorKind>,
    pub span: Span,
}

impl Error for NyarError {}

impl Display for NyarError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
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
    pub fn with_span(self, span: Span) -> Self {
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

    pub fn invalid_index<S>(msg: S, position: Span) -> NyarError
    where
        S: Into<String>,
    {
        Self { kind: Box::new(NyarErrorKind::InvalidIndex { message: msg.into() }), span: position }
    }

    pub fn variable_not_found(name: impl Into<String>, position: Span) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::VariableNotFound { name: name.into() }), span: position }
    }

    pub fn read_write_error(name: impl Into<String>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::ReadWriteError { message: name.into() }), span: Default::default() }
    }

    pub fn msg(text: impl Into<String>) -> NyarError {
        Self { kind: Box::new(NyarErrorKind::CustomErrorText { text: text.into() }), span: Default::default() }
    }
}
