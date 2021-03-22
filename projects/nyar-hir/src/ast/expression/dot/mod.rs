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
pub struct DotCall {
    pub base: ASTNode,
    pub symbol: Symbol,
    pub generic: Option<ASTNode>,
    pub arguments: Option<ASTNode>,
    pub continuation: Option<ASTNode>,
}
