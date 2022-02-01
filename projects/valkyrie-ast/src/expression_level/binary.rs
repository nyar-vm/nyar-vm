use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinaryExpression {
    pub base: ValkyrieASTNode,
    pub terms: Vec<(ValkyrieOperator, ValkyrieASTNode)>,
}

impl BinaryExpression {
    pub fn new(lhs: ValkyrieASTNode, op: ValkyrieOperator, rhs: ValkyrieASTNode) -> Self {
        Self { base: lhs, terms: vec![(op, rhs)] }
    }
    pub fn extend(&mut self, op: ValkyrieOperator, rhs: ValkyrieASTNode) {
        self.terms.push((op, rhs));
    }
    pub fn combine(lhs: ValkyrieASTNode, op: ValkyrieOperator, rhs: ValkyrieASTNode) -> ValkyrieASTNode {
        match (lhs, rhs) {
            (ValkyrieASTKind::Binary(a), ValkyrieASTKind::Binary(b)) => {
                let mut a = *a;
                a.extend(op, *b);
                a.to_node(a.span.file, &Range { start: a.span.head, end: b.span.tail })
            }
            (ValkyrieASTKind::Binary(a), b) => {}
            (a, ValkyrieASTKind::Binary(b)) => {}
            (a, b) => {}
        }
    }

    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: ValkyrieASTKind::Binary(box self), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
