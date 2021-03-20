use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct TableExpression {
    pub inner: Vec<ASTNode>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct KVPair {
    pub key: ASTNode,
    pub value: ASTNode,
}

impl KVPair {
    pub fn new(key: ASTNode, value: ASTNode) -> Self {
        Self { key, value }
    }
    pub fn key_name(&self) -> Option<String> {
        todo!()
    }
    // pub fn key_id(&self) -> Option<usize> {
    //     match &self.k.kind {
    //         ASTKind::NumberLiteral(s) => {
    //             s.
    //         },
    //         _ => None
    //     }
    // }
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
}

impl TableExpression {
    pub fn insert_pair(&mut self, _pair: KVPair) {}
}
