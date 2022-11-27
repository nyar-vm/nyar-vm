use crate::NyarError;
use walkdir::Error;

impl From<walkdir::Error> for NyarError {
    fn from(value: Error) -> Self {
        crate::errors::NyarError::runtime_error(value.to_string())
    }
}
