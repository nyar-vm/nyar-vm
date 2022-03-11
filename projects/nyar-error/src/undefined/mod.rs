use crate::{NyarError, NyarErrorKind};
use diagnostic::{Color, Diagnostic, FileID, FileSpan, ReportKind};
use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
    ops::Range,
};

#[derive(Clone, Debug)]
pub struct MissingError {
    pub(crate) kind: MissingErrorKind,
    pub(crate) span: FileSpan,
}

#[derive(Clone, Debug)]
enum MissingErrorKind {
    /// empty symbol
    EmptyPath,
    /// undefined symbol
    Undefined(Box<str>),
}

impl Error for MissingError {}

impl Display for MissingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.kind, f)
    }
}

impl Display for MissingErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "Empty symbol"),
            Self::Undefined(v) => write!(f, "Undefined symbol `{}`", v),
        }
    }
}

impl From<MissingError> for NyarError {
    fn from(value: MissingError) -> Self {
        value.as_error(ReportKind::Error)
    }
}

impl MissingError {
    pub fn empty() -> Self {
        Self { kind: MissingErrorKind::EmptyPath, span: FileSpan::default() }
    }
    pub fn undefined(symbol: &str) -> Self {
        Self { kind: MissingErrorKind::Undefined(Box::from(symbol)), span: FileSpan::default() }
    }
    pub fn with_span(self, span: FileSpan) -> Self {
        Self { span, ..self }
    }
    pub fn with_range(self, range: Range<usize>) -> Self {
        Self { span: self.span.with_range(range), ..self }
    }
    pub fn with_file(self, file: FileID) -> Self {
        Self { span: self.span.with_file(file), ..self }
    }
    pub fn as_error(self, kind: ReportKind) -> NyarError {
        NyarErrorKind::Missing(self).as_error(kind)
    }
    pub fn as_report(&self, kind: ReportKind) -> Diagnostic {
        let mut report = Diagnostic::new(kind).with_location(self.span.get_file(), Some(self.span.get_start()));
        report.set_message(self.to_string());
        match self.kind {
            MissingErrorKind::EmptyPath => {
                report.add_label(self.span.as_label("Symbol path cannot be empty").with_color(Color::Red))
            }
            MissingErrorKind::Undefined(_) => {
                report.add_label(self.span.as_label("You need to declare this symbol by `let` first").with_color(Color::Red))
            }
        }
        report.finish()
    }
}
