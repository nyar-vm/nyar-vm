use super::*;

impl Display for ASTNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("AST").field("kind", &self.kind).finish()
    }
}

impl Debug for ASTNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("AST")
            .field("kind", &self.kind)
            .field(
                "range",
                &format!(
                    "({}:{}, {}:{})",
                    self.range.start.line, self.range.start.character, self.range.end.line, self.range.end.character
                ),
            )
            .finish()
    }
}

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::None, range: Default::default() }
    }
}
