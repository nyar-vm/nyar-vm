use std::ops::Range;

use ariadne::{Report, ReportKind};

use crate::{parsing::SyntaxError, DuplicateError, FileID, RuntimeError};

pub mod display;

pub type ValkyrieReport = Report<(FileID, Range<usize>)>;

pub type ValkyrieResult<T = ()> = Result<T, ValkyrieError>;

#[derive(Clone)]
pub struct ValkyrieError {
    pub kind: ValkyrieErrorKind,
    pub level: ReportKind,
}

#[derive(Clone)]
pub enum ValkyrieErrorKind {
    Duplicate(Box<DuplicateError>),
    Runtime(Box<RuntimeError>),
    Parsing(Box<SyntaxError>),
}

impl ValkyrieError {
    pub fn as_report(&self) -> ValkyrieReport {
        match &self.kind {
            ValkyrieErrorKind::Duplicate(e) => e.as_report(self.level),
            ValkyrieErrorKind::Runtime(e) => e.as_report(self.level),
            ValkyrieErrorKind::Parsing(e) => e.as_report(self.level),
        }
    }
}
