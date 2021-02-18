mod atom;

use crate::{
    ast::{DecimalLiteral, IntegerLiteral},
    ASTKind, ASTNode,
};
use std::fmt::{Debug, Display, Formatter};

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
            ASTKind::ListExpression(v) => write_tuple("List", v, f),
            ASTKind::TupleExpression(v) => write_tuple("Tuple", v, f),
            ASTKind::Boolean(v) => Display::fmt(v, f),
            ASTKind::Integer(v) => Display::fmt(v, f),
            ASTKind::Decimal(v) => Display::fmt(v, f),
            ASTKind::Byte(v) => Display::fmt(v, f),
            ASTKind::String(v) => {
                if v.handler.is_empty() {
                    Debug::fmt(&v.literal, f)
                }
                else {
                    Debug::fmt(v, f)
                }
            }
            ASTKind::StringTemplate(v) => {
                if v.is_empty() {
                    f.write_str("''")
                }
                else {
                    f.write_str("StringTemplate")?;
                    f.debug_list().entries(v.iter()).finish()
                }
            }
            ASTKind::Symbol(v) => Display::fmt(v, f),
            _ => Debug::fmt(&self.kind, f),
        }
    }
}

fn write_tuple(name: &str, v: &[ASTNode], f: &mut Formatter<'_>) -> std::fmt::Result {
    let w = &mut f.debug_tuple(name);
    for i in v {
        w.field(i);
    }
    w.finish()
}

impl Display for ASTKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ASTKind::Nothing => f.write_str("<<unreachable Nothing>>"),
            ASTKind::Program(v) => {
                f.write_str("Program")?;
                f.debug_list().entries(v.iter()).finish()
            }
            ASTKind::Sequence(_v) => f.write_str("<<unreachable Sequence>>"),
            ASTKind::LetBind(_v) => {
                todo!()
            }
            ASTKind::LambdaFunction(_v) => {
                todo!()
            }
            ASTKind::InfixExpression(_v) => {
                todo!()
            }
            ASTKind::TupleExpression(_v) => {
                todo!()
            }
            ASTKind::ListExpression(_v) => {
                todo!()
            }
            ASTKind::DictExpression(_v) => {
                todo!()
            }
            ASTKind::IfStatement(_) => {
                todo!()
            }
            ASTKind::LoopStatement(_) => {
                todo!()
            }
            ASTKind::Suite(_) => {
                todo!()
            }
            ASTKind::Boolean(v) => Display::fmt(v, f),
            ASTKind::Integer(v) => Display::fmt(v, f),
            ASTKind::Byte(v) => Display::fmt(v, f),
            ASTKind::Decimal(v) => Display::fmt(v, f),
            ASTKind::String(v) => Display::fmt(v, f),
            ASTKind::StringTemplate(_v) => {
                todo!()
            }
            ASTKind::XMLTemplate(_v) => {
                todo!()
            }
            ASTKind::Symbol(v) => write!(f, "{}", v),
        }
    }
}
