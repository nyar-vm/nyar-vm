use super::*;

#[derive(Clone, Debug)]
pub enum OperatorKind {}

#[derive(Clone, Debug)]
pub enum OperatorAss {}

#[derive(Clone, Debug)]
pub struct Operator {
    kind: OperatorKind,
    asso: OperatorAss,
    prec: u8,
    op: String,
}
