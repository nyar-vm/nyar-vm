use super::*;

mod apply_call;
mod dict_call;
mod infix_call;
mod kont_call;
mod slice_call;
mod unary_call;

pub use self::{apply_call::ApplyCallTerm, infix_call::InfixCall, slice_call::{SliceTerm,IndexTerm}, unary_call::UnaryCall};

#[derive(Debug, Clone)]
pub struct ChainCall {
    pub base: ASTNode,
    /// one of calls
    pub chain: Vec<ASTNode>,
}

impl Default for ChainCall {
    fn default() -> Self {
        Self { base: Default::default(), chain: vec![] }
    }
}

impl AddAssign<ASTNode> for ChainCall {
    fn add_assign(&mut self, rhs: ASTNode) {
        self.chain.push(rhs)
    }
}

impl ChainCall {
    pub fn new(base: ASTNode) -> Self {
        Self { base, chain: vec![] }
    }
    pub fn push(&mut self, term: ASTNode) {
        self.chain.push(term)
    }
    pub fn extend(&mut self, terms: &[ASTNode]) {
        self.chain.extend_from_slice(terms)
    }
    pub fn join_chain_terms(base: ASTNode, terms: &[ASTNode]) -> ASTNode {
        assert_ne!(terms.len(), 0);
        let start = base.range.start;
        let end = terms.iter().last().unwrap().range.end;
        let mut chain = match base.kind {
            ASTKind::CallChain(c) => *c,
            _ => ChainCall::new(base),
        };
        chain.extend(terms);
        ASTNode {
            kind: ASTKind::CallChain(box chain),
            range: Range {
                start,
                end,
            },
        }
    }
}
