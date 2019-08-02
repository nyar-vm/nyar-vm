use crate::integer::types::NativeType;
use std::fmt;
use std::fmt::{Display, Formatter};

pub type Float32 = NativeType<f32>;

impl Display for Float32 {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}f32", self.value)
    }
}

impl From<f32> for NativeType<f32> {
    fn from(i: f32) -> Self {
        NativeType { value: i }
    }
}
