use super::*;

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Operator::Prefix(s) => f.write_str(&s.symbol),
            Operator::Infix(s) => f.write_str(&s.symbol),
            Operator::Postfix(s) => f.write_str(&s.symbol),
        }
    }
}
