use crate::NyarError3;
use bigdecimal::ParseBigDecimalError;
use num::bigint::ParseBigIntError;

impl From<ParseBigIntError> for NyarError3 {
    fn from(e: ParseBigIntError) -> Self {
        NyarError3::syntax_error(e.to_string())
    }
}

impl From<ParseBigDecimalError> for NyarError3 {
    fn from(e: ParseBigDecimalError) -> Self {
        NyarError3::syntax_error(e.to_string())
    }
}
