use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IfStatement {
    pub pairs: Vec<(ASTNode, Vec<ASTNode>)>,
    pub default: Option<Vec<ASTNode>>,
}

impl Default for IfStatement {
    fn default() -> Self {
        Self { pairs: vec![], default: None }
    }
}

impl IfStatement {
    /// `if a {b}`
    pub fn if_only(condition: ASTNode, body: Vec<ASTNode>) -> Self {
        Self { pairs: vec![(condition, body)], default: None }
    }
    /// `if a {b} else {c}`
    pub fn if_else(condition: ASTNode, body: Vec<ASTNode>, default: Vec<ASTNode>) -> Self {
        Self { pairs: vec![(condition, body)], default: Some(default) }
    }
    pub fn push_else_if(&mut self, condition: ASTNode, body: Vec<ASTNode>) {
        self.pairs.push((condition, body))
    }
    pub fn push_else(&mut self, default: Vec<ASTNode>) {
        self.default = Some(default);
    }
    pub fn as_node(&self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::IfStatement(box self.clone()), span }
    }
}
