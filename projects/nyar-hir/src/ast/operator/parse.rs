use super::*;

impl FromStr for Prefix {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let out = match s {
            "!" => Self::Not,
            "+" => Self::Positive,
            "-" => Self::Negative,
            _ => return Err(()),
        };
        Ok(out)
    }
}

impl FromStr for Infix {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let out = match s {
            "+" => Self::Addition,
            "-" => Self::Subtraction,
            "*" => Self::Multiplication,
            "/" => Self::Division,
            _ => return Err(()),
        };
        Ok(out)
    }
}

impl FromStr for Postfix {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let out = match s {
            "+" => Self::Increment,
            "-" => Self::Decrement,
            _ => return Err(()),
        };
        Ok(out)
    }
}
