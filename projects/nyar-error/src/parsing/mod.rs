use diagnostic::{Color, Diagnostic, FileID, FileSpan, ReportKind};
use std::{
    char::ParseCharError,
    fmt::{Display, Formatter},
    num::{ParseFloatError, ParseIntError},
    ops::Range,
    str::ParseBoolError,
};

use crate::{NyarError, NyarErrorKind};

#[cfg(feature = "dashu")]
mod for_dashu;
#[cfg(feature = "json5")]
mod for_json5;
#[cfg(feature = "num")]
mod for_num;
#[cfg(feature = "peginator")]
mod for_peginator;
#[cfg(feature = "toml")]
mod for_toml;

#[cfg(feature = "pratt")]
mod for_pratt;

#[derive(Clone, Debug)]
pub struct SyntaxError {
    pub info: String,
    pub span: FileSpan,
}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.info)
    }
}

impl SyntaxError {
    pub fn new(info: impl Into<String>) -> Self {
        Self { span: FileSpan::default(), info: info.into() }
    }
    pub fn with_file(mut self, file: FileID) -> Self {
        self.span.set_file(file);
        self
    }
    pub fn with_range(mut self, range: &Range<usize>) -> Self {
        self.span.set_range(range.clone());
        self
    }
    pub fn with_span(mut self, span: FileSpan) -> Self {
        self.span = span;
        self
    }
    pub fn as_report(&self, kind: ReportKind) -> Diagnostic {
        let mut report = Diagnostic::new(kind, self.span.get_file(), self.span.get_start());
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

wrap_parse_error!(ParseIntError, ParseFloatError, ParseBoolError, ParseCharError, url::ParseError);

#[cfg(feature = "num")]
wrap_parse_error!(num::bigint::ParseBigIntError);

#[cfg(feature = "dashu")]
wrap_parse_error!(dashu::base::ParseError);
