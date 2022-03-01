use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhichCase {
    branch: Vec<WhichBranch>,
    default: Vec<ValkyrieASTNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhichBranch {
    pub pattern: ValkyriePattern,
    pub actions: Vec<ValkyrieASTNode>,
}

// if a {}
// if case A() := b {}

impl WhichCase {
    pub fn get_branch(&self) -> Iter<'_, WhichBranch> {
        self.branch.iter()
    }
    pub fn mut_branch(&mut self) -> &mut Vec<WhichBranch> {
        &mut self.branch
    }
    pub fn get_default(&self) -> Iter<'_, ValkyrieASTNode> {
        self.default.iter()
    }
    pub fn mut_default(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.default
    }
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTKind::Which(box self).to_node(file, range)
    }
}
