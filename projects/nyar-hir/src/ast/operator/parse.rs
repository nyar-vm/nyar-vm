use nyar_error::NyarError;

use super::*;

impl Operator {
    pub fn parse_prefix(o: &str) -> Self {
        Self::Prefix(Prefix::from_str(o).unwrap())
    }
    pub fn parse_infix(o: &str) -> Self {
        Self::Infix(Infix::from_str(o).unwrap())
    }
    pub fn parse_postfix(o: &str) -> Self {
        Self::Postfix(Postfix::from_str(o).unwrap())
    }
}

impl FromStr for Prefix {
    type Err = NyarError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let out = match s {
            "!" => Self::Not,
            "+" => Self::Positive,
            "-" => Self::Negative,
            _ => return Err(NyarError::syntax_error(format!("Unknown prefix {}", s))),
        };
        Ok(out)
    }
}

impl FromStr for Infix {
    type Err = NyarError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let out = match s {
            "+" => Self::Addition,
            "++" => Self::Concat,
            "-" => Self::Subtraction,
            "--" => Self::Remove,
            "*" => Self::Multiplication,
            "/" => Self::Division,
            "^" => Self::Power,
            _ => return Err(NyarError::syntax_error(format!("Unknown infix {}", s))),
        };
        Ok(out)
    }
}

impl FromStr for Postfix {
    type Err = NyarError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let out = match s {
            "+" => Self::Increment,
            "-" => Self::Decrement,
            "!" => Self::Unchecked,
            _ => return Err(NyarError::syntax_error(format!("Unknown suffix {}", s))),
        };
        Ok(out)
    }
}
