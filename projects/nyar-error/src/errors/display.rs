use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use crate::{NyarError, NyarErrorKind};

impl Error for NyarError {}

impl Error for NyarErrorKind {}
impl Debug for NyarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.kind.as_ref(), f)
    }
}
impl Debug for NyarErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(e) => Debug::fmt(e, f),
            Self::Runtime(e) => Debug::fmt(e, f),
            Self::Parsing(e) => Debug::fmt(e, f),
            Self::Custom(e) => Debug::fmt(e, f),
        }
    }
}

impl Display for NyarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.kind.as_ref(), f)
    }
}

impl Display for NyarErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(e) => Display::fmt(e, f),
            Self::Runtime(e) => Display::fmt(e, f),
            Self::Parsing(e) => Display::fmt(e, f),
            Self::Custom(e) => Display::fmt(e, f),
        }
    }
}
