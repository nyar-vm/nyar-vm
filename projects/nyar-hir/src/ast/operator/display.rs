use super::*;

impl Debug for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operator::Prefix(o) => Debug::fmt(o, f),
            Operator::Infix(o) => Debug::fmt(o, f),
            Operator::Postfix(o) => Debug::fmt(o, f),
        }
    }
}

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
            Infix::Concat => f.write_str("Concat::concat"),
            Infix::Remove => f.write_str("Remove::remove"),
            Infix::Power => f.write_str("Power::power"),
        }
    }
}

impl Debug for Postfix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Postfix::Increment => f.write_str("Increment::increment"),
            Postfix::Decrement => f.write_str("Decrement::decrement"),
            Postfix::Unchecked => f.write_str("unchecked"),
            Postfix::Raise => f.write_str("raise"),
        }
    }
}

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operator::Prefix(o) => Display::fmt(o, f),
            Operator::Infix(o) => Display::fmt(o, f),
            Operator::Postfix(o) => Display::fmt(o, f),
        }
    }
}

impl Display for Prefix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Prefix::Negative => f.write_str("-"),
            Prefix::Positive => f.write_str("+"),
            Prefix::Not => f.write_str("!"),
        }
    }
}

impl Display for Infix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Infix::Addition => f.write_str("+"),
            Infix::Subtraction => f.write_str("-"),
            Infix::Multiplication => f.write_str("*"),
            Infix::Division => f.write_str("/"),
            Infix::Concat => f.write_str("++"),
            Infix::Remove => f.write_str("--"),
            Infix::Power => f.write_str("^"),
        }
    }
}

impl Display for Postfix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Postfix::Increment => f.write_str("++"),
            Postfix::Decrement => f.write_str("--"),
            Postfix::Unchecked => f.write_str("!"),
            Postfix::Raise => f.write_str("?"),
        }
    }
}
