use super::*;

pub struct NumberLiteral {
    handler: Option<ASTNode>,
    value: Box<ASTNode>,
}
