#[derive(Debug, Clone)]
pub enum IndexExpression {
    None,
    ///
    Single(),
    Normal {
        start: Box<ASTKind>,
        end: Box<ASTKind>,
        step: Box<ASTKind>,
    },
}
