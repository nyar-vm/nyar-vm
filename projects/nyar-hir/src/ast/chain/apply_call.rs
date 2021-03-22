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
    pub arguments: Vec<ASTNode>,
    pub options: BTreeMap<String, ASTNode>,
    pub continuation: Option<ASTNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyArgument {
    positional: Vec<ASTNode>,
    named: BTreeMap<String, ASTNode>,
}

impl Default for ApplyArgument {
    fn default() -> Self {
        Self { positional: vec![], named: Default::default() }
    }
}

impl ApplyArgument {
    pub fn push(&mut self, node: ASTNode) {
        self.positional.push(node);
    }
    pub fn push_named(&mut self, name: Symbol, node: ASTNode) {
        assert!(name.scope.is_empty());
        self.named.insert(name.name, node);
    }
}
