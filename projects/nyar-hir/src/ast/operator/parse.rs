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
    type Err = !;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        try {
            match s {
                _ => Self { symbol: s.to_string(), priority: 0 },
            }
        }
    }
}

impl FromStr for Infix {
    type Err = !;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        try {
            match s {
                "^" => Self { symbol: s.to_string(), priority: 120, associativity: OperatorAssociativity::Right },
                _ => Self { symbol: s.to_string(), priority: 100, associativity: OperatorAssociativity::Left },
            }
        }
    }
}

impl FromStr for Postfix {
    type Err = !;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        try {
            match s {
                _ => Self { symbol: s.to_string(), priority: 255 },
            }
        }
    }
}
