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
            ASTKind::Suite(v) => {
                f.write_str("AST::Suite: ")?;
                f.debug_list().entries(v.iter()).finish()
            }
            ASTKind::Expression { base, eos } => f
                .debug_struct("AST::Expression:")
                .field("expr", base)
                .field("eos", eos)
                .field("range", &range) // force format
                .finish(),
            ASTKind::CallInfix(v) => f
                .debug_struct("AST::CallInfix")
                .field("chain", &v)
                .field("range", &range) // force format
                .finish(),
            ASTKind::CallUnary(v) => f
                .debug_struct("AST::CallUnary")
                .field("base", &v.base)
                .field("prefix", &v.prefix)
                .field("suffix", &v.suffix)
                .field("range", &range) // force format
                .finish(),
            ASTKind::CallSlice(v) => f.debug_struct("AST::CallSlice").field("base", &v.base).field("terms", &v.terms).finish(),
            ASTKind::ListExpression(v) => f.debug_list().entries(v.iter()).finish(),
            ASTKind::Symbol(v) => write!(f, "{}", v),
            ASTKind::Boolean(v) => write!(f, "{}", v),
            ASTKind::ByteLiteral(v) => write!(f, "{}", v),
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
