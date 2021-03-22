use super::*;

pub use self::{
    slice_call::{IndexTerm, SliceTerm},
    unary_call::UnaryCall,
};

pub(crate) mod apply_call;
pub(crate) mod dict_call;
pub(crate) mod kont_call;
pub(crate) mod slice_call;
pub(crate) mod unary_call;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainCall {
    pub base: ASTNode,
    pub chain: Vec<CallableItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CallableItem {
    ApplyCall(ApplyArgument),
}

impl AddAssign<ApplyArgument> for ChainCall {
    fn add_assign(&mut self, rhs: ApplyArgument) {
        self.chain.push(CallableItem::ApplyCall(rhs));
    }
}

impl ChainCall {
    pub fn new(base: ASTNode) -> Self {
        Self { base, chain: vec![] }
    }
}
