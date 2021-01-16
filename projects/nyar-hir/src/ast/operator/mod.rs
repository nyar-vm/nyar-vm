use super::*;
mod display;
mod parse;

#[derive(Copy, Clone, Serialize, Deserialize)]
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
    pub fn parse(o: &str, a: i8) -> Self {
        match a {
            a if a < 0 => Operator::parse_prefix(o),
            a if a > 0 => Operator::parse_postfix(o),
            _ => Operator::parse_infix(o),
        }
    }
    fn parse_prefix(o: &str) -> Self {
        match Prefix::from_str(o) {
            Ok(o) => Self::Prefix(o),
            Err(_) => unimplemented!("Unknown prefix {}", o),
        }
    }
    fn parse_infix(o: &str) -> Self {
        match Infix::from_str(o) {
            Ok(o) => Self::Infix(o),
            Err(_) => unimplemented!("Unknown infix {}", o),
        }
    }
    fn parse_postfix(o: &str) -> Self {
        match Postfix::from_str(o) {
            Ok(o) => Self::Postfix(o),
            Err(_) => unimplemented!("Unknown postfix {}", o),
        }
    }
    pub fn associativity(&self) -> OperatorAssociativity {
        use OperatorAssociativity::{Left, Right};
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
}
