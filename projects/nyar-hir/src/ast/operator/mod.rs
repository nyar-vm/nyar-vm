use super::*;

mod display;
mod parse;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Operator {
    Prefix(Prefix),
    Infix(Infix),
    Postfix(Postfix),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Prefix {
    pub symbol: String,
    pub priority: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Infix {
    pub symbol: String,
    pub priority: u8,
    pub associativity: OperatorAssociativity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Postfix {
    pub symbol: String,
    pub priority: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OperatorAssociativity {
    None,
    Left,
    Right,
}

impl Operator {
    pub fn is_shortcut(&self) -> bool {
        match self {
            Operator::Prefix(_) => false,
            Operator::Infix(s) => match s.symbol.as_str() {
                "||" | "&&" => true,
                _ => false,
            },
            Operator::Postfix(_) => false,
        }
    }
    pub fn priority(&self) -> u8 {
        match self {
            Operator::Prefix(s) => s.priority,
            Operator::Infix(s) => s.priority,
            Operator::Postfix(s) => s.priority,
        }
    }
}
