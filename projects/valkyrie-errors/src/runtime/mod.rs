use std::{
    fmt::{Display, Formatter},
    panic::Location,
};

use ariadne::ReportKind;

use crate::{errors::ValkyrieReport, FileSpan, SyntaxError, ValkyrieError, ValkyrieErrorKind};

#[derive(Clone, Debug)]
pub struct RuntimeError {
    message: String,
}

impl From<RuntimeError> for ValkyrieError {
    fn from(value: RuntimeError) -> Self {
        ValkyrieError { kind: ValkyrieErrorKind::Runtime(Box::new(value)), level: ReportKind::Error }
    }
}

impl From<()> for ValkyrieError {
    #[track_caller]
    fn from(_: ()) -> Self {
        let caller = Location::caller();
        RuntimeError { message: caller.to_string() }.into()
    }
}

impl From<std::io::Error> for ValkyrieError {
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
    pub fn as_report(&self, kind: ReportKind) -> ValkyrieReport {
        let mut report = ValkyrieReport::build(kind, 0usize, 0);
        report.set_message(self.to_string());
        report.finish()
    }
}

impl ValkyrieError {
    pub fn syntax_error(message: impl Into<String>, position: FileSpan) -> Self {
        let this = SyntaxError { info: message.into(), span: position };
        Self { kind: ValkyrieErrorKind::Parsing(Box::new(this)), level: ReportKind::Error }
    }

    pub fn runtime_error(message: impl Into<String>) -> Self {
        let this = RuntimeError { message: message.into() };
        Self { kind: ValkyrieErrorKind::Runtime(Box::new(this)), level: ReportKind::Error }
    }
}
