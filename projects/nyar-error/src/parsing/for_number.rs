use crate::{NyarError, SyntaxError};
#[cfg(feature = "bigdecimal")]
use bigdecimal::ParseBigDecimalError;
#[cfg(feature = "num")]
use num::{bigint::ParseBigIntError, rational::ParseRatioError};
use std::num::{ParseFloatError, ParseIntError};

impl From<ParseIntError> for NyarError {
    fn from(value: ParseIntError) -> Self {
        SyntaxError::invalid_integer(value).into()
    }
}

#[cfg(feature = "num")]
impl From<ParseBigIntError> for NyarError {
    fn from(value: ParseBigIntError) -> Self {
        SyntaxError::invalid_integer(value).into()
    }
}
#[cfg(feature = "num")]
impl From<ParseRatioError> for NyarError {
    fn from(value: ParseRatioError) -> Self {
        SyntaxError::invalid_decimal(value).into()
    }
}
impl From<ParseFloatError> for NyarError {
    fn from(value: ParseFloatError) -> Self {
        SyntaxError::invalid_decimal(value).into()
    }
}
#[cfg(feature = "bigdecimal")]
impl From<ParseBigDecimalError> for NyarError {
    fn from(value: ParseBigDecimalError) -> Self {
        SyntaxError::invalid_decimal(value).into()
    }
}
