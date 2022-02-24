use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForLoop {
    pattern: ValkyriePattern,
    iterator: ValkyrieASTNode,
    body: Vec<ValkyrieASTNode>,
    otherwise: Vec<ValkyrieASTNode>,
}

impl ForLoop {
    pub fn new(pattern: ValkyriePattern, iterator: ValkyrieASTNode) -> Self {
        Self { pattern, iterator, body: vec![], otherwise: vec![] }
    }
    pub fn get_pattern(&self) -> &ValkyriePattern {
        &self.pattern
    }
    pub fn get_iterator(&self) -> &ValkyrieASTNode {
        &self.iterator
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
        ValkyrieASTKind::For(box self.clone()).to_node(file, range)
    }
}
