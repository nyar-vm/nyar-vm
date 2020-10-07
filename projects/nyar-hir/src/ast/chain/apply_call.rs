use super::*;
use std::collections::HashMap;

///
/// ```v
/// a(x, value: 2, ..y)
/// ```
#[derive(Clone, Debug)]
pub struct ApplyCallTerm {
    pub args: Vec<ASTNode>,
    pub kv_pairs: HashMap<String, ASTNode>,
}

impl ApplyCallTerm {
    // pub fn new(base: ASTNode) -> Self {
    //     Self {
    //         base,
    //         terms: vec![]
    //     }
    // }
    // pub fn push(&mut self, term: ASTNode) {
    //     self.terms.push(term)
    // }
    // pub fn extend(&mut self, terms: &[ASTNode]) {
    //     self.terms.extend_from_slice(terms)
    // }
}