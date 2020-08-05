use super::*;

///
/// ```v
/// a[1][2]
/// ```
#[derive(Clone, Debug)]
pub struct SliceCall {
    terms: ASTNode,
}
