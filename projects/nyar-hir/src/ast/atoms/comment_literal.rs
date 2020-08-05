use super::*;

///
pub struct CommentLiteral {
    kind: Option<ASTNode>,
    value: Box<ASTNode>,
}
