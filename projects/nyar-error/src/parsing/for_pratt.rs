use std::fmt::{Debug, Display};

use pratt::PrattError;

use crate::NyarError;

impl<I, E> From<PrattError<I, E>> for NyarError
where
    I: Debug,
    E: Into<NyarError> + Display,
{
    fn from(value: PrattError<I, E>) -> Self {
        match value {
            PrattError::UserError(e) => e.into(),
            PrattError::EmptyInput => NyarError::runtime_error("Empty input"),
            PrattError::UnexpectedNilfix(v) => NyarError::runtime_error(format!("Unexpected nilfix: {:?}", v)),
            PrattError::UnexpectedPrefix(v) => NyarError::runtime_error(format!("Unexpected prefix: {:?}", v)),
            PrattError::UnexpectedInfix(v) => NyarError::runtime_error(format!("Unexpected infix: {:?}", v)),
            PrattError::UnexpectedPostfix(v) => NyarError::runtime_error(format!("Unexpected postfix: {:?}", v)),
        }
    }
}
