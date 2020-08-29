use super::*;
use OperatorAssociativity::*;

#[derive(Copy, Clone, Debug)]
pub enum OperatorAssociativity {
    Left,
    Right,
}

#[derive(Copy,Clone)]
pub enum Operator {
    Prefix {
        op: &'static str,
    },
    Infix {
        assoc: OperatorAssociativity,
        prec: u8,
        op: &'static str,
    },
    Suffix {
        op: &'static str,
    },
}

impl Debug for Operator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Operator::Prefix { op,.. } => f.write_str(op),
            Operator::Infix { op,.. } => f.write_str(op),
            Operator::Suffix { op,.. } => f.write_str(op)
        }
    }
}

impl Operator {
    pub fn parse(o: &str, a: i8) -> Self {
        match a {
            a if a < 0 => Operator::parse_prefix(o),
            a if a > 0 => Operator::parse_suffix(o),
            _ => Operator::parse_infix(o),
        }
    }
    fn parse_prefix(o: &str) -> Self {
        match o {
            "+" => Self::PREFIX_PLUS,
            "-" => Self::PREFIX_MINUS,
            "!" => Self::PREFIX_NOT,
            _ => unimplemented!("{:?}", o),
        }
    }
    fn parse_infix(o: &str) -> Self {
        match o {
            "+" => Self::INFIX_PLUS,
            "-" => Self::INFIX_MINUS,
            _ => unimplemented!("{:?}", o),
        }
    }
    fn parse_suffix(o: &str) -> Self {
        match o {
            "+" => Self::SUFFIX_PLUS,
            "-" => Self::SUFFIX_MINUS,
            "!" => Self::SUFFIX_FACTORIAL,
            "!!" => Self::SUFFIX_FACTORIAL2,
            _ => unimplemented!("{:?}", o),
        }
    }
}

impl Operator {
    pub const PREFIX_PLUS: Self = Self::Prefix { op: "+" };
    pub const PREFIX_MINUS: Self = Self::Prefix { op: "-" };
    pub const PREFIX_NOT: Self = Self::Prefix { op: "!" };
}


impl Operator {
    pub const INFIX_PLUS: Self = Self::Infix { assoc: Left, prec: 100, op: "+" };
    pub const INFIX_MINUS: Self = Self::Infix { assoc: Left, prec: 100, op: "-" };
    pub const INFIX_POWER: Self = Self::Infix { assoc: Right, prec: 120, op: "*" };
}

impl Operator {
    pub const SUFFIX_PLUS: Self = Self::Suffix { op: "+" };
    pub const SUFFIX_MINUS: Self = Self::Suffix { op: "-" };
    pub const SUFFIX_FACTORIAL: Self = Self::Suffix { op: "!" };
    pub const SUFFIX_FACTORIAL2: Self = Self::Suffix { op: "!!" };
}
