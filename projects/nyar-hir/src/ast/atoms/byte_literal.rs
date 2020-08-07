use super::*;

/// - `Number`: raw number represent
#[derive(Debug, Clone)]
pub struct ByteLiteral {
    handler: Option<ASTNode>,
    value: Box<ASTNode>,
}
