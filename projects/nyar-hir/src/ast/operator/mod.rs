use super::*;

mod display;
mod parse;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum OperatorAssociativity {
    None,
    Left,
    Right,
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Prefix {
    /// `-`
    Negative,
    /// `+`
    Positive,
    /// `!`
    Not,
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Infix {
    // `+`
    Addition,
    // `-`
    Subtraction,
    // `*`
    Multiplication,
    // `/`
    Division,
    // `^`
    Power,
    // `++`
    Concat,
    // `--`
    Remove,
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Postfix {
    /// `++`
    Increment,
    /// `--`
    Decrement,
    /// `a!`
    Unchecked,
    /// `a?`
    Raise,
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Operator {
    Prefix(Prefix),
    Infix(Infix),
    Postfix(Postfix),
}

impl Operator {
    pub fn associativity(&self) -> OperatorAssociativity {
        use OperatorAssociativity::{Left, Right};
        match self {
            Operator::Prefix(_) => OperatorAssociativity::None,
            Operator::Postfix(_) => OperatorAssociativity::None,
            Operator::Infix(o) => match o {
                Infix::Power => Right,
                _ => Left,
            },
        }
    }
    pub fn get_priority(&self) -> u8 {
        match self {
            Operator::Prefix(_) => 255,
            Operator::Postfix(_) => 255,
            Operator::Infix(o) => match o {
                Infix::Addition | Infix::Subtraction => 100,
                Infix::Multiplication | Infix::Division => 110,
                Infix::Power => 120,
                Infix::Concat => 90,
                Infix::Remove => 90,
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
