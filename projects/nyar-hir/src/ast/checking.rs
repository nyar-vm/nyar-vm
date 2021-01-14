use super::*;

impl Deref for ASTNode {
    type Target = ASTKind;
    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

impl ASTNode {
    pub fn kind(&self) -> &ASTKind {
        &self.kind
    }
}

impl ASTKind {
    pub fn is_true(&self) -> bool {
        matches!(self, ASTKind::Boolean(true))
    }
    pub fn is_false(&self) -> bool {
        matches!(self, ASTKind::Boolean(false))
    }
    pub fn is_boolean(&self) -> bool {
        matches!(self, ASTKind::Boolean(_))
    }
}
