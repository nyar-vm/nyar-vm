#[derive(Debug, Clone, PartialEq)]
pub enum AST {
    /// - `None`: It doesn't look like anything to me
    None,
    /// - `Program`:
    Program(Vec<AST>),
    /// - `EmptyStatement`: Nothing
    EmptyStatement,
}