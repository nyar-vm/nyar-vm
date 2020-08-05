use super::*;

/// - `Number`: raw number represent
pub struct ByteLiteral {
    handler: Option<ASTNode>,
    value: Box<ASTNode>,
}
