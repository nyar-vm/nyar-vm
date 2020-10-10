use super::*;

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug)]
pub struct SliceTerm {
    pub terms: Vec<ASTNode>,
}

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug)]
pub struct IndexTerm {
    pub start: Option<ASTNode>,
    pub end: Option<ASTNode>,
    pub steps: Option<ASTNode>,
}

impl Display for SliceTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.terms).finish()
    }
}

// impl Display for IndexTerm {
//     fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
//         f.debug_struct()
//     }
// }

impl IndexTerm {
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
