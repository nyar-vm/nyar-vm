use super::*;

///
/// ```v
/// base {
///     key: value
/// }
/// ```
#[derive(Clone, Debug)]
pub struct DictCall {
    terms: ASTNode,
}
