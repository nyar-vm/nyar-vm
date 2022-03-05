use std::fmt::{Debug, Display, Formatter};

use ariadne::{Color, Report, ReportKind};

use crate::{errors::ValkyrieReport, FileSpan, NyarError, NyarErrorKind};

mod kind;

#[derive(Copy, Clone)]
pub enum DuplicateKind {
    Type = 1002,
    Function = 1003,
    Variable = 1004,
}

#[derive(Clone, Debug)]
pub struct DuplicateError {
    kind: DuplicateKind,
    name: String,
    this_item: FileSpan,
    last_item: FileSpan,
}

impl Display for DuplicateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Duplicate {} `{}`", self.kind, self.name)
    }
}

impl DuplicateError {
    pub fn as_report(&self, level: ReportKind) -> ValkyrieReport {
        let mut report = Report::build(level, self.this_item.file, 0).with_code(self.kind as u32);
        report.set_message(self.to_string());
        report.add_label(
            self.this_item.as_label(format!("{:?} `{}` is defined here.", self.kind, self.name)).with_color(Color::Blue),
        );
        report.add_label(
            self.last_item
                .as_label(format!("But {} `{}` has been defined here.", self.kind, self.name))
                .with_color(Color::Cyan),
        );
        report.set_help(format!("Items must have unique names, rename one of the items to have a unique name"));
        report.finish()
    }
}

impl NyarError {
    pub fn duplicate_type(name: String, this: FileSpan, last: FileSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Type, name, this_item: this, last_item: last };
        Self { kind: NyarErrorKind::Duplicate(Box::new(this)), level: ReportKind::Error }
    }
    pub fn duplicate_function(name: String, this: FileSpan, last: FileSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Function, name, this_item: this, last_item: last };
        Self { kind: NyarErrorKind::Duplicate(Box::new(this)), level: ReportKind::Error }
    }
    pub fn duplicate_variable(name: String, this: FileSpan, last: FileSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Variable, name, this_item: this, last_item: last };
        Self { kind: NyarErrorKind::Duplicate(Box::new(this)), level: ReportKind::Error }
    }
}
