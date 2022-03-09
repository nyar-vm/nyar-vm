use diagnostic::{Diagnostic, FileID, ReportKind};

use crate::{parsing::SyntaxError, DuplicateError, RuntimeError};

pub mod display;

pub type Validation<T> = validatus::Validation<T, NyarError>;
pub type Result<T = ()> = core::result::Result<T, NyarError>;

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
    Custom(String),
}

impl NyarError {
    pub fn set_file(&mut self, file: FileID) {
        match self.kind.as_mut() {
            NyarErrorKind::Duplicate(_) => {}
            NyarErrorKind::Runtime(_) => {}
            NyarErrorKind::Parsing(s) => s.span.set_file(file),
            NyarErrorKind::Custom(_) => {}
        }
    }

    pub fn as_report(&self) -> Diagnostic {
        match self.kind.as_ref() {
            NyarErrorKind::Duplicate(e) => e.as_report(self.level),
            NyarErrorKind::Runtime(e) => e.as_report(self.level),
            NyarErrorKind::Parsing(e) => e.as_report(self.level),
            NyarErrorKind::Custom(e) => Diagnostic::new(self.level, unsafe { FileID::new(0) }, 0).with_message(e).finish(),
        }
    }
}
#[allow(clippy::wrong_self_convention)]
impl NyarErrorKind {
    pub fn as_error(self, level: ReportKind) -> NyarError {
        NyarError { kind: Box::new(self), level }
    }
}
