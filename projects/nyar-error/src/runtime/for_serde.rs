use std::fmt::Display;

use crate::NyarError;

impl serde::ser::Error for NyarError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        NyarError::runtime_error(msg.to_string())
    }
}

impl serde::de::Error for NyarError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        NyarError::runtime_error(msg.to_string())
    }
}
