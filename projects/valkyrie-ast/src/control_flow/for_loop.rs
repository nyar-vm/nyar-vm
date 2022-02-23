use super::*;

pub struct ForLoop {
    pub pattern: ValkyrieASTNode,
    pub iterator: ValkyrieASTNode,
    body: Vec<ValkyrieASTNode>,
    otherwise: Vec<ValkyrieASTNode>,
}

impl ForLoop {
    pub fn mut_body(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.body
    }
    pub fn mut_otherwise(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.otherwise
    }
}
