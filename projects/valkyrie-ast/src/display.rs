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
            ValkyrieASTKind::Identifier(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Integer(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Boolean(v) => Debug::fmt(v, f),
            ValkyrieASTKind::Null => {
                write!(f, "null")
            }
            ValkyrieASTKind::HList(v) => {
                f.debug_struct("Tuple")
                    .field("nodes", v)
                    .finish()
            }
        }
    }
}

impl Display for ValkyrieASTKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl ValkyrieASTKind {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: self, span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
