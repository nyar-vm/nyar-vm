mod assign;
mod atoms;
mod chain;
mod control;
mod display;
mod function;
mod infix;
mod let_bind;

pub use self::{
    atoms::{
        byte_literal::ByteLiteral, comment_literal::CommentLiteral, kv_pair::KVPair, number_literal::NumberLiteral,
        operator::Operator, string_literal::StringLiteral, symbol::Symbol, *,
    },
    chain::*,
    control::*,
};
use crate::ast::dict_literal::DictLiteral;
pub use crate::ast::{assign::ImportStatement, function::LambdaFunction, infix::BinaryExpression, let_bind::LetBind};
use nyar_error::Span;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::AddAssign,
};

pub type Range = std::ops::Range<u32>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct ASTNode {
    pub(crate) kind: ASTKind,
    pub(crate) meta: ASTMeta,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ASTMeta {
    /// span start offset
    pub span: Span,
    /// comment documentations
    pub document: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub enum ASTKind {
    /// Wrong node
    Nothing,
    ///
    Program(Vec<ASTNode>),
    ///
    Sequence(Vec<ASTNode>),
    ///
    LetBind(Box<LetBind>),
    /// Lambda Function
    LambdaFunction(Box<LambdaFunction>),

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
    String(Box<StringLiteral>),
    /// String Template
    StringTemplate(Vec<ASTNode>),
    /// XML Template
    XMLTemplate(Vec<ASTNode>),
    Symbol(Box<Symbol>),
}

impl ASTNode {
    pub fn kind(&self) -> &ASTKind {
        &self.kind
    }
    pub fn start(self) -> u32 {
        self.meta.span.start
    }
    pub fn end(self) -> u32 {
        self.meta.span.end
    }
}

impl ASTNode {
    pub fn program(v: Vec<ASTNode>, meta: ASTMeta) -> Self {
        Self { kind: ASTKind::Program(v), meta }
    }
    pub fn suite(v: Vec<ASTNode>, meta: ASTMeta) -> Self {
        todo!()
        // Self { kind: ASTKind::Suite(v), meta }
    }

    pub fn empty_block() -> Self {
        todo!()
    }

    pub fn if_statement(pairs: Vec<(ASTNode, ASTNode)>, default: Option<ASTNode>, meta: ASTMeta) -> Self {
        todo!()
        // let s = IfStatement { pairs, default };
        // Self { kind: ASTKind::IfStatement(box s), meta }
    }

    pub fn expression(base: ASTNode, eos: bool, meta: ASTMeta) -> Self {
        todo!()
        // Self { kind: ASTKind::Expression { base: box base, eos }, meta }
    }

    pub fn string_expression(h: &str, v: &[ASTNode], meta: ASTMeta) -> Self {
        todo!()
        // let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        // let v = StringLiteral { handler, value: Vec::from(v) };
        // Self { kind: ASTKind::StringExpression(box v), meta }
    }

    pub fn push_infix_chain(self, op: &str, rhs: ASTNode, meta: ASTMeta) -> Self {
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

    pub fn push_unary_operations(self, prefix: &[String], suffix: &[String], meta: ASTMeta) -> Self {
        todo!()
        // if prefix.is_empty() && suffix.is_empty() {
        //     return self.refine();
        // }
        // let mut unary = match self.kind {
        //     ASTKind::CallUnary(u) => *u,
        //     _ => UnaryCall::new(self),
        // };
        // unary.push_prefix(prefix);
        // unary.push_suffix(suffix);
        // Self { kind: ASTKind::CallUnary(box unary), meta }
    }

    pub fn chain_join(self, terms: ASTNode) -> Self {
        ChainCall::join_chain_terms(self, &[terms])
    }

    pub fn apply_call(args: Vec<ASTNode>, meta: ASTMeta) -> Self {
        todo!()
        // ASTNode { kind: ASTKind::CallApply(args), meta }
    }

    pub fn kv_pair(k: ASTNode, v: ASTNode) -> KVPair {
        KVPair { k, v }
    }

    pub fn apply_slice(indexes: &[ASTNode], meta: ASTMeta) -> Self {
        todo!()
        // let kind = SliceTerm { terms: Vec::from(indexes) };
        // ASTNode { kind: ASTKind::CallSlice(box kind), meta }
    }

    pub fn apply_index(start: Option<ASTNode>, end: Option<ASTNode>, steps: Option<ASTNode>, meta: ASTMeta) -> Self {
        todo!()
        // let kind = IndexTerm { start, end, steps };
        // ASTNode { kind: ASTKind::CallIndex(box kind), meta }
    }

    pub fn list(v: Vec<ASTNode>, meta: ASTMeta) -> Self {
        Self { kind: ASTKind::ListExpression(v), meta }
    }

    pub fn dict(v: DictLiteral, meta: ASTMeta) -> Self {
        Self { kind: ASTKind::DictExpression(box v), meta }
    }

    pub fn tuple(v: Vec<ASTNode>, meta: ASTMeta) -> Self {
        Self { kind: ASTKind::TupleExpression(v), meta }
    }

    pub fn symbol(symbol: Symbol, meta: ASTMeta) -> Self {
        Self { kind: ASTKind::Symbol(box symbol), meta }
    }

    pub fn number(literal: &str, handler: &str, meta: ASTMeta) -> Self {
        let v = NumberLiteral { handler: handler.to_string(), value: literal.to_string() };
        Self { kind: ASTKind::Number(box v), meta }
    }

    pub fn bytes(literal: &str, mode: &str, meta: ASTMeta) -> Self {
        let v = NumberLiteral { handler: mode.to_string(), value: literal.to_string() };
        Self { kind: ASTKind::Number(box v), meta }
    }

    pub fn string(literal: &str, meta: ASTMeta) -> Self {
        let s = StringLiteral { handler: String::new(), literal: literal.to_string() };
        Self { kind: ASTKind::String(box s), meta }
    }

    pub fn string_handler(literal: &str, handler: &str, meta: ASTMeta) -> ASTNode {
        let s = StringLiteral { handler: handler.to_string(), literal: literal.to_string() };
        Self { kind: ASTKind::String(box s), meta }
    }

    pub fn boolean(v: bool, meta: ASTMeta) -> Self {
        Self { kind: ASTKind::Boolean(v), meta }
    }

    pub fn null(meta: ASTMeta) -> Self {
        Self { kind: ASTKind::Nothing, meta }
    }
}

#[test]
fn check_size() {
    assert_eq!(std::mem::size_of::<String>(), 24);
    assert_eq!(std::mem::size_of::<ASTKind>(), 32);
    assert_eq!(std::mem::size_of::<ASTNode>(), 40);
}
