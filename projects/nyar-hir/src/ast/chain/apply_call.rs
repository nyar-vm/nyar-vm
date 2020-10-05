use super::*;

///
/// ```v
/// a(x, value: 2, ..y)
/// ```
#[derive(Clone, Debug)]
pub struct ApplyCall {
    pub base: ASTNode,
    pub terms: Vec<ASTNode>,
}

impl ApplyCall {
    pub fn new(base: ASTNode) -> Self {
        Self {
            base,
            terms: vec![]
        }
    }
    pub fn push(&mut self, term: ASTNode) {
        self.terms.push(term)
    }
    pub fn extend(&mut self, terms: &[ASTNode]) {
        self.terms.extend_from_slice(terms)
    }
}