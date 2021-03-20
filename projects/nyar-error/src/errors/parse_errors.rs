use super::*;
use std::{char::ParseCharError, num::ParseIntError};

impl From<ParseIntError> for NyarError {
    fn from(e: ParseIntError) -> Self {
        NyarError::syntax_error(e.to_string())
    }
}
impl From<ParseCharError> for NyarError {
    fn from(e: ParseCharError) -> Self {
        NyarError::syntax_error(e.to_string())
    }
}
