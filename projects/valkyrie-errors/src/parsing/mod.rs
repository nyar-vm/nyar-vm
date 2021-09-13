use std::{
    char::ParseCharError,
    fmt::{Display, Formatter},
    num::{ParseFloatError, ParseIntError},
    ops::Range,
    str::ParseBoolError,
};

use ariadne::{Color, ReportKind};

use crate::{FileID, FileSpan, ValkyrieError, ValkyrieErrorKind, ValkyrieReport};

#[cfg(feature = "num")]
mod for_num;
#[cfg(feature = "dashu")]
mod for_dashu;
#[cfg(feature = "peginator")]
mod for_peginator;

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
        self.span.file = file;
        self
    }
    pub fn with_range(mut self, range: &Range<usize>) -> Self {
        self.span.head = range.start;
        self.span.tail = range.end;
        self
    }
    pub fn with_span(mut self, span: FileSpan) -> Self {
        self.span = span;
        self
    }
    pub fn as_report(&self, kind: ReportKind) -> ValkyrieReport {
        let mut report = ValkyrieReport::build(kind, self.span.file, self.span.head);
        report.set_message(self.to_string());
        report.add_label(self.span.as_label(self.to_string()).with_color(Color::Red));
        report.finish()
    }
}

impl From<SyntaxError> for ValkyrieError {
    fn from(value: SyntaxError) -> Self {
        ValkyrieError { kind: ValkyrieErrorKind::Parsing(Box::new(value)), level: ReportKind::Error }
    }
}

macro_rules! wrap_parse_error {
    ($($type:ty),*) => {
        $(
            impl From<$type> for ValkyrieError {
                fn from(value: $type) -> Self {
                    SyntaxError::new(value.to_string()).into()
                }
            }
        )*
    };
}

wrap_parse_error!(ParseIntError, ParseFloatError, ParseBoolError, ParseCharError, url::ParseError);

#[cfg(feature = "peginator")]
wrap_parse_error!(peginator::ParseError);

#[cfg(feature = "num")]
wrap_parse_error!(num::bigint::ParseBigIntError);

// #[cfg(feature = "dashu")]
// wrap_parse_error!(num::bigint::ParseBigIntError);