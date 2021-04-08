use nyar_error::third_party::debug_indent;

use super::*;

pub use self::{
    apply_call::ApplyArgument,
    block_call::ContinuationArgument,
    slice_call::{SliceArgument, SliceTerm},
    unary_call::UnaryArgument,
};

mod apply_call;
mod block_call;
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

impl Debug for ChainCall {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "(chain-call")?;
        for node in &self.chain {
            match node {
                CallableItem::DotCall(v) => writeln!(f, "    (dot-call {})", v)?,
                CallableItem::ApplyCall(v) => debug_indent(v, f)?,
                CallableItem::SliceCall(v) => debug_indent(v, f)?,
                CallableItem::UnaryCall(v) => debug_indent(v, f)?,
                CallableItem::BlockCall(v) => debug_indent(v, f)?,
                CallableItem::StaticCall(v) => writeln!(f, "    (static-call {})", v)?,
            }
        }
        write!(f, ")")
    }
}

impl Debug for SliceArgument {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for node in &self.terms {
            match node {
                SliceTerm::Index { index } => writeln!(f, "(index-call {})", index)?,
                SliceTerm::Slice { start, end, steps } => writeln!(f, "(slice-call {} {} {})", start, end, steps)?,
            }
        }
        Ok(())
    }
}
