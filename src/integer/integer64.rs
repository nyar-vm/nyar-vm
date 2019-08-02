use std::fmt::{Display, Formatter, Result};

use super::types::{Integer64, NativeType};

impl Display for Integer64 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i64", self.value)
    }
}
impl From<i64> for Integer64 {
    fn from(i: i64) -> Self {
        NativeType { value: i }
    }
}
