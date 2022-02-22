use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeterogeneousDict {
    pub nodes: Vec<HeterogeneousList>,
}

impl HeterogeneousDict {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTKind::HDict(box self).to_node(file, range)
    }
}

impl ValkyrieASTNode {
    pub fn dict(nodes: Vec<HeterogeneousList>, file: FileID, range: &Range<usize>) -> Self {
        HeterogeneousDict { nodes }.to_node(file, range)
    }
}
