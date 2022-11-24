use diagnostic::{Color, Diagnostic, Label, ReportKind, SourceID, SourceSpan};
use std::{
    char::ParseCharError,
    fmt::{Display, Formatter},
    ops::Range,
    str::ParseBoolError,
};

use crate::{NyarError, NyarErrorKind};

#[cfg(feature = "dashu")]
mod for_dashu;
#[cfg(feature = "json5")]
mod for_json5;

mod for_number;
#[cfg(feature = "peginator")]
mod for_peginator;
#[cfg(feature = "toml")]
mod for_toml;

#[cfg(feature = "pratt")]
mod for_pratt;
#[cfg(feature = "yggdrasil-rt")]
mod for_ygg;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxError {
    pub info: String,
    pub hint: String,
    pub span: SourceSpan,
}

#[derive(Debug, Copy, Clone)]
pub enum ForeignInterfaceError {
    MissingForeignMark { span: SourceSpan },
    MissingForeignFlag { kind: &'static str, hint: &'static str, span: SourceSpan },
    InvalidForeignModule { span: SourceSpan },
    InvalidForeignName { span: SourceSpan },
}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.info)
    }
}

impl SyntaxError {
    /// Create a new syntax error with given message
    pub fn new(info: impl Into<String>) -> Self {
        Self { span: SourceSpan::default(), info: info.into(), hint: "".to_string() }
    }
    /// Set the excepted hint
    pub fn with_hint<S: ToString>(mut self, hint: S) -> Self {
        self.hint = hint.to_string();
        self
    }
    /// Set the file id
    pub fn with_file(mut self, file: SourceID) -> Self {
        self.span.set_file(file);
        self
    }
    /// Set the file range
    pub fn with_range(mut self, range: &Range<u32>) -> Self {
        self.span.set_range(range.clone());
        self
    }
    /// Set the file span
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = span;
        self
    }
}

impl SyntaxError {
    pub fn as_error(self, kind: ReportKind) -> NyarError {
        NyarErrorKind::Parsing(self).as_error(kind)
    }
    pub fn as_report(&self, kind: ReportKind) -> Diagnostic {
        let mut report = Diagnostic::new(kind).with_location(self.span.get_file(), Some(self.span.get_start()));
        report.set_message(&self.info);
        report.add_label(Label::new(self.span).with_message(&self.hint).with_color(Color::Red));
        report.finish()
    }
}

impl From<SyntaxError> for NyarError {
    fn from(value: SyntaxError) -> Self {
        NyarErrorKind::Parsing(value).as_error(ReportKind::Error)
    }
}

macro_rules! wrap_parse_error {
    ($($type:ty),*) => {
        $(
            impl From<$type> for NyarError {
                fn from(value: $type) -> Self {
                    SyntaxError::new(value.to_string()).into()
                }
            }
        )*
    };
}

wrap_parse_error!(ParseBoolError, ParseCharError, url::ParseError);

#[cfg(feature = "dashu")]
wrap_parse_error!(dashu::base::ParseError);
