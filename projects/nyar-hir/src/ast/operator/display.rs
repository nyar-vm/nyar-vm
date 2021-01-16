use super::*;

impl Debug for Prefix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Prefix::Negative => f.write_str("Negative::negative"),
            Prefix::Positive => f.write_str("Positive::positive"),
            Prefix::Not => f.write_str("Not::not"),
        }
    }
}

impl Debug for Infix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Infix::Addition => f.write_str("Addition::add"),
            Infix::Subtraction => f.write_str("Subtraction::subtract"),
            Infix::Multiplication => f.write_str("Multiplication::multiply"),
            Infix::Division => f.write_str("Division::divide"),
        }
    }
}

impl Debug for Postfix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Postfix::Increment => f.write_str("Increment::increment"),
            Postfix::Decrement => f.write_str("Decrement::decrement"),
        }
    }
}

impl Debug for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operator::Prefix(o) => Debug::fmt(o, f),
            Operator::Infix(o) => Debug::fmt(o, f),
            Operator::Postfix(o) => Debug::fmt(o, f),
        }
    }
}
