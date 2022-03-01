use crate::ValkyrieOperator;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnaryExpression {
    pub base: ValkyrieASTNode,
    pub term: Vec<ValkyrieOperator>,
}

impl UnaryExpression {
    pub fn new(rhs: ValkyrieASTNode, op: ValkyrieOperator) -> Self {
        Self { base: rhs, term: vec![op] }
    }
    pub fn combine(base: ValkyrieASTNode, op: ValkyrieOperator) -> ValkyrieASTNode {
        let span = base.span;
        let unary = match base.kind {
            ValkyrieASTKind::Unary(mut a) => {
                a.term.push(op);
                a
            }
            a => box Self::new(ValkyrieASTNode { kind: a, span }, op),
        };
        unary.to_node(base.span.file, &Range { start: base.span.head, end: op.span.tail })
    }

    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: ValkyrieASTKind::Unary(box self), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
