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

pub struct TableBuilder {
    inner: Vec<ASTNode>,
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self { inner: vec![] }
    }
}

impl TableBuilder {
    pub fn push_node(&mut self, v: ASTNode) {
        todo!()
    }
    pub fn push_pair(&mut self, k: ASTNode, v: ASTNode, r: Span) {
        let pair = KVPair { key: k, value: v };
        self.inner.push(pair.as_node(r));
    }
    pub fn finish(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::TableExpression(box TableExpression { inner: self.inner }), span }
    }
}

impl TableExpression {
    pub fn insert_pair(&mut self, _pair: KVPair) {}
}
