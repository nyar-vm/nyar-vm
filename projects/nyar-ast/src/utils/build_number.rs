use crate::{ast::Position, AST};
use arc_number::Number;
use std::fmt::{self, Debug};

pub fn number_refine(h: &str, data: &str) -> AST {
    match Number::parse(h, data) {
        Some(s) => AST::Number(s),
        None => AST::NumberLiteral { handler: h.to_string(), data: data.to_string() },
    }
}

impl Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}:{}, {}:{})", self.start.0, self.start.1, self.end.0, self.end.1)
    }
}
