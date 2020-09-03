use super::*;

impl Display for ASTNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("AST").field("kind", &self.kind).finish()
    }
}

impl Debug for ASTNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let range = format!(
            "({}:{}, {}:{})",
            self.range.start.line, self.range.start.character, self.range.end.line, self.range.end.character
        );
        match &self.kind {
            ASTKind::Expression { base, eos } => f
                .debug_struct("AST::Expression")
                .field("base", base)
                .field("eos", eos)
                .field("range", &range) // force format
                .finish(),
            ASTKind::CallUnary(v) => f
                .debug_struct("AST::CallUnary")
                .field("base", &v.base)
                .field("prefix", &v.prefix)
                .field("suffix", &v.suffix)
                .field("range", &range) // force format
                .finish(),
            ASTKind::Boolean(v) => f
                .debug_struct("AST::Boolean")
                .field("value", v)
                .field("range", &range) //
                .finish(),
            ASTKind::ByteLiteral(v) => f
                .debug_struct("AST::ByteLiteral")
                .field("value", &v.value)
                .field("handler", &v.handler)
                .field("range", &range) //
                .finish(),
            ASTKind::NumberLiteral(v) => write!(f, "{}", v),
            _ => f
                .debug_struct("AST")
                .field("kind", &self.kind)
                .field("range", &range) //
                .finish(),
        }
    }
}

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::None, range: Default::default() }
    }
}
