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
        let file = lhs.span.file;
        let head = lhs.span.head;
        let tail = rhs.span.tail;
        let binary = match (lhs, rhs) {
            (ValkyrieASTKind::Binary(mut a), ValkyrieASTKind::Binary(b)) => {
                a.extend(op, b.base);
                a.terms.extend(b.terms);
                a
            }
            (ValkyrieASTKind::Binary(mut a), b) => {
                a.extend(op, b);
                a
            }
            (a, ValkyrieASTKind::Binary(b)) => {
                let mut new = Self::new(a, op, b.base);
                new.terms.extend(b.terms);
                new
            }
            (a, b) => Self::new(a, op, b),
        };
        binary.to_node(file, &Range { start: head, end: tail })
    }

    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: ValkyrieASTKind::Binary(box self), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
