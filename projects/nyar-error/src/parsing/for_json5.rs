use crate::ValkyrieError;
use json5::Error;

impl From<Error> for ValkyrieError {
    fn from(value: Error) -> Self {
        // can't get the span from the error
        ValkyrieError::runtime_error(value.to_string())
    }
}
