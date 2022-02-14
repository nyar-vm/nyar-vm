use std::ops::Range;

use serde::{Deserialize, Serialize};

use valkyrie_errors::{
    third_party::{
        Affix,
        Associativity::{Left, Right},
        PrattParser, Precedence,
    },
    FileID, FileSpan, SyntaxError, ValkyrieResult,
};

use crate::{BinaryExpression, UnaryExpression, ValkyrieASTNode};

pub mod resolver;

// From this
#[derive(Debug)]
pub enum UnknownOrder {
    Infix(ValkyrieOperator),
    Prefix(ValkyrieOperator),
    Suffix(ValkyrieOperator),
    Value(ValkyrieASTNode),
    Group(Vec<UnknownOrder>),
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieOperator {
    pub kind: OperatorKind,
    pub span: FileSpan,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OperatorKind {
    // suffix operator: `a!`
    Bang,
    // suffix operator: `a?`
    Question,
    // suffix operator: `a%, b‰, c‱`
    DivideByDecimalPower(u8),
    // suffix operator: `℃'
    Celsius,
    // suffix operator: '℉`
    Fahrenheit,
    // suffix operator: `!a`
    Not,
    // suffix operator: `⇵a`
    Flip,
    // prefix positive: `⇆a`
    Reverse,
    // prefix positive: `+a`
    Positive,
    // prefix negative: `-a`
    Negative,
    // prefix operator: `√a, ∛b, ∜c`
    RootOf(u8),
    // infix operator: `+`
    Add,
    // ++
    Concat,
    //
    Subtract,
    // >
    Greater,
    // >=
    GreaterEqual,
    // <
    Less,
    // <=
    LessEqual,
    // infix operator `∗ ⋆ ⋆`
    MultiplyBroadcast,
    // infix operator `/ ⁄ ∕`
    Divide,
    // infix operator `÷`
    Quotient,
    // infix operator `^`
    Power,
    LogicGate(u8),

    // function return operator `→`
    Return,
    Is(bool),
    //
    As,
    // a in b, a ∊
    In(bool),
    // a contains b
    Contains(bool),
}

impl ValkyrieOperator {
    pub fn prefix(s: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        match OperatorKind::parse_prefix(s) {
            Ok(o) => Ok(Self { kind: o, span: FileSpan { file, head: range.start, tail: range.end } }),
            Err(e) => Err(e.with_file(file).with_range(range))?,
        }
    }
    pub fn infix(s: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        match OperatorKind::parse_infix(s) {
            Ok(o) => Ok(Self { kind: o, span: FileSpan { file, head: range.start, tail: range.end } }),
            Err(e) => Err(e.with_file(file).with_range(range))?,
        }
    }
    pub fn suffix(s: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        match OperatorKind::parse_suffix(s) {
            Ok(o) => Ok(Self { kind: o, span: FileSpan { file, head: range.start, tail: range.end } }),
            Err(e) => Err(e.with_file(file).with_range(range))?,
        }
    }
}

impl OperatorKind {
    pub fn parse_prefix(s: &str) -> Result<Self, SyntaxError> {
        let out = match Self::normalize(s).as_str() {
            "+" => OperatorKind::Positive,
            "-" => OperatorKind::Negative,
            _ => Err(SyntaxError::new(format!("Unknown prefix `{}`", s)))?,
        };
        Ok(out)
    }
    pub fn parse_infix(s: &str) -> Result<Self, SyntaxError> {
        let normed = Self::normalize(s);
        let out = match normed.as_str() {
            "+" => OperatorKind::Add,
            "++" => OperatorKind::Concat,
            "-" => OperatorKind::Subtract,
            "*" => OperatorKind::MultiplyBroadcast,
            "/" => OperatorKind::Divide,
            // root of
            "√" => OperatorKind::RootOf(2),
            "∛" => OperatorKind::RootOf(3),
            "∜" => OperatorKind::RootOf(4),
            // comparison
            "^" => OperatorKind::Power,
            ">" => OperatorKind::Greater,
            ">=" => OperatorKind::GreaterEqual,
            "<" => OperatorKind::Less,
            "<=" => OperatorKind::LessEqual,
            // other
            "->" => OperatorKind::Return,
            "∈" => OperatorKind::In(true),
            "!∈" => OperatorKind::In(false),
            s if s.starts_with("not") && s.ends_with("in") => OperatorKind::In(false),
            "<:" => OperatorKind::Is(true),
            "!<:" => OperatorKind::Is(false),
            "<:!" => OperatorKind::Is(false),
            // logic gate
            "∧" | "&&" => OperatorKind::LogicGate(0b0001),
            "⊼" => OperatorKind::LogicGate(0b0100),
            "⩟" => OperatorKind::LogicGate(0b0000),
            "∨" | "||" => OperatorKind::LogicGate(0b0111),
            "⊽" => OperatorKind::LogicGate(0b0010),
            "⊻" => OperatorKind::LogicGate(0b0110),
            // "==" => OperatorKind::Equal,
            s if s.starts_with("is") && s.ends_with("not") => OperatorKind::Is(false),
            _ => Err(SyntaxError::new(format!("Unknown infix `{}`", normed)))?,
        };
        Ok(out)
    }
    pub fn parse_suffix(s: &str) -> Result<Self, SyntaxError> {
        let out = match Self::normalize(s).as_str() {
            "contains" => OperatorKind::Contains(true),
            s if s.starts_with("not") && s.ends_with("contains") => OperatorKind::Contains(false),
            _ => Err(SyntaxError::new(format!("Unknown suffix `{}`", s)))?,
        };
        Ok(out)
    }
}

impl OperatorKind {
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
                '∊' | '∈' => out.push_str("∈"),
                '∉' => out.push_str("!∈"),
                '∋' | '∍' => out.push_str("∋"),
                s if s.is_whitespace() => continue,
                _ => out.push(c),
            }
        }

        out
    }
}

impl OperatorKind {
    pub fn literal(&self) -> &str {
        // Ligatures are not supported in document
        match self {
            OperatorKind::Add => "+",
            OperatorKind::Subtract => "-",
            OperatorKind::MultiplyBroadcast => "×",
            OperatorKind::Divide => "÷",
            OperatorKind::Return => "→",
            OperatorKind::Is(_) => {
                todo!()
            }
            OperatorKind::In(_) => {
                todo!()
            }
            OperatorKind::Contains(_) => {
                todo!()
            }
            _ => {
                todo!("{:?}", self)
            }
        }
    }
    pub fn name(&self) -> &str {
        match self {
            OperatorKind::Add => "plus",
            OperatorKind::Subtract => "minus",
            OperatorKind::MultiplyBroadcast => "multiply",
            OperatorKind::Divide => "divide",
            OperatorKind::Return => "return",
            OperatorKind::Is(_) => {
                todo!()
            }
            _ => todo!("{:?}", self),
        }
    }
}
