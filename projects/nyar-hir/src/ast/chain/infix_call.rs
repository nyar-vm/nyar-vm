use super::*;

///
/// ```v
/// base + node1 + node2
/// ```
#[derive(Clone, Debug)]
pub struct InfixCall {
    terms: Vec<(Operator, ASTNode)>,
}
