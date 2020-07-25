pub use self::import::ImportStatement;
use super::*;

mod chain;
mod import;

#[derive(Debug, Clone)]
pub enum ASTKind {
    Program(Vec<ASTKind>),
    Suite(Vec<ASTKind>),
    /// - `EmptyStatement`: Skip
    EmptyStatement,
    /// - `ImportStatement`
    ImportStatement(Box<ImportStatement>),
    IfStatement {
        pairs: Vec<(ASTKind, ASTKind)>,
        default: Option<Box<ASTKind>>,
        annotations: Option<Box<ASTKind>>,
    },
    LetBinding {
        symbol: Box<ASTKind>,
        modifiers: Vec<String>,
        types: Box<ASTKind>,
        annotations: Option<Box<ASTKind>>,
    },
    /// - `Expression`
    Expression {
        base: Box<ASTKind>,
        eos: bool,
        annotations: Option<Box<ASTKind>>,
    },
    /// - `Expression`
    TypeExpression {
        base: Box<ASTKind>,
        annotations: Option<Box<ASTKind>>,
    },
    CallChain(Box<CallChain>),
    ///
    ListExpression(Vec<ASTKind>),
    ///
    TupleExpression(Vec<ASTKind>),
    /// - `UnaryOperators`
    ///     - `base`
    UnaryOperators {
        base: Box<ASTKind>,
        prefix: Vec<String>,
        suffix: Vec<String>,
    },
    /// - `InfixOperators`
    InfixOperators {
        o: String,
        lhs: Box<ASTKind>,
        rhs: Box<ASTKind>,
    },
    /// - `SliceExpression`
    /// the terms must `IndexExpression`
    SliceExpression {
        base: Box<ASTKind>,
        list: Vec<ASTKind>,
    },
    IndexExpression(IndexExpression),
    ApplyExpression {
        base: Box<ASTKind>,
        types: Vec<ASTKind>,
        args: Vec<ASTKind>,
        kv_pairs: Vec<(ASTKind, ASTKind)>,
    },
    /// - `Symbol`
    Symbol {
        name: String,
        scope: Vec<String>,
    },
    /// - `Number`: raw number represent
    NumberLiteral {
        handler: String,
        data: String,
    },
    ///
    Number(Number),
    ///
    StringLiteral {
        handler: String,
        data: String,
    },
    /// - `String`: raw string
    String(String),
    /// - `Comment`: raw comment with handler
    CommentLiteral {
        handler: String,
        data: String,
    },
    ///
    Boolean(bool),
    /// - `None`: It doesn't look like anything to me
    None,
}
