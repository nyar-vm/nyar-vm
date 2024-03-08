use diagnostic::{Color, Label, ReportKind, SourceSpan};
use std::fmt::{Debug, Display, Formatter};

use crate::{Diagnostic, NyarError, NyarErrorKind};

mod kind;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DuplicateKind {
    Type = 1002,
    Function = 1003,
    Variable = 1004,
    Key = 1005,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateError {
    kind: DuplicateKind,
    name: String,
    this_item: SourceSpan,
    last_item: SourceSpan,
}

impl Display for DuplicateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Duplicate {} `{}`", self.kind, self.name)
    }
}

impl DuplicateError {
    pub fn as_report(&self, level: ReportKind) -> Diagnostic {
        let mut report = Diagnostic::new(level).with_code(self.kind as usize);
        report.set_message(self.to_string());
        report.add_label(
            Label::new(self.this_item)
                .with_message(format!("{:?} `{}` is defined here.", self.kind, self.name))
                .with_color(Color::Blue),
        );
        report.add_label(
            Label::new(self.last_item)
                .with_message(format!("{} `{}` is defined here.", self.kind, self.name))
                .with_color(Color::Cyan),
        );
        report.set_help(format!("Items must have unique names, rename one of the items to have a unique name"));
        report.finish()
    }
}

impl DuplicateError {
    pub fn duplicate_type(name: String, this: SourceSpan, last: SourceSpan) -> Self {
        DuplicateError { kind: DuplicateKind::Type, name, this_item: this, last_item: last }
    }
}

impl NyarError {
    pub fn duplicate_type(name: String, this: SourceSpan, last: SourceSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Type, name, this_item: this, last_item: last };
        NyarErrorKind::Duplicate(this).as_error(ReportKind::Error)
    }
    pub fn duplicate_function(name: String, this: SourceSpan, last: SourceSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Function, name, this_item: this, last_item: last };
        NyarErrorKind::Duplicate(this).as_error(ReportKind::Error)
    }
    pub fn duplicate_variable(name: String, this: SourceSpan, last: SourceSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Variable, name, this_item: this, last_item: last };
        NyarErrorKind::Duplicate(this).as_error(ReportKind::Error)
    }
    pub fn duplicate_key(name: String, this: SourceSpan, last: SourceSpan) -> Self {
        let this = DuplicateError { kind: DuplicateKind::Key, name, this_item: this, last_item: last };
        NyarErrorKind::Duplicate(this).as_error(ReportKind::Error)
    }
}
