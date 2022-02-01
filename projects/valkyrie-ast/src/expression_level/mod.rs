use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::{HeterogeneousList, ValkyrieASTKind, ValkyrieASTNode, ValkyrieIdentifierNode, ValkyrieOperator};
use std::fmt::{Display, Formatter};
use valkyrie_errors::{FBig, FileID, FileSpan, IBig};

pub mod binary;
pub mod decimal;
pub mod dict;
pub mod identifier;
pub mod integer;
pub mod list;
pub mod unary;

impl ValkyrieASTNode {
    pub fn null(file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::Null.to_node(file, range)
    }
    pub fn boolean(b: bool, file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::Boolean(b).to_node(file, range)
    }
    pub fn tuple(nodes: Vec<ValkyrieASTNode>, file: FileID, range: &Range<usize>) -> Self {
        HeterogeneousList { consistent: false, nodes }.to_node(file, range)
    }
    pub fn list(nodes: Vec<ValkyrieASTNode>, file: FileID, range: &Range<usize>) -> Self {
        HeterogeneousList { consistent: true, nodes }.to_node(file, range)
    }
}
