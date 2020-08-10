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
    pub fn push_infix_pair(&self, _op: &str, _base: ASTNode) {
        print!("{}", _op);
        print!("{}", _op);
    }
}
