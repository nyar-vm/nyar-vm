use super::*;

///
#[derive(Debug, Clone)]
pub struct StringLiteral {
    handler: Option<StringRange>,
    value: Box<ASTNode>,
}
