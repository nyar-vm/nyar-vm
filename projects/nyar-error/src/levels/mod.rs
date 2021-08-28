use std::{
    any::type_name_of_val,
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use miette::{Diagnostic, LabeledSpan, Severity, SourceCode};

use crate::NyarError;

impl Error for NyarError {}

impl Debug for NyarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarError::IoError(e) => Debug::fmt(e, f),
            NyarError::ParseError(e) => Debug::fmt(e, f),
            NyarError::InternalError(e) => Debug::fmt(e, f),
        }
    }
}

impl Display for NyarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarError::IoError(e) => Display::fmt(e, f),
            NyarError::ParseError(e) => Display::fmt(e, f),
            NyarError::InternalError(e) => Display::fmt(e, f),
        }
    }
}

impl Diagnostic for NyarError {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match self {
            NyarError::IoError(e) => Some(box type_name_of_val(e)),
            NyarError::ParseError(e) => Some(box type_name_of_val(e)),
            NyarError::InternalError(e) => Some(box type_name_of_val(e)),
        }
    }
    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match self {
            NyarError::IoError(_) => Some(box "E1001"),
            NyarError::ParseError(e) => e.help(),
            NyarError::InternalError(e) => e.help(),
        }
    }
    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match self {
            NyarError::IoError(_) => Some(box "E1001"),
            NyarError::ParseError(e) => e.url(),
            NyarError::InternalError(e) => e.url(),
        }
    }
    fn severity(&self) -> Option<Severity> {
        let out = match self {
            NyarError::IoError(_) => Severity::Error,
            NyarError::ParseError(_) => Severity::Error,
            NyarError::InternalError(_) => Severity::Error,
        };
        Some(out)
    }
    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            NyarError::IoError(_) => return None,
            NyarError::ParseError(e) => e.source_code(),
            NyarError::InternalError(e) => e.source_code(),
        }
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match self {
            NyarError::IoError(_) => return None,
            NyarError::ParseError(e) => e.labels(),
            NyarError::InternalError(e) => e.labels(),
        }
    }
}
