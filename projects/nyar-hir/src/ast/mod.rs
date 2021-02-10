use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::{AddAssign, Deref},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use nyar_error::Span;

pub use crate::ast::{
    assign::ImportStatement,
    atoms::{
        byte_literal::ByteLiteral, dict_literal::DictLiteral, kv_pair::KVPair, number_literal::NumberLiteral,
        string_literal::StringLiteral, string_template::StringTemplateBuilder, symbol::Symbol,
    },
    chain::*,
    control::*,
    expression::Expression,
    function::LambdaFunction,
    infix::BinaryExpression,
    let_bind::LetBind,
    looping::{LoopStatement, WhileLoop},
    operator::{Infix, Operator, Postfix, Prefix},
};

mod assign;
mod atoms;
mod chain;
mod checking;
mod control;
mod display;
mod expression;
mod function;
mod infix;
mod let_bind;
mod looping;
mod operator;

pub type Range = std::ops::Range<u32>;

#[derive(Clone, Serialize, Deserialize)]
pub struct ASTNode {
    pub kind: ASTKind,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ASTKind {
    /// Wrong node
    Nothing,
    ///
    Program(Vec<ASTNode>),
    /// A block with new scope
    Suite(Vec<ASTNode>),
    /// A block without new scope
    Sequence(Vec<ASTNode>),
    ///
    LetBind(Box<LetBind>),
    /// Lambda Function
    LambdaFunction(Box<LambdaFunction>),
    ///
    IfStatement(Box<IfStatement>),
    ///
    LoopStatement(Box<LoopStatement>),
    ///
    InfixExpression(Box<BinaryExpression>),
    /// `(1, 2, 3)`
    TupleExpression(Vec<ASTNode>),
    /// `[1, 2, 3]`
    ListExpression(Vec<ASTNode>),
    ///
    DictExpression(Box<DictLiteral>),
    ///
    Boolean(bool),
    Number(Box<NumberLiteral>),
    Byte(Box<ByteLiteral>),
    String(Box<StringLiteral>),
    /// String Template
    StringTemplate(Vec<ASTNode>),
    /// XML Template
    XMLTemplate(Vec<ASTNode>),
    Symbol(Box<Symbol>),
}

impl ASTNode {
    pub fn program(v: Vec<ASTNode>, span: Span) -> Self {
        Self { kind: ASTKind::Program(v), span }
    }
    pub fn suite(v: Vec<ASTNode>, span: Span) -> Self {
        Self { kind: ASTKind::Suite(v), span }
    }

    pub fn empty_block() -> Self {
        todo!()
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

    pub fn string_expression(_h: &str, _v: &[ASTNode], _meta: Span) -> Self {
        todo!()
        // let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        // let v = StringLiteral { handler, value: Vec::from(v) };
        // Self { kind: ASTKind::StringExpression(box v), meta }
    }

    pub fn push_infix_chain(self, _op: &str, _rhs: ASTNode, _meta: Span) -> Self {
        todo!()
        // let op = Operator::parse(op, 0);
        //
        // let mut infix = match self.kind {
        //     ASTKind::CallInfix(e) if op.get_priority() == e.get_priority() => *e,
        //     _ => InfixCall { base: self, terms: vec![] },
        // };
        // infix.push_infix_pair(op, rhs);
        // Self { kind: ASTKind::CallInfix(box infix), meta }
    }

    pub fn push_unary_operations(self, prefix: &[String], suffix: &[String], span: Span) -> Self {
        return self;
        // if prefix.is_empty() && suffix.is_empty() {
        //     return self.refine();
        // }
        // let mut unary = match self.kind {
        //     ASTKind::CallUnary(u) => *u,
        //     _ => UnaryCall::new(self),
        // };
        // unary.push_prefix(prefix);
        // unary.push_suffix(suffix);
        // Self { kind: ASTKind::CallUnary(box unary), meta: span, span: Default::default() }
    }

    pub fn chain_join(self, terms: ASTNode) -> Self {
        ChainCall::join_chain_terms(self, &[terms])
    }

    pub fn apply_call(_args: Vec<ASTNode>, _meta: Span) -> Self {
        todo!()
        // ASTNode { kind: ASTKind::CallApply(args), meta }
    }

    pub fn kv_pair(k: ASTNode, v: ASTNode) -> KVPair {
        KVPair { k, v }
    }

    pub fn apply_slice(_indexes: &[ASTNode], _meta: Span) -> Self {
        todo!()
        // let kind = SliceTerm { terms: Vec::from(indexes) };
        // ASTNode { kind: ASTKind::CallSlice(box kind), meta }
    }

    pub fn apply_index(_start: Option<ASTNode>, _end: Option<ASTNode>, _steps: Option<ASTNode>, _meta: Span) -> Self {
        todo!()
        // let kind = IndexTerm { start, end, steps };
        // ASTNode { kind: ASTKind::CallIndex(box kind), meta }
    }

    pub fn list(v: Vec<ASTNode>, meta: Span) -> Self {
        Self { kind: ASTKind::ListExpression(v), span: meta }
    }

    pub fn dict(v: DictLiteral, meta: Span) -> Self {
        Self { kind: ASTKind::DictExpression(box v), span: meta }
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

    pub fn number<S>(literal: S, handler: &str, meta: Span) -> Self
    where
        S: Into<String>,
    {
        let v = NumberLiteral { handler: handler.to_string(), value: literal.into() };
        Self { kind: ASTKind::Number(box v), span: meta }
    }

    pub fn bytes<S>(literal: S, mode: char, meta: Span) -> Self
    where
        S: Into<String>,
    {
        let v = ByteLiteral { handler: mode, value: literal.into() };
        Self { kind: ASTKind::Byte(box v), span: meta }
    }

    pub fn string(literal: &str, meta: Span) -> Self {
        let s = StringLiteral { handler: String::new(), literal: literal.to_string() };
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
