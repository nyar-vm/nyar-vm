use std::fmt::{Debug, Formatter};

/// `%var = binary(==, %a, %b);`
pub struct BinaryCall {
    operator: Operator,
    lhs: Variable,
    rhs: Variable,
    output: Variable,
}

pub struct Operator {
    kind: OperatorKind
}

pub enum OperatorKind {
    Prefix,
    Infix,
    Postfix
}


pub struct Variable {
    global: bool,
    name: String,
}

impl Debug for Variable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.global {
            true => write!(f, "@{}", self.name),
            false => write!(f, "%{}", self.name),
        }
    }
}