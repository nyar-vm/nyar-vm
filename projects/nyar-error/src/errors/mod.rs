use diagnostic::{Diagnostic, ReportKind};

use crate::{parsing::SyntaxError, DuplicateError, RuntimeError};

pub mod display;

pub type NyarResult<T = ()> = Result<T, NyarError>;

#[derive(Clone)]
pub struct NyarError {
    kind: Box<NyarErrorKind>,
    level: ReportKind,
}

#[derive(Clone)]
pub enum NyarErrorKind {
    Duplicate(DuplicateError),
    Runtime(RuntimeError),
    Parsing(SyntaxError),
}

impl NyarError {
    pub fn set_file(&mut self, file: FileID) {
        match self.kind.as_mut() {
            NyarErrorKind::Duplicate(_) => {}
            NyarErrorKind::Runtime(_) => {}
            NyarErrorKind::Parsing(s) => {
                s.span.file = file;
            }
        }
    }

    pub fn as_report(&self) -> Diagnostic {
        match self.kind.as_ref() {
            NyarErrorKind::Duplicate(e) => e.as_report(self.level),
            NyarErrorKind::Runtime(e) => e.as_report(self.level),
            NyarErrorKind::Parsing(e) => e.as_report(self.level),
        }
    }
}
#[allow(clippy::wrong_self_convention)]
impl NyarErrorKind {
    pub fn as_error(self, level: ReportKind) -> NyarError {
        NyarError { kind: Box::new(self), level }
    }
}
