use super::*;

///
/// ```v
/// a(x, value: 2, ..y)
/// ```
#[derive(Clone, Debug)]
pub struct ApplyCall {
    terms: Vec<ASTNode>,
}
