use std::ops::Range;

use valkyrie_errors::FileID;

use crate::ValkyrieASTKind;

use super::*;

// while a {
//     b
// }
// else {
//     c
// }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhileLoop {
    condition: ValkyrieASTNode,
    body: Vec<ValkyrieASTNode>,
    otherwise: Vec<ValkyrieASTNode>,
}

impl WhileLoop {
    pub fn new(condition: ValkyrieASTNode) -> Self {
        Self { condition, body: vec![], otherwise: vec![] }
    }
    pub fn get_condition(&self) -> &ValkyrieASTNode {
        &self.condition
    }
    pub fn get_body(&self) -> Iter<'_, ValkyrieASTNode> {
        self.body.iter()
    }
    pub fn mut_body(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.body
    }
    pub fn get_otherwise(&self) -> Iter<'_, ValkyrieASTNode> {
        self.otherwise.iter()
    }
    pub fn mut_otherwise(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.otherwise
    }
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTKind::While(box self.clone()).to_node(file, range)
    }
}
