use super::*;
use lsp_types::Range;

#[derive(Debug, Clone)]
pub struct ASTNode {
    pub kind: ASTKind,
    pub range: Range,
}

impl ASTNode {}
