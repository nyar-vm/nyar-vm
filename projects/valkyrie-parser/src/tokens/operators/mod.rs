use super::*;

pub enum ValkyrieOperator {
    Add,
    Subtract,
    // infix operator `∗ ⋆ ⋆`
    MultiplyBroadcast,
    // infix operator `÷ / ⁄ ∕`
    Slash,
    // function return operator `→`
    Return,
    Is(bool),
    // a in b
    In(bool),
    // a contains b
    Contains(bool),
}

impl FromStr for ValkyrieOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let o = match normalize_operator(s).as_str() {
            "+" => ValkyrieOperator::Add,
            "-" => ValkyrieOperator::Subtract,
            "*" => ValkyrieOperator::MultiplyBroadcast,
            "/" => ValkyrieOperator::Slash,
            "->" => ValkyrieOperator::Return,
            "in" => ValkyrieOperator::In(true),
            s if s.starts_with("not") && s.ends_with("in") => ValkyrieOperator::In(false),
            "is" => ValkyrieOperator::Is(true),
            s if s.starts_with("is") && s.ends_with("not") => ValkyrieOperator::Is(false),
            _ => Err(s.to_string())?,
        };
        Ok(o)
    }
}

fn normalize_operator(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for char in s.chars() {
        match char {
            '⋆' | '∗' => out.push('*'),
            '÷' | '⁄' | '∕' => out.push('/'),
            '→' => out.push_str("->"),
            _ => out.push(char),
        }
    }
    out
}

impl ValkyrieOperator {
    pub fn literal(&self) -> &str {
        // Ligatures are not supported in document
        match self {
            ValkyrieOperator::Add => "+",
            ValkyrieOperator::Subtract => "-",
            ValkyrieOperator::MultiplyBroadcast => "×",
            ValkyrieOperator::Slash => "÷",
            ValkyrieOperator::Return => "→",
        }
    }
    pub fn name(&self) -> &str {
        match self {
            ValkyrieOperator::Add => "plus",
            ValkyrieOperator::Subtract => "minus",
            ValkyrieOperator::MultiplyBroadcast => "multiply",
            ValkyrieOperator::Slash => "divide",
            ValkyrieOperator::Return => "return",
        }
    }
}
