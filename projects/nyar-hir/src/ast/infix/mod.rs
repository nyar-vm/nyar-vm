use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct InfixExpression {
    pub o: Operator,
    pub lhs: ASTNode,
    pub rhs: ASTNode,
}

impl Operator {
    pub fn is_shortcut_and(&self) -> bool {
        matches!(self, Operator::Infix { op: "&&", .. })
    }
    pub fn is_shortcut_or(&self) -> bool {
        matches!(self, Operator::Infix { op: "||", .. })
    }
}
