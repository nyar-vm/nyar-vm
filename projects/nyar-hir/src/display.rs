use std::{
    fmt::{Debug, Display, Formatter},
    ops::Range,
};

use valkyrie_errors::{FileID, FileSpan};

use crate::{ValkyrieASTKind, ValkyrieASTNode};

impl Debug for ValkyrieASTKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValkyrieASTKind::Statement(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Namespace(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Unary(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Binary(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Namepath(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Integer(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Decimal(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Boolean(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Null => {
                write!(f, "null")
            }
            ValkyrieASTKind::HDict(v) => Debug::fmt(v, f),
            ValkyrieASTKind::HList(v) => f.debug_struct("Tuple").field("nodes", v).finish(),
            ValkyrieASTKind::Bytes(v) => f.debug_struct("Bytes").field("nodes", v).finish(),
            ValkyrieASTKind::StringInterpolation(v) => Debug::fmt(v, f),
            ValkyrieASTKind::String(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Class(v) => Debug::fmt(v, f),
            ValkyrieASTKind::For(v) => Debug::fmt(v, f),
            ValkyrieASTKind::While(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Match(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Which(v) => Debug::fmt(v, f),
        }
    }
}

impl Display for ValkyrieASTKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValkyrieASTKind::Statement(_) => {
                todo!()
            }
            ValkyrieASTKind::Namespace(_) => {
                todo!()
            }
            ValkyrieASTKind::Binary(_) => {
                todo!()
            }
            ValkyrieASTKind::Unary(_) => {
                todo!()
            }
            ValkyrieASTKind::HDict(_) => {
                todo!()
            }
            ValkyrieASTKind::HList(_) => {
                todo!()
            }
            ValkyrieASTKind::Namepath(_) => {
                todo!()
            }
            ValkyrieASTKind::Decimal(v) => Display::fmt(v, f),
            ValkyrieASTKind::Integer(v) => Display::fmt(v, f),
            ValkyrieASTKind::Boolean(v) => match v {
                true => f.write_str("true"),
                false => f.write_str("false"),
            },
            ValkyrieASTKind::Null => f.write_str("null"),
            ValkyrieASTKind::Bytes(v) => {
                write!(f, "0x")?;
                for byte in v {
                    write!(f, "{:02x}", byte)?;
                }
                Ok(())
            }
            ValkyrieASTKind::StringInterpolation(_) => {
                todo!()
            }
            ValkyrieASTKind::String(_) => {
                todo!()
            }
            ValkyrieASTKind::Class(_) => {
                todo!()
            }
            ValkyrieASTKind::For(_) => {
                todo!()
            }
            ValkyrieASTKind::While(_) => {
                todo!()
            }
            ValkyrieASTKind::Match(_) => {
                todo!()
            }
            ValkyrieASTKind::Which(_) => {
                todo!()
            }
        }
    }
}

impl ValkyrieASTKind {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: self, span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
