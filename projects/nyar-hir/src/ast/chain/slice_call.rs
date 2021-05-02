use super::*;

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SliceArgument {
    pub terms: Vec<SliceTerm>,
}

/// Valid slice range
///
/// ```v
/// [index]
/// [start:end]
/// [start:end:step]
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SliceTerm {
    Index { index: ASTNode },
    Slice { start: Option<ASTNode>, end: Option<ASTNode>, steps: Option<ASTNode> },
}

impl AddAssign<SliceArgument> for ChainCall {
    fn add_assign(&mut self, rhs: SliceArgument) {
        self.chain.push(CallableItem::SliceCall(rhs));
    }
}

impl AddAssign<SliceTerm> for SliceArgument {
    fn add_assign(&mut self, rhs: SliceTerm) {
        self.terms.push(rhs);
    }
}

impl Default for SliceArgument {
    fn default() -> Self {
        Self { terms: vec![] }
    }
}

impl SliceArgument {
    pub fn push_index(&mut self, index: ASTNode) {
        self.terms.push(SliceTerm::Index { index });
    }
    pub fn push_slice(&mut self, start: Option<ASTNode>, end: Option<ASTNode>, steps: Option<ASTNode>) {
        self.terms.push(SliceTerm::Slice { start, end, steps });
    }
}
