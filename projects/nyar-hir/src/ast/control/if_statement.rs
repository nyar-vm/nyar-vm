use super::*;

#[derive(Debug, Clone)]
pub struct IfStatement {
    pairs: Vec<(ASTKind, ASTKind)>,
    default: Option<Box<ASTKind>>,
    annotations: Option<Box<ASTKind>>,
}
