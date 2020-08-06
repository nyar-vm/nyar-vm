use super::*;

///
#[derive(Debug, Clone)]
pub struct CommentLiteral {
    kind: Option<ASTNode>,
    value: Box<ASTNode>,
}
