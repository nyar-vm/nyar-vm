use std::fmt::Display;

use crate::ValkyrieError;

impl serde::ser::Error for ValkyrieError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        ValkyrieError::runtime_error(msg.to_string())
    }
}

impl serde::de::Error for ValkyrieError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        ValkyrieError::runtime_error(msg.to_string())
    }
}
