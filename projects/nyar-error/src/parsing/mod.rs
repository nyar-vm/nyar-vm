use diagnostic::{Color, Diagnostic, FileID, FileSpan, ReportKind};
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

#[derive(Clone, Debug)]
pub struct SyntaxError {
    pub info: String,
    pub except: String,
    pub span: FileSpan,
}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.info)
    }
}

impl SyntaxError {
    pub fn new(info: impl Into<String>) -> Self {
        Self { span: FileSpan::default(), info: info.into(), except: "".to_string() }
    }
    pub fn invalid_integer<S: ToString>(message: S) -> Self {
        Self { info: message.to_string(), except: "Integer".to_string(), span: Default::default() }
    }
    pub fn invalid_decimal<S: ToString>(message: S) -> Self {
        Self { info: message.to_string(), except: "Decimal".to_string(), span: Default::default() }
    }
    pub fn with_file(mut self, file: FileID) -> Self {
        self.span.set_file(file);
        self
    }
    pub fn with_range(mut self, range: &Range<u32>) -> Self {
        self.span.set_range(Range { start: range.start as usize, end: range.end as usize });
        self
    }
    pub fn with_span(mut self, span: FileSpan) -> Self {
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
        report.set_message(self.to_string());
        report.add_label(self.span.as_label(self.to_string()).with_color(Color::Red));
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
