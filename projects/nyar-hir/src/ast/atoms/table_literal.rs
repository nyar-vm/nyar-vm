use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableExpression {
    pub inner: Vec<ASTNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KVPair {
    pub key: ASTNode,
    pub value: ASTNode,
}

impl Default for TableExpression {
    fn default() -> Self {
        Self { inner: vec![] }
    }
}

impl TableExpression {
    pub fn push_node(&mut self, v: ASTNode) {
        self.inner.push(v);
    }
    pub fn push_pair(&mut self, k: ASTNode, v: ASTNode, r: Span) {
        self.inner.push(KVPair { key: k, value: v }.as_node(r));
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::TableExpression(box self), span }
    }
}
impl KVPair {
    pub fn new(key: ASTNode, value: ASTNode) -> Self {
        Self { key, value }
    }
    pub fn key_name(&self) -> Option<String> {
        todo!()
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::PairExpression(box self), span }
    }
}
