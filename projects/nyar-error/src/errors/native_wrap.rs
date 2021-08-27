use crate::NyarError;

use super::*;

impl From<std::io::Error> for NyarError3 {
    fn from(e: std::io::Error) -> Self {
        Self { kind: box NyarErrorKind::IOError(e), span: Default::default() }
    }
}
impl From<std::fmt::Error> for NyarError3 {
    fn from(e: std::fmt::Error) -> Self {
        Self { kind: box NyarErrorKind::FormatError(e), span: Default::default() }
    }
}

impl From<std::io::Error> for NyarError {
    fn from(e: std::io::Error) -> Self {
        NyarError::IoError(box e)
    }
}
