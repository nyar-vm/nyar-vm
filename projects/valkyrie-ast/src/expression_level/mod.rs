use std::ops::Range;

use serde::{Deserialize, Serialize};

use valkyrie_errors::{BigInt, FileID, FileSpan};

use crate::{ValkyrieASTKind, ValkyrieASTNode, ValkyrieIdentifierNode};
use crate::expression_level::list::HeterogeneousList;

mod atomic;
mod binary;
pub mod identifier;
pub mod integer;
pub mod list;
mod unary;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinaryExpression {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnaryExpression {}

impl ValkyrieASTNode {
    pub fn null(file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::Null.to_node(file, range)
    }
    pub fn boolean(b: bool, file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::Boolean(b).to_node(file, range)
    }
    pub fn tuple(nodes: Vec<ValkyrieASTNode>, file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::HList(box HeterogeneousList {
            consistent: false,
            nodes,
        }).to_node(file, range)
    }
}
