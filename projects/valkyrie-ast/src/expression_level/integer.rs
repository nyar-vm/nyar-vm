use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieIntegerNode {
    pub hint: ValkyrieIdentifierNode,
    pub value: BigInt,
}

impl ValkyrieIntegerNode {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode {
            kind: ValkyrieASTKind::Integer(box self),
            span: FileSpan { file, head: range.start, tail: range.end },
        }
    }
}
