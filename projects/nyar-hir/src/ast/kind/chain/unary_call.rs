use super::*;

///
/// ```v
/// ++base!!
/// ```
#[derive(Clone, Debug)]
pub struct UnaryCall {
    prefix: Vec<Operator>,
    suffix: Vec<Operator>
}