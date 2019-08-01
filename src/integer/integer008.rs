use std::fmt::{Display, Formatter, Result};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Integer8 {
    pub value: i8,
}

impl Display for Integer8 {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}i8", self.value)
    }
}
