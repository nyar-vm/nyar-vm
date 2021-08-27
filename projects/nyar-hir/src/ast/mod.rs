use nyar_error::{
    third_party::{num::Zero, BigDecimal},
    NyarError3,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display, Formatter},
    ops::{AddAssign, Deref},
    str::FromStr,
};

use nyar_error::Span;

pub use crate::ast::{
    assign::{
        assign::{AssignBlock, LetBindSimple, LetBindStatement},
        import::{ImportBuilder, ImportStatement},
        match_statement::{BuilderStatement, CatchStatement, MatchStatement},
    },
    atoms::{
        byte_literal::ByteLiteral,
        decimal_literal::DecimalLiteral,
        integer_literal::IntegerLiteral,
        string_literal::StringLiteral,
        string_template::StringTemplateBuilder,
        symbol::{Symbol, SymbolNode},
        table_literal::{KVPair, TableExpression},
        xml_template::XMLTemplateBuilder,
    },
    chain::*,
    control::*,
    definition::{
        function::{FunctionDefinition, FunctionParameter, FunctionParameterKind},
        LetBind,
    },
    expression::{infix::InfixCall, Expression},
    function::LambdaFunction,
    looping::{ForInLoop, LoopStatement, WhileLoop},
    operator::{Infix, Operator, OperatorAssociativity, Postfix, Prefix},
};

mod assign;
mod atoms;
mod chain;
mod checking;
mod control;
mod definition;
mod expression;
mod function;
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
    /// Import
    ImportStatement(Box<ImportStatement>),
    /// let lazy (mut a, b) = (1, 2)
    LetBind(Box<LetBind>),
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
    DefineFunction(Box<FunctionDefinition>),
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

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::Nothing, span: Default::default() }
    }
}

impl From<ASTKind> for ASTNode {
    fn from(kind: ASTKind) -> Self {
        Self { kind, span: Default::default() }
    }
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

    pub fn loop_statement(loop_chain: Vec<ASTNode>, meta: Span) -> Self {
        Self { kind: ASTKind::LoopStatement(box LoopStatement { body: loop_chain }), span: meta }
    }

    pub fn expression(base: ASTNode, eos: bool) -> Expression {
        Expression { base, eos }
    }

    pub fn push_infix_chain(self, op: &str, rhs: ASTNode) -> Self {
        let op = Operator::parse_infix(op);
        let infix = match self.kind {
            ASTKind::InfixExpression(mut e) if op == e.operator => {
                e.terms.push(rhs);
                *e
            }
            _ => InfixCall::new(self, op, rhs),
        };
        infix.as_node()
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
