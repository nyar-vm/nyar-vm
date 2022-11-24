use crate::{NyarError, NyarErrorKind};
use diagnostic::{Color, Diagnostic, Label, ReportKind, SourceID, SourceSpan};
use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
    ops::Range,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingError {
    pub(crate) kind: MissingErrorKind,
    pub(crate) span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MissingErrorKind {
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
        Self { kind: MissingErrorKind::EmptyPath, span: SourceSpan::default() }
    }
    pub fn undefined(symbol: &str) -> Self {
        Self { kind: MissingErrorKind::Undefined(Box::from(symbol)), span: SourceSpan::default() }
    }
    pub fn with_span(self, span: SourceSpan) -> Self {
        Self { span, ..self }
    }
    pub fn with_range(self, range: Range<u32>) -> Self {
        Self { span: self.span.with_range(range), ..self }
    }
    pub fn with_file(self, file: SourceID) -> Self {
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
                report.add_label(Label::new(self.span).with_message("Symbol path cannot be empty").with_color(Color::Red))
            }
            MissingErrorKind::Undefined(_) => report.add_label(
                Label::new(self.span).with_message("You need to declare this symbol by `let` first").with_color(Color::Red),
            ),
        }
        report.finish()
    }
}
