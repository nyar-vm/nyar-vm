use super::*;
use crate::parsing::ForeignInterfaceError;

impl From<ForeignInterfaceError> for NyarError {
    fn from(value: ForeignInterfaceError) -> Self {
        Self { kind: Box::new(NyarErrorKind::Parsing(value.into())), level: value.into() }
    }
}

impl Into<SyntaxError> for ForeignInterfaceError {
    fn into(self) -> SyntaxError {
        match self {
            Self::MissingForeignMark { span } => {
                SyntaxError::new("foreign mark not found".to_string()).with_hint("missing hint 3").with_span(span)
            }
            Self::MissingForeignFlag { kind, hint, span } => SyntaxError::new(format!("foreign {kind} must mark as `{hint}`"))
                .with_hint(format!("add `{hint}` modifier before declaration"))
                .with_span(span),
            Self::InvalidForeignModule { span } => {
                SyntaxError::new("foreign module not found".to_string()).with_hint("missing hint 1").with_span(span)
            }
            Self::InvalidForeignName { span } => {
                SyntaxError::new("foreign name not found".to_string()).with_hint("missing hint 2").with_span(span)
            }
        }
    }
}

impl Into<ReportKind> for ForeignInterfaceError {
    fn into(self) -> ReportKind {
        match self {
            Self::MissingForeignMark { .. } => ReportKind::Trace,
            Self::MissingForeignFlag { .. } => ReportKind::Alert,
            Self::InvalidForeignModule { .. } => ReportKind::Error,
            Self::InvalidForeignName { .. } => ReportKind::Error,
        }
    }
}
