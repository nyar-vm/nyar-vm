use pratt::{Affix, Associativity, PrattParser, Precedence};
use valkyrie_errors::SyntaxError;
mod resolver;

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
    // a in b, a ∊
    In(bool),
    // a contains b
    Contains(bool),
}

impl FromStr for ValkyrieOperator {
    type Err = SyntaxError;

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
            _ => Err(SyntaxError::new(format!("Unknown operator `{}`", s)))?,
        };
        Ok(o)
    }
}

impl ValkyrieOperator {
    pub fn normalize(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '⋆' | '∗' => out.push('*'),
                '÷' | '⁄' | '∕' => out.push('/'),
                '→' => out.push_str("->"),
                '¬' => out.push('!'),
                'n' => {
                    if chars.next() == Some('o') && chars.next() == Some('t') {
                        out.push_str("!");
                    }
                }
                'i' => match chars.next() {
                    Some('s') => out.push_str("<:"),
                    Some('n') => out.push('∈'),
                    _ => {}
                },
                //
                '∋' | '∍' | '∊' | '∈' | '∉' | '∌' => out.push_str("in"),
                s if s.is_whitespace() => continue,
                _ => out.push(char),
            }
        }

        out
    }
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
            ValkyrieOperator::Is(_) => {
                todo!()
            }
            ValkyrieOperator::In(_) => {
                todo!()
            }
            ValkyrieOperator::Contains(_) => {
                todo!()
            }
        }
    }
    pub fn name(&self) -> &str {
        match self {
            ValkyrieOperator::Add => "plus",
            ValkyrieOperator::Subtract => "minus",
            ValkyrieOperator::MultiplyBroadcast => "multiply",
            ValkyrieOperator::Slash => "divide",
            ValkyrieOperator::Return => "return",
            ValkyrieOperator::Is(_) => {
                todo!()
            }
            ValkyrieOperator::In(_) => {
                todo!()
            }
            ValkyrieOperator::Contains(_) => {
                todo!()
            }
        }
    }
}
