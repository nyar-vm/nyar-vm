use super::*;

#[derive(Debug, Clone)]
pub struct IfStatement {
    pub pairs: Vec<(ASTNode, ASTNode)>,
    pub default: Option<ASTNode>,
    // annotations: Option<Box<ASTKind>>,
}
