use super::*;

impl Operator {
    pub fn parse_prefix(o: &str) -> Self {
        match Prefix::from_str(o) {
            Ok(o) => Self::Prefix(o),
            Err(_) => unimplemented!("Unknown prefix {}", o),
        }
    }
    pub fn parse_infix(o: &str) -> Self {
        match Infix::from_str(o) {
            Ok(o) => Self::Infix(o),
            Err(_) => unimplemented!("Unknown infix {}", o),
        }
    }
    pub fn parse_postfix(o: &str) -> Self {
        match Postfix::from_str(o) {
            Ok(o) => Self::Postfix(o),
            Err(_) => unimplemented!("Unknown postfix {}", o),
        }
    }
}

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
