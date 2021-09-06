use super::*;

pub enum ValkyrieOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Return,
}

impl FromStr for ValkyrieOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+" => Ok(ValkyrieOperator::Add),
            "-" => Ok(ValkyrieOperator::Subtract),
            "*" => Ok(ValkyrieOperator::Multiply),
            "/" => Ok(ValkyrieOperator::Divide),
            "->" => Ok(ValkyrieOperator::Return),
            _ => Err(s.to_string()),
        }
    }
}

impl ValkyrieOperator {
    pub fn literal(&self) -> &str {
        // Ligatures are not supported in document
        match self {
            ValkyrieOperator::Add => "+",
            ValkyrieOperator::Subtract => "-",
            ValkyrieOperator::Multiply => "×",
            ValkyrieOperator::Divide => "÷",
            ValkyrieOperator::Return => "→",
        }
    }
    pub fn name(&self) -> &str {
        match self {
            ValkyrieOperator::Add => "plus",
            ValkyrieOperator::Subtract => "minus",
            ValkyrieOperator::Multiply => "multiply",
            ValkyrieOperator::Divide => "divide",
            ValkyrieOperator::Return => "return",
        }
    }
}
