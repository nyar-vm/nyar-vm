use std::ops::AddAssign;

pub use self::import::ImportStatement;
use super::*;
use crate::ast::kind::chain::CallChain;

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
    ///
    ListExpression(Vec<ASTKind>),
    ///
    TupleExpression(Vec<ASTKind>),
    /// - `InfixOperators`
    Operator(Box<Operator>),
    CallChain(Box<CallChain>),
    /// - `SliceCall`
    ///
    /// ```v
    /// expr[index]
    /// ```
    SliceCall {
        base: Box<ASTKind>,
        list: Vec<ASTKind>,
    },
    ///
    /// ```v
    /// expr(index)
    /// ```
    ApplyCall(Box<ApplyCall>),
    ///
    /// ```v
    /// expr + rhs
    /// ```
    InfixCall(Box<InfixCall>),
    /// - `UnaryOperators`
    ///     - `base`
    UnaryCall(Box<UnaryCall>),
    /// - `Symbol`
    Symbol {
        name: String,
        scope: Vec<String>,
    },
    /// - `Number`: raw number represent
    NumberLiteral {
        handler: Option<ASTNode>,
        value: Box<ASTNode>,
    },
    /// - `Number`: raw number represent
    ByteLiteral {
        handler: Option<ASTNode>,
        value: Box<ASTNode>,
    },
    ///
    StringLiteral {
        handler: Option<ASTNode>,
        value: Box<ASTNode>,
    },
    /// - `Comment`: raw comment with handler
    Comment {
        handler: String,
        data: String,
    },
    ///
    Boolean(bool),
    /// - `None`: It doesn't look like anything to me
    None,
}
