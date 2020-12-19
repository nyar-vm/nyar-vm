use super::*;
use std::num::{IntErrorKind, ParseIntError};

#[derive(Clone, Debug)]
pub enum ParseIntegerError {
    /// Value being parsed is empty.
    ///
    /// Among other causes, this variant will be constructed when parsing an empty string.
    Empty,
    /// Contains an invalid digit in its context.
    ///
    /// Among other causes, this variant will be constructed when parsing a string that
    /// contains a non-ASCII char.
    ///
    /// This variant is also constructed when a `+` or `-` is misplaced within a string
    /// either on its own or in the middle of a number.
    InvalidDigit,
    /// Integer is too large to store in target integer type.
    PositiveOverflow,
    /// Integer is too small to store in target integer type.
    NegativeOverflow,
    /// Value was Zero
    ///
    /// This variant will be emitted when the parsing string has a value of zero, which
    /// would be illegal for non-zero types.
    Zero,
    HandlerNotFound(String),
}

impl From<ParseIntError> for NyarError {
    fn from(e: ParseIntError) -> Self {
        let kind = match e.kind() {
            IntErrorKind::Empty => ParseIntegerError::Empty,
            IntErrorKind::InvalidDigit => ParseIntegerError::InvalidDigit,
            IntErrorKind::PosOverflow => ParseIntegerError::PositiveOverflow,
            IntErrorKind::NegOverflow => ParseIntegerError::NegativeOverflow,
            _ => ParseIntegerError::Zero,
        };
        NyarError { kind: box NyarErrorKind::ParseIntegerError { kind }, span: Default::default() }
    }
}
