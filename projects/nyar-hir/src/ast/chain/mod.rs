use super::*;

pub use self::{
    apply_call::ApplyArgument,
    slice_call::{IndexTerm, SliceArgument},
    unary_call::UnaryCall,
};

mod apply_call;
mod dict_call;
mod kont_call;
mod slice_call;
mod unary_call;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainCall {
    pub base: ASTNode,
    pub chain: Vec<CallableItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CallableItem {
    ApplyCall(ApplyArgument),
    SliceCall(SliceArgument),
}

impl ChainCall {
    pub fn new(base: ASTNode) -> Self {
        Self { base, chain: vec![] }
    }
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::ApplyExpression(box self), span }
    }
}
