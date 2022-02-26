use std::fmt::Display;

use serde::de::Expected;

use crate::ValkyrieError;

impl serde::ser::Error for ValkyrieError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        todo!()
    }
}

impl serde::de::Error for ValkyrieError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        todo!()
    }
    fn invalid_length(len: usize, exp: &dyn Expected) -> Self {
        todo!()
    }
}
