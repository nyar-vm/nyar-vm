use dashu::base::{ConversionError, ParseError};

use crate::{RuntimeError, SyntaxError};

impl From<ParseError> for SyntaxError {
    fn from(value: ParseError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<ConversionError> for RuntimeError {
    fn from(value: ConversionError) -> Self {
        Self::new(value.to_string())
    }
}

