use std::fmt::{Display, Formatter, Result};

use super::types::{Integer128, NativeType};

impl Display for Integer128 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i128", self.value)
    }
}
impl From<i128> for Integer128 {
    fn from(i: i128) -> Self {
        NativeType { value: i }
    }
}
