#![feature(trivial_bounds)]
#![feature(never_type)]
#![feature(box_syntax)]

use serde::{Deserialize, Serialize};

use valkyrie_errors::FileSpan;

pub use crate::{
    expression_level::{identifier::ValkyrieIdentifierNode, integer::ValkyrieIntegerNode, BinaryExpression, UnaryExpression},
    package_level::{NamespaceDeclare, NamespaceKind},
};
pub use crate::expression_level::list::HeterogeneousList;

mod display;
mod expression_level;
mod package_level;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieASTNode {
    pub kind: ValkyrieASTKind,
    pub span: FileSpan,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ValkyrieASTKind {
    Statement(Vec<ValkyrieASTNode>),
    Namespace(Box<NamespaceDeclare>),
    Binary(Box<BinaryExpression>),
    Unary(Box<UnaryExpression>),
    // ()
    // (1, )
    // (1, 2, 3)
    HList(Box<HeterogeneousList>),
    Identifier(Box<ValkyrieIdentifierNode>),
    Integer(Box<ValkyrieIntegerNode>),
    Boolean(bool),
    Null,
}
