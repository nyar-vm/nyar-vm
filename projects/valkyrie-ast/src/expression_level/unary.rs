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
        let file = base.span.file;
        let head = base.span.head;
        let tail = base.span.tail;
        let unary = match base {
            ValkyrieASTKind::Unary(mut a) => {
                a.term.push(op);
                a
            }
            a => Self::new(a, op),
        };
        unary.to_node(file, &Range { start: head, end: tail })
    }

    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode { kind: ValkyrieASTKind::Unary(box self), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}
