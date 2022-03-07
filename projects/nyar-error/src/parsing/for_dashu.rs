use dashu::base::{ConversionError, ParseError};

use crate::{RuntimeError, SyntaxError};

impl From<ParseError> for SyntaxError {
    fn from(value: ParseError) -> Self {
        match value {
            ParseError::NoDigits => Self::new("No digits found in number"),
            ParseError::InvalidDigit => Self::new("Invalid digit found in number"),
            ParseError::UnsupportedRadix => Self::new("Unsupported radix"),
            ParseError::InconsistentRadix => Self::new("Inconsistent radix"),
        }
    }
}

impl From<ConversionError> for RuntimeError {
    fn from(value: ConversionError) -> Self {
        Self::new(value.to_string())
    }
}
