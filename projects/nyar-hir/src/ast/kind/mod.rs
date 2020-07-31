use std::ops::AddAssign;

pub use self::import::ImportStatement;
use super::*;
use crate::ast::kind::chain::CallChain;

mod chain;
mod atoms;
mod import;
mod control;

pub use self::atoms::*;
pub use self::chain::*;

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
    },
    /// - `Expression`
    TypeExpression {
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
    CallSlice(Box<SliceCall>),
    ///
    /// ```v
    /// expr(index)
    /// ```
    CallApply(Box<ApplyCall>),
    ///
    /// ```v
    /// expr + rhs1 + rhs2
    /// ```
    CallInfix(Box<InfixCall>),
    /// - `UnaryOperators`
    ///     - `base`
    ///
    /// ```v
    /// ++expr!!
    /// ```
    CallUnary(Box<UnaryCall>),
    /// - `Symbol`
    Symbol(Box<Symbol>),
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
