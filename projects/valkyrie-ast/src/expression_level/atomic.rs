use super::*;

impl ValkyrieASTNode {
    pub fn null(file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::Null.to_node(file, range)
    }
    pub fn boolean(b: bool, file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTKind::Boolean(b).to_node(file, range)
    }
}
