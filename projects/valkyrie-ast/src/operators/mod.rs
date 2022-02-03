use std::ops::Range;

use pratt::{
    Affix,
    Associativity::{Left, Right},
    PrattParser, Precedence,
};
use serde::{Deserialize, Serialize};

use valkyrie_errors::{FileID, FileSpan, SyntaxError, ValkyrieResult};

use crate::{BinaryExpression, UnaryExpression, ValkyrieASTNode};

mod resolver;

pub struct ExpressionOrderResolver {}

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

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum OperatorKind {
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

impl ValkyrieOperator {
    pub fn prefix(s: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        let kind = OperatorKind::parse_prefix(s)?;
        Ok(Self { kind, span: FileSpan { file, head: range.start, tail: range.end } })
    }
    pub fn infix(s: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        let kind = OperatorKind::parse_infix(s)?;
        Ok(Self { kind, span: FileSpan { file, head: range.start, tail: range.end } })
    }
    pub fn suffix(s: &str, file: FileID, range: &Range<usize>) -> ValkyrieResult<Self> {
        let kind = OperatorKind::parse_suffix(s)?;
        Ok(Self { kind, span: FileSpan { file, head: range.start, tail: range.end } })
    }
}

impl OperatorKind {
    pub fn parse_prefix(s: &str) -> ValkyrieResult<Self> {
        let out = match Self::normalize(s).as_str() {
            "not" => OperatorKind::Is(false),
            _ => Err(SyntaxError::new(format!("Unknown prefix `{}`", s)))?,
        };
        Ok(out)
    }
    pub fn parse_infix(s: &str) -> ValkyrieResult<Self> {
        let out = match Self::normalize(s).as_str() {
            "+" => OperatorKind::Add,
            "-" => OperatorKind::Subtract,
            "*" => OperatorKind::MultiplyBroadcast,
            "/" => OperatorKind::Slash,
            "->" => OperatorKind::Return,
            "in" => OperatorKind::In(true),
            s if s.starts_with("not") && s.ends_with("in") => OperatorKind::In(false),
            "is" => OperatorKind::Is(true),
            s if s.starts_with("is") && s.ends_with("not") => OperatorKind::Is(false),
            _ => Err(SyntaxError::new(format!("Unknown infix `{}`", s)))?,
        };
        Ok(out)
    }
    pub fn parse_suffix(s: &str) -> ValkyrieResult<Self> {
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
                '∋' | '∍' | '∊' | '∈' | '∉' | '∌' => out.push_str("in"),
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
            OperatorKind::Slash => "÷",
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
        }
    }
    pub fn name(&self) -> &str {
        match self {
            OperatorKind::Add => "plus",
            OperatorKind::Subtract => "minus",
            OperatorKind::MultiplyBroadcast => "multiply",
            OperatorKind::Slash => "divide",
            OperatorKind::Return => "return",
            OperatorKind::Is(_) => {
                todo!()
            }
            OperatorKind::In(_) => {
                todo!()
            }
            OperatorKind::Contains(_) => {
                todo!()
            }
        }
    }
}
