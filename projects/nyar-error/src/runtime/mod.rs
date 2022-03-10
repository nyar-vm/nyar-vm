use diagnostic::{FileSpan, ReportKind};
use std::{
    fmt::{Display, Formatter},
    panic::Location,
};

use crate::{Diagnostic, NyarError, NyarErrorKind, SyntaxError};

mod for_serde;

#[derive(Clone, Debug)]
pub struct RuntimeError {
    message: String,
}

impl From<RuntimeError> for NyarError {
    fn from(value: RuntimeError) -> Self {
        NyarErrorKind::Runtime(value).as_error(ReportKind::Error)
    }
}

impl From<()> for NyarError {
    #[track_caller]
    fn from(_: ()) -> Self {
        let caller = Location::caller();
        RuntimeError { message: caller.to_string() }.into()
    }
}

impl From<std::io::Error> for NyarError {
    fn from(value: std::io::Error) -> Self {
        RuntimeError { message: value.to_string() }.into()
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl RuntimeError {
    pub fn new(message: impl Display) -> Self {
        Self { message: message.to_string() }
    }
    pub fn as_report(&self, kind: ReportKind) -> Diagnostic {
        let mut report = Diagnostic::new(kind);
        report.set_message(self.to_string());
        report.finish()
    }
}

impl NyarError {
    pub fn syntax_error(message: impl Into<String>, position: FileSpan) -> Self {
        let this = SyntaxError { info: message.into(), hint: "".to_string(), span: position };
        NyarErrorKind::Parsing(this).as_error(ReportKind::Error)
    }

    pub fn runtime_error(message: impl Into<String>) -> Self {
        let this = RuntimeError { message: message.into() };
        NyarErrorKind::Runtime(this).as_error(ReportKind::Error)
    }

    pub fn custom<S: ToString>(message: S) -> Self {
        let this = message.to_string();
        NyarErrorKind::Custom(this).as_error(ReportKind::Error)
    }
}
