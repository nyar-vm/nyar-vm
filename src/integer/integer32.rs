use std::fmt::{Display, Formatter, Result};

use super::types::{Integer32, IntegerType};

impl Display for Integer32 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i32", self.value)
    }
}

impl From<i32> for IntegerType<i32> {
    fn from(i: i32) -> Self {
        IntegerType { value: i }
    }
}
