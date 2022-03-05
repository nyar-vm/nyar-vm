use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use crate::{NyarError, NyarErrorKind};

impl Error for NyarError {}

impl Debug for NyarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            NyarErrorKind::Duplicate(e) => Debug::fmt(e, f),
            NyarErrorKind::Runtime(e) => Debug::fmt(e, f),
            NyarErrorKind::Parsing(e) => Debug::fmt(e, f),
        }
    }
}

impl Display for NyarError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            NyarErrorKind::Duplicate(e) => Display::fmt(e, f),
            NyarErrorKind::Runtime(e) => Display::fmt(e, f),
            NyarErrorKind::Parsing(e) => Display::fmt(e, f),
        }
    }
}
