use crate::ASTNode;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnaryExpression {
    pub prefix: bool,
    pub op: ASTNode,
}
