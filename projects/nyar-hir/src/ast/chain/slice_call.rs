use super::*;

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug)]
pub struct SliceCallTerm {
    pub start: Option<ASTNode>,
    pub end: Option<ASTNode>,
    pub steps: Option<ASTNode>,
}

impl SliceCallTerm {
    // pub fn new(base: ASTNode) -> Self {
    //     Self { base, terms: vec![] }
    // }
    // pub fn push(&mut self, term: ASTNode) {
    //     self.terms.push(term)
    // }
    // pub fn extend(&mut self, terms: &[ASTNode]) {
    //     self.terms.extend_from_slice(terms)
    // }
}
