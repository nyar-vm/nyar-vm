use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use crate::{ValkyrieError, ValkyrieErrorKind};

impl Error for ValkyrieError {}

impl Debug for ValkyrieError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ValkyrieErrorKind::Duplicate(e) => Debug::fmt(e, f),
            ValkyrieErrorKind::Runtime(e) => Debug::fmt(e, f),
            ValkyrieErrorKind::Parsing(e) => Debug::fmt(e, f),
        }
    }
}

impl Display for ValkyrieError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ValkyrieErrorKind::Duplicate(e) => Display::fmt(e, f),
            ValkyrieErrorKind::Runtime(e) => Display::fmt(e, f),
            ValkyrieErrorKind::Parsing(e) => Display::fmt(e, f),
        }
    }
}
