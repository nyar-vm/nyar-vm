use super::*;

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct SliceArgument {
    pub terms: Vec<IndexTerm>,
}

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexTerm {
    pub start: Option<ASTNode>,
    pub end: Option<ASTNode>,
    pub steps: Option<ASTNode>,
}

impl AddAssign<SliceArgument> for ChainCall {
    fn add_assign(&mut self, rhs: SliceArgument) {
        self.chain.push(CallableItem::SliceCall(rhs));
    }
}

impl AddAssign<IndexTerm> for SliceArgument {
    fn add_assign(&mut self, rhs: IndexTerm) {
        self.terms.push(rhs);
    }
}

impl Debug for SliceArgument {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut w = &mut f.debug_struct("SliceArgument");
        for (i, term) in self.terms.iter().enumerate() {
            let key = &format!("{}.start", i);
            w = match &term.start {
                None => w.field(key, &"1"),
                Some(s) => w.field(key, &s),
            };
            let key = &format!("{}.end", i);
            w = match &term.end {
                None => w.field(key, &"-1"),
                Some(s) => w.field(key, &s),
            };
            let key = &format!("{}.steps", i);
            w = match &term.steps {
                None => w.field(key, &"1"),
                Some(s) => w.field(key, &s),
            };
        }
        w.finish()
    }
}

impl Default for SliceArgument {
    fn default() -> Self {
        Self { terms: vec![] }
    }
}

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
