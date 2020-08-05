use super::*;

///
pub struct StringLiteral {
    handler: Option<ASTNode>,
    value: Box<ASTNode>,
}
