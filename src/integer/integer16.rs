use super::types::*;
use std::fmt::{Display, Formatter, Result};

impl Display for Integer16 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i16", self.value)
    }
}

impl From<i16> for Integer16 {
    fn from(i: i16) -> Self {
        NativeType { value: i }
    }
}
