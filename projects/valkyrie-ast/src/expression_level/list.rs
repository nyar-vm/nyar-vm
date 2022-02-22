use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeterogeneousList {
    pub consistent: bool,
    pub nodes: Vec<ValkyrieASTNode>,
}

impl HeterogeneousList {
    pub fn pair(key: ValkyrieASTNode, value: ValkyrieASTNode) -> Self {
        Self { consistent: false, nodes: vec![key, value] }
    }
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTKind::HList(box self).to_node(file, range)
    }
}
