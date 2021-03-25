use nyar_error::third_party::BigInt;

use super::*;

pub use self::{
    apply_call::ApplyArgument,
    slice_call::{SliceArgument, SliceTerm},
    unary_call::UnaryArgument,
};

mod apply_call;
mod dict_call;
mod dot_call;
mod slice_call;
mod unary_call;

#[derive(Clone, Serialize, Deserialize)]
pub struct ChainCall {
    pub base: ASTNode,
    pub chain: Vec<CallableItem>,
}

/// ```vk
/// a
/// a::b()
/// a::[]()
/// a::[]() {
///    continuation
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CallableItem {
    /// ```vk
    /// a
    /// a::b()
    /// a::[]()
    /// a::[]() {
    ///    continuation
    /// }
    /// ```
    ApplyCall(ApplyArgument),
    SliceCall(SliceArgument),
    /// ```vk
    /// ++a--
    /// ```
    UnaryCall(UnaryArgument),
    DotCall(Symbol),
    StaticCall(String),
}

impl Default for ChainCall {
    fn default() -> Self {
        ChainCall { base: ASTNode::default(), chain: Vec::new() }
    }
}

impl ChainCall {
    pub fn as_node(self, span: Span) -> ASTNode {
        if self.chain.is_empty() {
            return self.base;
        }
        ASTNode { kind: ASTKind::ApplyExpression(box self), span }
    }
}

impl Debug for ChainCall {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut w = &mut f.debug_tuple("ChainCall");
        w = w.field(&self.base);
        for node in &self.chain {
            w = match node {
                CallableItem::ApplyCall(v) => w.field(v),
                CallableItem::SliceCall(v) => w.field(v),
                CallableItem::UnaryCall(v) => w.field(v),
                CallableItem::DotCall(v) => w.field(&format!("DotCall({})", v)),
                CallableItem::StaticCall(v) => w.field(&format!("StaticCall({})", v)),
            }
        }
        w.finish()
    }
}
