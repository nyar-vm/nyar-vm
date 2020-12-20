use super::*;

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::Nothing, meta: Default::default() }
    }
}

impl Default for ASTMeta {
    fn default() -> Self {
        Self { span: Default::default(), document: String::new() }
    }
}

impl From<ASTKind> for ASTNode {
    fn from(kind: ASTKind) -> Self {
        Self { kind, meta: Default::default() }
    }
}

impl Debug for ASTNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.kind, f)
    }
}

impl Debug for ASTKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        todo!()
    }
}
