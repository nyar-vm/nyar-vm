use super::*;
use crate::ast::ByteLiteral;

impl Display for IntegerLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.value, self.handler)
    }
}

impl Display for DecimalLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.value, self.handler)
    }
}

impl Display for ByteLiteral {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "0{}{}", self.handler, self.value)
    }
}
