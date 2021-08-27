use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use miette::{Diagnostic, LabeledSpan, Severity, SourceCode};

use crate::NyarError2;

impl Error for NyarError2 {}

impl Debug for NyarError2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarError2::IoError(e) => Debug::fmt(e, f),
            NyarError2::ParseError(e) => Debug::fmt(e, f),
        }
    }
}

impl Display for NyarError2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarError2::IoError(e) => Display::fmt(e, f),
            NyarError2::ParseError(e) => Display::fmt(e, f),
        }
    }
}

impl Diagnostic for NyarError2 {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        let out = match self {
            NyarError2::IoError(e) => box "E1001",
            NyarError2::ParseError(e) => box "E9000",
        };
        Some(out)
    }
    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match self {
            NyarError2::IoError(e) => Some(box "E1001"),
            NyarError2::ParseError(e) => Some(box &e.message),
        }
    }
    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        let out = match self {
            NyarError2::IoError(e) => box "E1001",
            NyarError2::ParseError(e) => box "https://docs.rs/E9000",
        };
        Some(out)
    }
    fn severity(&self) -> Option<Severity> {
        let out = match self {
            NyarError2::IoError(e) => Severity::Error,
            NyarError2::ParseError(e) => Severity::Error,
        };
        Some(out)
    }
    fn source_code(&self) -> Option<&dyn SourceCode> {
        let out = match self {
            NyarError2::IoError(e) => return None,
            NyarError2::ParseError(e) => &e.file,
        };
        Some(out)
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let out = match self {
            NyarError2::IoError(e) => return None,
            NyarError2::ParseError(e) => box e.label().into_iter(),
        };
        Some(out)
    }
}
