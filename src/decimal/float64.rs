use crate::integer::types::NativeType;
use std::fmt;
use std::fmt::{Display, Formatter};

pub type Float64 = NativeType<f64>;

impl Display for Float64 {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}f64", self.value)
    }
}

impl From<f64> for NativeType<f64> {
    fn from(i: f64) -> Self {
        NativeType { value: i }
    }
}
