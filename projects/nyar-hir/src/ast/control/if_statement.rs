use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct IfStatement {
    pub pairs: Vec<(ASTNode, Vec<ASTNode>)>,
    pub default: Option<Vec<ASTNode>>,
    // annotations: Option<Box<ASTKind>>,
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
    pub fn with_else_if(mut self, condition: ASTNode, body: Vec<ASTNode>) -> Self {
        self.pairs.push((condition, body));
        self
    }
    pub fn with_else(mut self, default: Vec<ASTNode>) -> Self {
        self.default = Some(default);
        self
    }
}
