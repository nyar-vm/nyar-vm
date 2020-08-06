use super::*;
///
#[derive(Debug, Clone)]
pub enum OperatorKind {}
///
#[derive(Debug, Clone)]
pub enum OperatorAss {}
///
#[derive(Debug, Clone)]
pub struct Symbol {
    name: String,
    scope: Vec<StringRange>,
}
