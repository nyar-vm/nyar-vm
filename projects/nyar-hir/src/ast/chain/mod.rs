use super::*;

pub use self::{
    apply_call::ApplyArgument,
    block_call::ContinuationArgument,
    slice_call::{SliceArgument, SliceTerm},
    unary_call::UnaryArgument,
};

mod apply_call;
mod block_call;
mod dot_call;
mod slice_call;
mod unary_call;

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// ```vk
    /// a.1
    /// a[1]
    /// a[b]
    /// a[start:end:steps]
    /// ```
    SliceCall(SliceArgument),
    /// ```vk
    /// ++a--
    /// ```
    UnaryCall(UnaryArgument),
    /// ```vk
    /// a
    /// a::b()
    /// a::[]()
    /// a::[]() {
    ///    continuation
    /// }
    /// ```
    DotCall(Symbol),
    /// ```vk
    /// a.b
    /// ```
    StaticCall(String),
    /// ```vk
    /// a
    /// a::b {}
    /// a::[]()
    /// a::[]() {
    ///    continuation
    /// }
    /// ```
    BlockCall(ContinuationArgument),
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
