mod atoms;
mod chain;
mod control;
mod import;

pub use self::{kind::ASTKind};
pub use self::import::ImportStatement;
pub use self::{atoms::*, chain::*, control::*};

use lsp_types::Range;
use std::ops::AddAssign;
use crate::ast::kind::chain::CallChain;

#[derive(Debug, Clone)]
pub struct ASTNode {
    pub kind: ASTKind,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub enum ASTKind {
    Program(Vec<ASTKind>),
    Suite(Vec<ASTKind>),
    /// - `EmptyStatement`: Skip
    EmptyStatement,
    /// - `ImportStatement`
    ImportStatement(Box<ImportStatement>),
    IfStatement(Box<IfStatement>),
    LetBinding,
    /// - `Expression`
    Expression {},
    /// - `Expression`
    TypeExpression {},
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
    NumberLiteral(Box<NumberLiteral>),
    /// - `Number`: raw number represent
    ByteLiteral(Box<ByteLiteral>),
    ///
    StringLiteral(Box<StringLiteral>),
    /// - `Comment`: raw comment with handler
    Comment(Box<CommentLiteral>),
    ///
    Boolean(bool),
    /// - `None`: It doesn't look like anything to me
    None,
}
