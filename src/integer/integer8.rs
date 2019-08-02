use super::types::*;
use std::fmt::{Display, Formatter, Result};

impl Display for Integer8 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i8", self.value)
    }
}

impl From<i8> for NativeType<i8> {
    fn from(i: i8) -> Self {
        NativeType { value: i }
    }
}
