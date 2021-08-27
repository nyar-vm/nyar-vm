use crate::NyarError2;

use super::*;

impl From<std::io::Error> for NyarError {
    fn from(e: std::io::Error) -> Self {
        Self { kind: box NyarErrorKind::IOError(e), span: Default::default() }
    }
}
impl From<std::fmt::Error> for NyarError {
    fn from(e: std::fmt::Error) -> Self {
        Self { kind: box NyarErrorKind::FormatError(e), span: Default::default() }
    }
}

impl From<std::io::Error> for NyarError2 {
    fn from(e: std::io::Error) -> Self {
        NyarError2::IoError(box e)
    }
}
impl From<()> for NyarError2 {
    fn from(e: ()) -> Self {
        NyarError2::IoError(box e)
    }
}
