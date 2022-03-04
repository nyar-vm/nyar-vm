use std::fmt::{Debug, Display};

use pratt::PrattError;

use crate::ValkyrieError;

impl<I, E> From<PrattError<I, E>> for ValkyrieError
where
    I: Debug,
    E: Into<ValkyrieError> + Display,
{
    fn from(value: PrattError<I, E>) -> Self {
        match value {
            PrattError::UserError(e) => e.into(),
            PrattError::EmptyInput => ValkyrieError::runtime_error("Empty input"),
            PrattError::UnexpectedNilfix(v) => ValkyrieError::runtime_error(format!("Unexpected nilfix: {:?}", v)),
            PrattError::UnexpectedPrefix(v) => ValkyrieError::runtime_error(format!("Unexpected prefix: {:?}", v)),
            PrattError::UnexpectedInfix(v) => ValkyrieError::runtime_error(format!("Unexpected infix: {:?}", v)),
            PrattError::UnexpectedPostfix(v) => ValkyrieError::runtime_error(format!("Unexpected postfix: {:?}", v)),
        }
    }
}
