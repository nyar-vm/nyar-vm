use std::fmt::{Display, Formatter, Result};

use super::types::{Integer8, IntegerType};

impl Display for Integer8 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i8", self.value)
    }
}


impl From<i8> for IntegerType<i8> {
    fn from(i: i8) -> Self {
        IntegerType { value: i }
    }
}