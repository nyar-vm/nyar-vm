use crate::{NyarError, SyntaxError};
#[cfg(feature = "bigdecimal")]
use bigdecimal::ParseBigDecimalError;
#[cfg(feature = "num")]
use num::{bigint::ParseBigIntError, rational::ParseRatioError};
use std::num::{ParseFloatError, ParseIntError};

impl From<ParseIntError> for SyntaxError {
    fn from(error: ParseIntError) -> Self {
        Self { info: error.to_string(), hint: "".to_string(), span: Default::default() }
    }
}

impl From<ParseIntError> for NyarError {
    fn from(error: ParseIntError) -> Self {
        SyntaxError::from(error).into()
    }
}
#[cfg(feature = "num")]
impl From<ParseBigIntError> for SyntaxError {
    fn from(error: ParseBigIntError) -> Self {
        Self { info: error.to_string(), hint: "".to_string(), span: Default::default() }
    }
}

#[cfg(feature = "num")]
impl From<ParseBigIntError> for NyarError {
    fn from(error: ParseBigIntError) -> Self {
        SyntaxError::from(error).into()
    }
}

#[cfg(feature = "num")]
impl From<ParseRatioError> for NyarError {
    fn from(error: ParseRatioError) -> Self {
        SyntaxError { info: error.to_string(), hint: "".to_string(), span: Default::default() }.into()
    }
}
impl From<ParseFloatError> for NyarError {
    fn from(error: ParseFloatError) -> Self {
        SyntaxError { info: error.to_string(), hint: "".to_string(), span: Default::default() }.into()
    }
}
#[cfg(feature = "bigdecimal")]
impl From<ParseBigDecimalError> for NyarError {
    fn from(error: ParseBigDecimalError) -> Self {
        SyntaxError { info: error.to_string(), hint: "".to_string(), span: Default::default() }.into()
    }
}
