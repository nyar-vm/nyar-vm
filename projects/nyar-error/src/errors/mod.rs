use diagnostic::{Diagnostic, ReportKind, SourceID};

use crate::{parsing::SyntaxError, DuplicateError, MissingError, RuntimeError, SourceSpan};

mod convert;
pub mod display;

pub type Validation<T> = validatus::Validation<T, NyarError>;
pub type Result<T = ()> = core::result::Result<T, NyarError>;

#[derive(Clone, Eq, PartialEq)]
pub struct NyarError {
    kind: Box<NyarErrorKind>,
    level: ReportKind,
}

#[derive(Clone, Eq, PartialEq)]
pub enum NyarErrorKind {
    Missing(MissingError),
    Duplicate(DuplicateError),
    Runtime(RuntimeError),
    Parsing(SyntaxError),
    Custom(String),
}

impl NyarError {
    pub fn set_file(&mut self, file: SourceID) {
        match self.kind.as_mut() {
            NyarErrorKind::Duplicate(_) => {}
            NyarErrorKind::Runtime(_) => {}
            NyarErrorKind::Parsing(s) => s.span.set_file(file),
            NyarErrorKind::Custom(_) => {}
            NyarErrorKind::Missing(s) => s.span.set_file(file),
        }
    }
    pub fn with_file(mut self, file: SourceID) -> Self {
        self.set_file(file);
        self
    }

    pub fn set_span(&mut self, span: SourceSpan) {
        match self.kind.as_mut() {
            NyarErrorKind::Duplicate(_) => {}
            NyarErrorKind::Runtime(_) => {}
            NyarErrorKind::Parsing(s) => s.span = span,
            NyarErrorKind::Custom(_) => {}
            NyarErrorKind::Missing(s) => s.span = span,
        }
    }
    pub fn with_span(mut self, file: SourceSpan) -> Self {
        self.set_span(file);
        self
    }

    pub fn as_report(&self) -> Diagnostic {
        match self.kind.as_ref() {
            NyarErrorKind::Missing(e) => e.as_report(self.level),
            NyarErrorKind::Duplicate(e) => e.as_report(self.level),
            NyarErrorKind::Runtime(e) => e.as_report(self.level),
            NyarErrorKind::Parsing(e) => e.as_report(self.level),
            NyarErrorKind::Custom(e) => Diagnostic::new(self.level).with_message(e).finish(),
        }
    }
}

#[allow(clippy::wrong_self_convention)]
impl NyarErrorKind {
    pub fn as_error(self, level: ReportKind) -> NyarError {
        NyarError { kind: Box::new(self), level }
    }
}
