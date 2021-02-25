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

impl Debug for KVPair {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pair").field("key", &self.key).field("value", &self.value).finish()
    }
}

impl Debug for TableExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut w = &mut f.debug_tuple("Table");
        for node in &self.inner {
            w = w.field(node)
        }
        w.finish()
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
