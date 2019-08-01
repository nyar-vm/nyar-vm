use std::fmt::{Display, Formatter, Result};

use super::types::{Integer16, IntegerType};

impl Display for Integer16 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i16", self.value)
    }
}

impl From<i16> for IntegerType<i16> {
    fn from(i: i16) -> Self {
        IntegerType { value: i }
    }
}
