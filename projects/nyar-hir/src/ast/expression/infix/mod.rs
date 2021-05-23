use crate::ast::*;

///
/// ```v
/// base (+ node1) (+ node2)
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfixCall {
    pub operator: Operator,
    pub terms: Vec<ASTNode>,
}

impl InfixCall {
    pub fn new(lhs: ASTNode, operator: Operator, rhs: ASTNode) -> Self {
        Self { operator, terms: vec![lhs, rhs] }
    }
    pub fn get_priority(&self) -> u8 {
        self.operator.get_priority()
    }
}