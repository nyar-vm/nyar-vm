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

impl ASTNode {
    pub fn join_span(items: &[Self]) -> Span {
        let head = items.first().map(|f| f.span).unwrap_or_default();
        let tail = items.last().map(|f| f.span).unwrap_or_default();
        Span { start: head.start, end: tail.end, file_id: head.file_id }
    }
}
