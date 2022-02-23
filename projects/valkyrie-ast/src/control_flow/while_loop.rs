use super::*;

// while a {
//     b
// }
// else {
//     c
// }
pub struct WhileLoop {
    pub condition: ValkyrieASTNode,
    body: Vec<ValkyrieASTNode>,
    otherwise: Vec<ValkyrieASTNode>,
}

impl WhileLoop {
    pub fn new(condition: ValkyrieASTNode) -> Self {
        Self { condition, body: vec![], otherwise: vec![] }
    }
    pub fn mut_body(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.body
    }
    pub fn mut_otherwise(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.otherwise
    }
}
