use super::*;

impl UnaryExpression {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: ValkyrieASTKind::Unary(box self), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
