mod assign;
mod atoms;
mod chain;
mod control;
mod display;
mod cps;
mod let_bind;

pub use self::{assign::ImportStatement, atoms::*, chain::*, control::*};
use rkyv::{Archive, Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::AddAssign,
};
use crate::ast::let_bind::LetBind;

pub type Range = std::ops::Range<u32>;

#[derive(Clone, Archive, Deserialize, Serialize)]
pub struct ASTNode {
    kind: ASTKind,
    meta: ASTMeta,
}

#[derive(Clone, Archive, Deserialize, Serialize)]
pub struct ASTMeta {
    /// span start offset
    pub start: u32,
    /// span end offset
    pub end: u32,
    /// same as ptr size, 0 refers to `<anonymous file>`
    pub file_id: u64,
    /// comment documentations
    pub document: String,
}

#[derive(Clone, Archive, Deserialize, Serialize)]
pub enum ASTKind {
    /// Wrong node
    Nothing,
    ///
    LetBind(Box<LetBind>),
    ///
    ASTAtom(Box<ASTAtom>),
}

impl ASTNode {
    pub fn kind(&self) -> &ASTKind {
        &self.kind
    }
    pub fn start(self) -> u32 {
        self.meta.start
    }
    pub fn end(self) -> u32 {
        self.meta.end
    }
}

impl ASTNode {
    pub fn empty_statement(r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::EmptyStatement, range: r }
    }
    pub fn suite(v: Vec<ASTNode>, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::Suite(v), range: r }
    }

    pub fn if_statement(pairs: Vec<(ASTNode, ASTNode)>, default: Option<ASTNode>, r: Range) -> Self {
        todo!()
        // let s = IfStatement { pairs, default };
        // Self { kind: ASTKind::IfStatement(box s), range: r }
    }

    pub fn expression(base: ASTNode, eos: bool, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::Expression { base: box base, eos }, range: r }
    }

    pub fn string_expression(h: &str, v: &[ASTNode], r: Range) -> Self {
        todo!()
        // let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        // let v = StringLiteral { handler, value: Vec::from(v) };
        // Self { kind: ASTKind::StringExpression(box v), range: r }
    }

    pub fn push_infix_chain(self, op: &str, rhs: ASTNode, r: Range) -> Self {
        todo!()
        // let op = Operator::parse(op, 0);
        //
        // let mut infix = match self.kind {
        //     ASTKind::CallInfix(e) if op.get_priority() == e.get_priority() => *e,
        //     _ => InfixCall { base: self, terms: vec![] },
        // };
        // infix.push_infix_pair(op, rhs);
        // Self { kind: ASTKind::CallInfix(box infix), range: r }
    }

    pub fn push_unary_operations(self, prefix: &[String], suffix: &[String], r: Range) -> Self {
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
        // Self { kind: ASTKind::CallUnary(box unary), range: r }
    }

    pub fn chain_join(self, terms: ASTNode) -> Self {
        ChainCall::join_chain_terms(self, &[terms])
    }

    pub fn apply_call(args: Vec<ASTNode>, r: Range) -> Self {
        todo!()
        // ASTNode { kind: ASTKind::CallApply(args), range: r }
    }

    pub fn kv_pair(k: ASTNode, v: ASTNode) -> ASTNode {
        todo!()
        // let start = k.range.start;
        // let end = v.range.end;
        // let kind = KVPair { k, v };
        // ASTNode { kind: ASTKind::KVPair(box kind), range: Range { start, end } }
    }

    pub fn apply_slice(indexes: &[ASTNode], r: Range) -> Self {
        todo!()
        // let kind = SliceTerm { terms: Vec::from(indexes) };
        // ASTNode { kind: ASTKind::CallSlice(box kind), range: r }
    }

    pub fn apply_index(start: Option<ASTNode>, end: Option<ASTNode>, steps: Option<ASTNode>, r: Range) -> Self {
        todo!()
        // let kind = IndexTerm { start, end, steps };
        // ASTNode { kind: ASTKind::CallIndex(box kind), range: r }
    }

    pub fn list(v: Vec<ASTNode>, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::ListExpression(v), range: r }
    }

    pub fn dict(v: Vec<ASTNode>, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::DictExpression(v), range: r }
    }

    pub fn tuple(v: Vec<ASTNode>, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::TupleExpression(v), range: r }
    }

    pub fn symbol(path: &[String], r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::Symbol(box Symbol::path(path)), range: r }
    }

    pub fn number(h: &str, v: &str, is_integer: bool, r: Range) -> Self {
        todo!()
        // let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        // let v = NumberLiteral { handler, value: String::from(v), is_integer };
        // Self { kind: ASTKind::NumberLiteral(box v), range: r }
    }

    pub fn bytes(h: char, v: &str, r: Range) -> Self {
        todo!()
        // let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        // let v = ByteLiteral { handler: h, value: String::from(v) };
        // Self { kind: ASTKind::ByteLiteral(box v), range: r }
    }

    pub fn string(s: &str, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::String(String::from(s)), range: r }
    }

    pub fn boolean(v: bool, r: Range) -> Self {
        todo!()
        // Self { kind: ASTKind::Boolean(v), range: r }
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
