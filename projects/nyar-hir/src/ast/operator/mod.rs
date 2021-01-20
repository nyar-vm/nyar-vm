use super::*;

mod display;
mod parse;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum OperatorAssociativity {
    None,
    Left,
    Right,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Prefix {
    /// `-`
    Negative,
    /// `+`
    Positive,
    /// `!`
    Not,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Infix {
    Addition,
    Subtraction,
    Multiplication,
    Division,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Postfix {
    /// `++`
    Increment,
    /// `--`
    Decrement,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Operator {
    Prefix(Prefix),
    Infix(Infix),
    Postfix(Postfix),
}

impl Operator {
    pub fn associativity(&self) -> OperatorAssociativity {
        use OperatorAssociativity::{Left};
        match self {
            Operator::Prefix(_) => OperatorAssociativity::None,
            Operator::Postfix(_) => OperatorAssociativity::None,
            Operator::Infix(o) => match o {
                Infix::Addition => Left,
                Infix::Subtraction => Left,
                Infix::Multiplication => Left,
                Infix::Division => Left,
            },
        }
    }
    pub fn get_priority(&self) -> u8 {
        match self {
            Operator::Prefix(_) => 255,
            Operator::Postfix(_) => 255,
            Operator::Infix(o) => match o {
                Infix::Addition => 100,
                Infix::Subtraction => 100,
                Infix::Multiplication => 110,
                Infix::Division => 110,
            },
        }
    }
    pub fn is_shortcut(&self) -> bool {
        match self {
            Operator::Prefix(_) => false,
            Operator::Postfix(_) => false,
            Operator::Infix(o) => match o {
                _ => false,
            },
        }
    }
}
