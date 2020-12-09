use super::*;

impl Display for ASTNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("AST").field("kind", &self.kind).finish()
    }
}

impl Debug for ASTNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        todo!()
    }
}

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::Nothing, range: Default::default() }
    }
}

impl From<ASTKind> for ASTNode {
    fn from(kind: ASTKind) -> Self {
        Self { kind, range: Default::default() }
    }
}
