use std::{
    fmt::{Debug, Display, Formatter},
    ops::{AddAssign, Deref},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use nyar_error::Span;

pub use crate::ast::{
    assign::ImportStatement,
    atoms::{
        number_literal::{ByteLiteral, DecimalLiteral, IntegerLiteral},
        string_literal::StringLiteral,
        string_template::StringTemplateBuilder,
        symbol::Symbol,
        table_literal::{KVPair, TableExpression},
    },
    chain::*,
    control::*,
    expression::{infix::InfixCall, Expression},
    function::LambdaFunction,
    let_bind::LetBind,
    looping::{LoopStatement, WhileLoop},
    operator::{Infix, Operator, Postfix, Prefix},
};
use crate::utils::write_tuple;
use std::collections::BTreeMap;
mod assign;
mod atoms;
mod chain;
mod checking;
mod control;
mod expression;
mod function;
mod let_bind;
mod looping;
mod operator;

///
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ASTNode {
    /// The kind of this ast node
    pub kind: ASTKind,
    /// The range and file of this ast node
    pub span: Span,
}

///
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ASTKind {
    /// Wrong node
    Nothing,
    /// Entry point of codes
    Program(Vec<ASTNode>),
    /// A block with new scope
    Suite(Vec<ASTNode>),
    /// A block without new scope
    Sequence(Vec<ASTNode>),
    /// let lazy (mut a, b) = (1, 2)
    LetBind(Box<LetBind>),
    /// Lambda Function
    LambdaFunction(Box<LambdaFunction>),
    /// ```vk
    /// if cond {
    ///     then
    /// }
    /// else if cond {
    ///     then
    /// }
    /// else {
    ///     then
    /// }
    /// ```
    IfStatement(Box<IfStatement>),
    /// ```vk
    /// loop {
    ///    do some thing
    ///    break
    /// }
    /// ```
    LoopStatement(Box<LoopStatement>),
    /// ```vk
    /// a + b
    /// ```
    InfixExpression(Box<InfixCall>),
    /// ```vk
    /// a::[G](args) {lambda}
    /// ```
    ApplyExpression(Box<ChainCall>),
    /// ```vk
    /// (1, 2, 3)
    /// ```
    TupleExpression(Vec<ASTNode>),
    /// ```vk
    /// [1, 2, 3]
    /// [a: 1, z: 26]
    /// ```
    TableExpression(Box<TableExpression>),
    /// ```vk
    /// a: b
    /// ```
    PairExpression(Box<KVPair>),
    /// Boolean literal, `true` and `false`
    Boolean(bool),
    /// Byte like literal, start with `0x`
    Byte(Box<ByteLiteral>),
    /// Integer literal, number without `.`
    Integer(Box<IntegerLiteral>),
    /// Decimal literal, number with `.`
    Decimal(Box<DecimalLiteral>),
    /// String literal
    String(Box<StringLiteral>),
    /// String Template
    StringTemplate(Vec<ASTNode>),
    /// XML Template
    XMLTemplate(Vec<ASTNode>),
    /// A symbol path, needs context to resolve
    Symbol(Box<Symbol>),
}

impl ASTNode {
    pub fn program(v: Vec<ASTNode>, span: Span) -> Self {
        Self { kind: ASTKind::Program(v), span }
    }
    pub fn block(v: Vec<ASTNode>, span: Span) -> Self {
        Self { kind: ASTKind::Suite(v), span }
    }

    pub fn sequence(v: Vec<ASTNode>, span: Span) -> Self {
        Self { kind: ASTKind::Suite(v), span }
    }

    pub fn if_statement(if_chain: IfStatement, meta: Span) -> Self {
        Self { kind: ASTKind::IfStatement(box if_chain), span: meta }
    }

    pub fn loop_statement(loop_chain: Vec<ASTNode>, meta: Span) -> Self {
        Self { kind: ASTKind::LoopStatement(box LoopStatement { body: loop_chain }), span: meta }
    }

    pub fn while_loop_statement(condition: ASTNode, body: Vec<ASTNode>, span: Span) -> Self {
        WhileLoop::new(condition, body, span)
    }

    pub fn while_else_statement(condition: ASTNode, body: Vec<ASTNode>, else_trigger: Vec<ASTNode>, span: Span) -> Self {
        WhileLoop::while_else(condition, body, else_trigger, span)
    }

    pub fn expression(base: ASTNode, eos: bool) -> Expression {
        Expression { base, eos }
    }

    pub fn push_infix_chain(self, op: &str, rhs: ASTNode, span: Span) -> Self {
        let op = Operator::parse_infix(op);
        let mut infix = match self.kind {
            ASTKind::InfixExpression(e) if op.get_priority() == e.get_priority() => *e,
            _ => InfixCall { base: self, terms: vec![] },
        };
        infix.push_infix_pair(op, rhs);
        Self { kind: ASTKind::InfixExpression(box infix), span }
    }

    pub fn kv_pair(k: ASTNode, v: ASTNode) -> KVPair {
        KVPair { key: k, value: v }
    }

    pub fn list(v: Vec<ASTNode>, meta: Span) -> Self {
        let table = TableExpression { inner: v };
        Self { kind: ASTKind::TableExpression(box table), span: meta }
    }

    pub fn tuple(v: Vec<ASTNode>, meta: Span) -> Self {
        Self { kind: ASTKind::TupleExpression(v), span: meta }
    }

    pub fn symbol(symbol: Symbol, meta: Span) -> Self {
        Self { kind: ASTKind::Symbol(box symbol), span: meta }
    }
    pub fn control_break(meta: Span) -> Self {
        let symbol = Symbol::atom("break");
        Self { kind: ASTKind::Symbol(box symbol), span: meta }
    }

    pub fn number(n: IntegerLiteral, meta: Span) -> Self {
        Self { kind: ASTKind::Integer(box n), span: meta }
    }

    pub fn bytes<S>(literal: S, mode: char, meta: Span) -> Self
    where
        S: Into<String>,
    {
        let v = ByteLiteral { handler: mode, value: literal.into() };
        Self { kind: ASTKind::Byte(box v), span: meta }
    }

    pub fn string<S>(literal: S, meta: Span) -> Self
    where
        S: Into<String>,
    {
        let s = StringLiteral { handler: String::new(), literal: literal.into() };
        Self { kind: ASTKind::String(box s), span: meta }
    }

    pub fn string_handler<S>(literal: S, handler: &str, span: Span) -> ASTNode
    where
        S: Into<String>,
    {
        let s = StringLiteral { handler: handler.to_string(), literal: literal.into() };
        Self { kind: ASTKind::String(box s), span }
    }
    pub fn string_template(nodes: Vec<ASTNode>, span: Span) -> ASTNode {
        Self { kind: ASTKind::StringTemplate(nodes), span }
    }

    pub fn boolean(v: bool, meta: Span) -> Self {
        Self { kind: ASTKind::Boolean(v), span: meta }
    }

    pub fn null(meta: Span) -> Self {
        Self { kind: ASTKind::Nothing, span: meta }
    }
}

#[test]
fn check_size() {
    assert_eq!(std::mem::size_of::<String>(), 24);
    assert_eq!(std::mem::size_of::<ASTKind>(), 32);
    assert_eq!(std::mem::size_of::<ASTNode>(), 40);
}
