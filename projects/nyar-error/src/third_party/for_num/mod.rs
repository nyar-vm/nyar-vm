use crate::NyarError;
use bigdecimal::ParseBigDecimalError;
use num::bigint::ParseBigIntError;

impl From<ParseBigIntError> for NyarError {
    fn from(e: ParseBigIntError) -> Self {
        NyarError::syntax_error(e.to_string())
    }
}

impl From<ParseBigDecimalError> for NyarError {
    fn from(e: ParseBigDecimalError) -> Self {
        NyarError::syntax_error(e.to_string())
    }
}
