use super::*;

/// ```vk
/// [].a
/// [].a()
/// [].a::[]()
/// [].a::[]() {
///    continuation
/// }
/// ```
pub struct DotCall {
    pub base: ASTNode,
    pub generic: Option<ASTNode>,
    pub arguments: Option<ASTNode>,
    pub continuation: Option<ASTNode>,
}

impl ChainBuilder {
    pub fn push_dot_call(&mut self, s: Symbol) {
        match self.base.kind {
            _ => unimplemented!("{:?}", self.base.kind),
        }
    }
}
