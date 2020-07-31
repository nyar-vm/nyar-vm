use super::*;

pub enum OperatorKind {

}

pub enum OperatorAss {

}

pub struct Operator {
    kind: OperatorKind,
    asso: OperatorAss,
    prec: u8,
    op: String,

}