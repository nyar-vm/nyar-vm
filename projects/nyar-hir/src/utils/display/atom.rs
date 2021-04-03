use super::*;
use crate::ast::{KVPair, TableExpression};

impl Display for IntegerLiteral {
    //noinspection DuplicatedCode
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.handler.is_empty() {
            true => write!(f, "{}", self.value),
            false => write!(f, "{}_{}", self.value, self.handler),
        }
    }
}

impl Display for DecimalLiteral {
    //noinspection DuplicatedCode
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.handler.is_empty() {
            true => write!(f, "{}", self.value),
            false => write!(f, "{}_{}", self.value, self.handler),
        }
    }
}

impl Display for ByteLiteral {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "0{}{}", self.handler, self.value)
    }
}

impl Display for KVPair {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.key, self.value)
    }
}
