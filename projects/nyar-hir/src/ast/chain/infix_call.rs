use super::*;

///
/// ```v
/// base (+ node1) (+ node2)
/// ```
#[derive(Clone, Debug)]
pub struct InfixCall {
    pub base: ASTNode,
    pub terms: Vec<(Operator, ASTNode)>,
}

impl InfixCall {
    pub fn new(base: ASTNode) -> Self {
        Self { base, terms: vec![] }
    }
    pub fn push_infix_pair(&mut self, op: Operator, base: ASTNode) {
        self.terms.push((op, base))
    }
    pub fn get_priority(&self) -> u8 {
        match &self.base.kind {
            ASTKind::Operator(o) => o.get_priority(),
            _ => unreachable!()
        }
    }
}
