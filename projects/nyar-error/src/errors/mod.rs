use std::ops::Range;
use syntax_error::{Report, ReportKind};

use crate::{parsing::SyntaxError, DuplicateError, FileID, RuntimeError};

pub mod display;

pub type ValkyrieReport = Report<(FileID, Range<usize>)>;

pub type ValkyrieResult<T = ()> = Result<T, NyarError>;

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
        match &mut self.kind {
            NyarErrorKind::Duplicate(_) => {}
            NyarErrorKind::Runtime(_) => {}
            NyarErrorKind::Parsing(s) => {
                s.span.file = file;
            }
        }
    }

    pub fn as_report(&self) -> ValkyrieReport {
        match &self.kind {
            NyarErrorKind::Duplicate(e) => e.as_report(self.level),
            NyarErrorKind::Runtime(e) => e.as_report(self.level),
            NyarErrorKind::Parsing(e) => e.as_report(self.level),
        }
    }
}
