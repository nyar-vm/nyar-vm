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
    pub fn start(self) -> u32 {
        self.span.span.start
    }
    pub fn end(self) -> u32 {
        self.span.span.end
    }
    pub fn file_id(self) -> u32 {
        self.span.span.file_id
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
