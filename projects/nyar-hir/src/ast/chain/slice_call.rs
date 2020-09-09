use super::*;

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug)]
pub struct SliceCall {
    base: ASTNode,
    terms: Vec<ASTNode>,
}


impl SliceCall {
    pub fn new(base: ASTNode) ->Self {
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