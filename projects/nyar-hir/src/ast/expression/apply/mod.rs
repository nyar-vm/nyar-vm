use super::*;

/// ```vk
/// [].a
/// [].a()
/// [].a::[]()
/// [].a::[]() {
///    continuation
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyCall {
    pub base: ASTNode,
    pub generic: Option<ASTNode>,
    pub arguments: Option<ASTNode>,
    pub continuation: Option<ASTNode>,
}

impl ChainBuilder {
    pub fn push_apply_call(&mut self) {
        match self.base.kind {
            _ => unimplemented!("{:?}", self.base.kind),
        }
    }
}
