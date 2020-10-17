mod atoms;
mod chain;
mod control;
mod import;
mod utils;

pub use self::{atoms::*, chain::*, control::*, import::ImportStatement};

use crate::ast::ASTKind::CallChain;
use lsp_types::Range;
use std::{
    collections::HashMap,
    fmt::{self, Debug, Display, Formatter},
    ops::AddAssign,
};

pub type StringRange = (String, Range);

#[derive(Clone)]
pub struct ASTNode {
    pub kind: ASTKind,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub enum ASTKind {
    /// Wrong node
    None,

    /// Root Node of the AST
    Program(Vec<ASTNode>),
    Suite(Vec<ASTNode>),
    /// - `EmptyStatement`: Skip
    EmptyStatement,
    /// - `ImportStatement`
    ImportStatement(Box<ImportStatement>),
    IfStatement(Box<IfStatement>),
    LetBinding,
    /// - `Expression`
    Expression {
        base: Box<ASTNode>,
        eos: bool,
    },
    /// - `Expression`
    TypeExpression {},
    ///
    ListExpression(Vec<ASTNode>),
    ///
    TupleExpression(Vec<ASTNode>),
    /// - `InfixOperators`
    StringExpression(Box<StringLiteral>),
    Operator(Box<Operator>),
    CallChain(Box<ChainCall>),
    /// - `SliceCall`
    ///
    /// ```v
    /// expr[index]
    /// ```
    CallSlice(Box<SliceTerm>),
    CallIndex(Box<IndexTerm>),
    ///
    /// ```v
    /// expr(index)
    /// ```
    CallApply(Box<ApplyCallTerm>),
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
    /// - `Comment`: raw comment with handler
    Comment(Box<CommentLiteral>),

    /// - `Symbol`
    Symbol(Box<Symbol>),
    /// - `Number`: raw number represent
    NumberLiteral(Box<NumberLiteral>),
    /// - `Number`: raw number represent
    ByteLiteral(Box<ByteLiteral>),
    ///
    String(Box<String>),
    ///
    Boolean(bool),
    /// - `Null`: It doesn't look like anything to me
    Null,
}

impl ASTNode {
    pub fn refine(self) -> Self {
        match self.kind {
            ASTKind::CallUnary(v) => v.base,
            _ => self,
        }
    }
}

impl ASTNode {
    pub fn empty_statement(r: Range) -> Self {
        Self { kind: ASTKind::EmptyStatement, range: r }
    }
    pub fn suite(v: Vec<ASTNode>, r: Range) -> Self {
        Self { kind: ASTKind::Suite(v), range: r }
    }

    pub fn if_statement(pairs: Vec<(ASTNode, ASTNode)>, default: Option<ASTNode>, r: Range) -> Self {
        let s = IfStatement { pairs, default };

        Self { kind: ASTKind::IfStatement(box s), range: r }
    }

    pub fn expression(base: ASTNode, eos: bool, r: Range) -> Self {
        Self { kind: ASTKind::Expression { base: box base, eos }, range: r }
    }

    pub fn string_expression(h: &str, v: &[ASTNode], r: Range) -> Self {
        let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        let v = StringLiteral { handler, value: Vec::from(v) };
        Self { kind: ASTKind::StringExpression(box v), range: r }
    }

    pub fn push_infix_chain(self, op: &str, rhs: ASTNode, r: Range) -> Self {
        let op = Operator::parse(op, 0);

        let mut infix = match self.kind {
            ASTKind::CallInfix(e) if op.get_priority() == e.get_priority() => *e,
            _ => InfixCall { base: self, terms: vec![] },
        };
        infix.push_infix_pair(op, rhs);
        Self { kind: ASTKind::CallInfix(box infix), range: r }
    }

    pub fn push_unary_operations(self, prefix: &[String], suffix: &[String], r: Range) -> Self {
        if prefix.is_empty() && suffix.is_empty() {
            return self.refine();
        }
        let mut unary = match self.kind {
            ASTKind::CallUnary(u) => *u,
            _ => UnaryCall::new(self),
        };
        unary.push_prefix(prefix);
        unary.push_suffix(suffix);
        Self { kind: ASTKind::CallUnary(box unary), range: r }
    }

    pub fn chain_join(self, terms: ASTNode) -> Self {
        ChainCall::join_chain_terms(self, &[terms])
    }

    pub fn apply_call(args: Vec<ASTNode>, kvs: Vec<(ASTNode, ASTNode)>, r: Range) -> Self {
        let mut kv_pairs: HashMap<String, ASTNode> = Default::default();
        for (kn, v) in kvs {
            let k = match &kn.kind {
                ASTKind::Symbol(s) => s.name.to_owned(),
                _ => unimplemented!("{:?}", kn.kind),
            };
            kv_pairs.insert(k, v);
        }
        let kind = ApplyCallTerm { args, kv_pairs };
        ASTNode { kind: ASTKind::CallApply(box kind), range: r }
    }

    pub fn apply_slice(indexes: &[ASTNode], r: Range) -> Self {
        let kind = SliceTerm { terms: Vec::from(indexes) };
        ASTNode { kind: ASTKind::CallSlice(box kind), range: r }
    }

    pub fn apply_index(start: Option<ASTNode>, end: Option<ASTNode>, steps: Option<ASTNode>, r: Range) -> Self {
        let kind = IndexTerm { start, end, steps };
        ASTNode { kind: ASTKind::CallIndex(box kind), range: r }
    }

    pub fn list(v: Vec<ASTNode>, r: Range) -> Self {
        Self { kind: ASTKind::ListExpression(v), range: r }
    }

    pub fn tuple(v: Vec<ASTNode>, r: Range) -> Self {
        Self { kind: ASTKind::TupleExpression(v), range: r }
    }

    pub fn symbol(path: &[String], r: Range) -> Self {
        Self { kind: ASTKind::Symbol(box Symbol::path(path)), range: r }
    }

    pub fn number(h: &str, v: &str, r: Range) -> Self {
        let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        let v = NumberLiteral { handler, value: String::from(v) };
        Self { kind: ASTKind::NumberLiteral(box v), range: r }
    }

    pub fn bytes(h: char, v: &str, r: Range) -> Self {
        // let handler = if h.is_empty() { None } else { Some(String::from(h)) };
        let v = ByteLiteral { handler: h, value: String::from(v) };
        Self { kind: ASTKind::ByteLiteral(box v), range: r }
    }

    pub fn string(s: &str, r: Range) -> Self {
        Self { kind: ASTKind::String(box String::from(s)), range: r }
    }

    pub fn boolean(v: bool, r: Range) -> Self {
        Self { kind: ASTKind::Boolean(v), range: r }
    }

    pub fn null(r: Range) -> Self {
        Self { kind: ASTKind::Null, range: r }
    }
}
