use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinaryExpression {}


impl BinaryExpression {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: ValkyrieASTKind::Binary(box self), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
