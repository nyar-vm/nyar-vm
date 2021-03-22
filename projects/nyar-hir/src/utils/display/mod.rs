use std::fmt::{Debug, Display, Formatter, Write};

use crate::{
    ast::{ByteLiteral, DecimalLiteral, IntegerLiteral},
    ASTKind, ASTNode,
};

mod atom;

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::Nothing, span: Default::default() }
    }
}

impl From<ASTKind> for ASTNode {
    fn from(kind: ASTKind) -> Self {
        Self { kind, span: Default::default() }
    }
}

impl Debug for ASTNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ASTKind::Nothing => f.write_str("<<unreachable Nothing>>"),
            ASTKind::Suite(v) => {
                f.write_str("Suite")?;
                f.debug_list().entries(v.iter()).finish()
            }
            ASTKind::Sequence(v) => {
                f.write_str("Sequence")?;
                f.debug_list().entries(v.iter()).finish()
            }
            ASTKind::InfixExpression(v) => Debug::fmt(v, f),
            ASTKind::TupleExpression(v) => write_tuple("Tuple", v, f),
            ASTKind::TableExpression(v) => Debug::fmt(v, f),
            ASTKind::PairExpression(v) => Debug::fmt(v, f),
            ASTKind::Boolean(v) => Display::fmt(v, f),
            ASTKind::Byte(v) => Debug::fmt(v, f),
            ASTKind::Integer(v) => Display::fmt(v, f),
            ASTKind::Decimal(v) => Display::fmt(v, f),
            ASTKind::String(v) => match v.handler.is_empty() {
                true => Debug::fmt(&v.literal, f),
                false => Debug::fmt(v, f),
            },
            ASTKind::StringTemplate(v) => write_tuple("StringTemplate", v, f),
            ASTKind::Symbol(v) => Display::fmt(v, f),
            _ => Debug::fmt(&self.kind, f),
        }
    }
}

pub fn write_tuple(name: &str, v: &[ASTNode], f: &mut Formatter<'_>) -> std::fmt::Result {
    if v.is_empty() {
        f.write_str(name)?;
        return f.write_str("()");
    }
    let mut w = &mut f.debug_tuple(name);
    for node in v {
        w = w.field(node)
    }
    w.finish()
}

impl Display for ASTNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.kind, f)
    }
}

impl Display for ASTKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ASTKind::Nothing => f.write_str("<<unreachable Nothing>>"),
            ASTKind::Program(v) | ASTKind::Sequence(v) => {
                for i in v {
                    Display::fmt(i, f)?;
                    f.write_char('\n')?
                }
                Ok(())
            }
            ASTKind::Boolean(v) => Display::fmt(v, f),
            ASTKind::Integer(v) => Display::fmt(v, f),
            ASTKind::Byte(v) => Display::fmt(v, f),
            ASTKind::Decimal(v) => Display::fmt(v, f),
            ASTKind::String(v) => Display::fmt(v, f),
            ASTKind::Symbol(v) => Display::fmt(v, f),
            ASTKind::PairExpression(v) => Display::fmt(v, f),
            _ => {
                todo!()
            }
        }
    }
}
