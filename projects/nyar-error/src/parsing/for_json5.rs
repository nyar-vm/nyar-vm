use crate::NyarError;
use json5::Error;

impl From<Error> for NyarError {
    fn from(value: Error) -> Self {
        // can't get the span from the error
        NyarError::runtime_error(value.to_string())
    }
}
